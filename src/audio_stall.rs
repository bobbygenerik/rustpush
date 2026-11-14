//! 1:1 remote-audio stall detector and disconnect policy for FaceTime.
//!
//! Citations:
//! - VCAudioStream_PullAudioSamples (0x205f7c90c)
//! - VCSession stall block (0x2061d6b80)
//! - Receive-group gate (0x205ee0cf0)

use std::time::Instant;

/// Native default remote media stall timeout (VCDefaults remoteMediaStallTimeout = 5.0s).
pub const REMOTE_MEDIA_STALL_TIMEOUT_SECS: f64 = 5.0;

/// Native disconnect floor (20.0s; server storebag value unknown).
pub const DISCONNECT_SECS: f64 = 20.0;

/// Native reconnect threshold (5.0s).
pub const RECONNECT_SECS: f64 = 5.0;

/// Native symptom report threshold (10.0s).
pub const SYMPTOM_SECS: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Report {
    pub stalled: bool,
    pub duration_secs: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    StallingChanged(bool),
    Disconnect { seconds: i64 },
}

#[derive(Debug, Clone)]
pub struct AudioStallDetector {
    last_seen: Option<Instant>,
    count: u8,
    stalled: bool,
    recovery: u32,
    timeout_secs: f64,
}

impl Default for AudioStallDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioStallDetector {
    pub fn new() -> Self {
        Self::with_timeout(REMOTE_MEDIA_STALL_TIMEOUT_SECS)
    }

    pub fn with_timeout(timeout_secs: f64) -> Self {
        Self {
            last_seen: None,
            count: 0,
            stalled: false,
            recovery: 0,
            timeout_secs,
        }
    }

    pub fn is_stalled(&self) -> bool {
        self.stalled
    }

    pub fn count(&self) -> u8 {
        self.count
    }

    pub fn poll(&mut self, now: Instant, last_audio_arrival: Option<Instant>) -> Option<Report> {
        // 1. If last_audio_arrival is None: return None (clock starts at the
        //    first audio packet; documented app choice).
        let arrival = last_audio_arrival?;

        // 2. If last_audio_arrival > last_seen: if stalled { recovery += 1 };
        //    last_seen = last_audio_arrival.
        let is_newer = match self.last_seen {
            Some(seen) => arrival > seen,
            None => true,
        };
        if is_newer {
            if self.stalled {
                self.recovery = self.recovery.saturating_add(1);
            }
            self.last_seen = Some(arrival);
        }

        // 3. gap = now - last_audio_arrival (seconds, f64).
        let gap = if now >= arrival {
            (now - arrival).as_secs_f64()
        } else {
            0.0
        };

        // 4. threshold = T * (count as f64 + 1.0) / 3.0.
        let threshold = self.timeout_secs * (self.count as f64 + 1.0) / 3.0;

        // 5. If gap > threshold: if count == 0 { stalled = true; recovery = 0 };
        //    count = count.saturating_add(1);
        //    return Some(Report { stalled, duration_secs: gap }).
        if gap > threshold {
            if self.count == 0 {
                self.stalled = true;
                self.recovery = 0;
            }
            // Native count is a byte; use saturating_add and note it.
            self.count = self.count.saturating_add(1);
            Some(Report {
                stalled: self.stalled,
                duration_secs: gap,
            })
        // 6. Else if recovery >= 2 && stalled: stalled = false; count = 0;
        //    return Some(Report { stalled: false, duration_secs: 0.0 }).
        } else if self.recovery >= 2 && self.stalled {
            self.stalled = false;
            self.count = 0;
            self.recovery = 0;
            Some(Report {
                stalled: false,
                duration_secs: 0.0,
            })
        // 7. Else None.
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioStallPolicy {
    symptom_reported: bool,
    stalling: bool,
    disconnect_secs: f64,
    reconnect_secs: f64,
    symptom_secs: f64,
}

impl Default for AudioStallPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioStallPolicy {
    pub fn new() -> Self {
        Self {
            symptom_reported: false,
            stalling: false,
            disconnect_secs: DISCONNECT_SECS,
            reconnect_secs: RECONNECT_SECS,
            symptom_secs: SYMPTOM_SECS,
        }
    }

    pub fn on_report(&mut self, report: Report, audio_expected: bool) -> Action {
        // Gate (receive-group block 0x205ee0cf0):
        // if report.stalled && !audio_expected -> Action::None.
        // If !report.stalled && report.duration_secs != 0.0 -> Action::None.
        if report.stalled && !audio_expected {
            return Action::None;
        }
        if !report.stalled && report.duration_secs != 0.0 {
            return Action::None;
        }

        // d = report.duration_secs. In this order:
        let d = report.duration_secs;

        // a. if d >= SYMPTOM && !symptom_reported { symptom_reported = true;
        //    log a warning once } (then continue to b).
        if d >= self.symptom_secs && !self.symptom_reported {
            self.symptom_reported = true;
            log::warn!(
                "[AudioStall] symptom reported: remote audio stall duration {:.2}s >= {:.1}s",
                d,
                self.symptom_secs
            );
        }

        // b. if d >= DISCONNECT -> Action::Disconnect { seconds: d as i64 }.
        if d >= self.disconnect_secs {
            Action::Disconnect {
                seconds: d as i64,
            }
        // c. else if d >= RECONNECT && !stalling { stalling = true;
        //    symptom_reported = false; Action::StallingChanged(true) }.
        } else if d >= self.reconnect_secs && !self.stalling {
            self.stalling = true;
            self.symptom_reported = false;
            Action::StallingChanged(true)
        // d. else if d < RECONNECT && stalling { stalling = false;
        //    Action::StallingChanged(false) }.
        } else if d < self.reconnect_secs && self.stalling {
            self.stalling = false;
            Action::StallingChanged(false)
        // e. else Action::None.
        } else {
            Action::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_no_audio_ever_received() {
        let mut detector = AudioStallDetector::new();
        let start = Instant::now();
        for tick in 0..3000 {
            let now = start + Duration::from_millis(tick * 20);
            assert_eq!(detector.poll(now, None), None);
        }
        assert!(!detector.is_stalled());
        assert_eq!(detector.count(), 0);
    }

    #[test]
    fn test_steady_audio_for_60s() {
        let mut detector = AudioStallDetector::new();
        let start = Instant::now();
        for tick in 0..3000 {
            let now = start + Duration::from_millis(tick * 20);
            assert_eq!(detector.poll(now, Some(now)), None);
        }
        assert!(!detector.is_stalled());
        assert_eq!(detector.count(), 0);
    }

    #[test]
    fn test_t5_threshold_progression_and_durations() {
        let mut detector = AudioStallDetector::new();
        let t0 = Instant::now();
        // One packet at t=0
        assert_eq!(detector.poll(t0, Some(t0)), None);

        let mut reports = Vec::new();
        // Poll every 20ms for 10s with no new packets
        for tick in 1..=500 {
            let now = t0 + Duration::from_millis(tick * 20);
            if let Some(report) = detector.poll(now, Some(t0)) {
                reports.push((tick * 20, report));
            }
        }

        // Expected thresholds with T=5.0:
        // count 0: 5.0 * 1.0 / 3.0 = 1.66667s -> first tick > 1.66667s is 1680ms (1.68s)
        // count 1: 5.0 * 2.0 / 3.0 = 3.33333s -> first tick > 3.33333s is 3340ms (3.34s)
        // count 2: 5.0 * 3.0 / 3.0 = 5.00000s -> first tick > 5.00000s is 5020ms (5.02s)
        // count 3: 5.0 * 4.0 / 3.0 = 6.66667s -> first tick > 6.66667s is 6680ms (6.68s)
        // count 4: 5.0 * 5.0 / 3.0 = 8.33333s -> first tick > 8.33333s is 8340ms (8.34s)
        // count 5: 5.0 * 6.0 / 3.0 = 10.0000s -> 10020ms
        assert_eq!(reports.len(), 5);

        let (ms0, r0) = reports[0];
        assert_eq!(ms0, 1680);
        assert!(r0.stalled);
        assert!((r0.duration_secs - 1.68).abs() < 1e-4);

        let (ms1, r1) = reports[1];
        assert_eq!(ms1, 3340);
        assert!(r1.stalled);
        assert!((r1.duration_secs - 3.34).abs() < 1e-4);

        let (ms2, r2) = reports[2];
        assert_eq!(ms2, 5020);
        assert!(r2.stalled);
        assert!((r2.duration_secs - 5.02).abs() < 1e-4);

        let (ms3, r3) = reports[3];
        assert_eq!(ms3, 6680);
        assert!(r3.stalled);
        assert!((r3.duration_secs - 6.68).abs() < 1e-4);

        let (ms4, r4) = reports[4];
        assert_eq!(ms4, 8340);
        assert!(r4.stalled);
        assert!((r4.duration_secs - 8.34).abs() < 1e-4);
    }

    #[test]
    fn test_policy_actions_stalling_symptom_disconnect() {
        let mut policy = AudioStallPolicy::new();

        // Under 5.0s: Action::None
        assert_eq!(
            policy.on_report(
                Report {
                    stalled: true,
                    duration_secs: 1.6667
                },
                true
            ),
            Action::None
        );
        assert_eq!(
            policy.on_report(
                Report {
                    stalled: true,
                    duration_secs: 3.3333
                },
                true
            ),
            Action::None
        );

        // At d >= 5.0s: StallingChanged(true)
        assert_eq!(
            policy.on_report(
                Report {
                    stalled: true,
                    duration_secs: 5.0
                },
                true
            ),
            Action::StallingChanged(true)
        );

        // While stalling continues below 10.0s: Action::None
        assert_eq!(
            policy.on_report(
                Report {
                    stalled: true,
                    duration_secs: 6.6667
                },
                true
            ),
            Action::None
        );

        // At d >= 10.0s: symptom warning logged once, Action::None
        assert_eq!(
            policy.on_report(
                Report {
                    stalled: true,
                    duration_secs: 10.0
                },
                true
            ),
            Action::None
        );
        assert_eq!(
            policy.on_report(
                Report {
                    stalled: true,
                    duration_secs: 11.6667
                },
                true
            ),
            Action::None
        );
        assert_eq!(
            policy.on_report(
                Report {
                    stalled: true,
                    duration_secs: 18.3333
                },
                true
            ),
            Action::None
        );

        // At d >= 20.0s: Disconnect at first report with d >= 20 and never before
        assert_eq!(
            policy.on_report(
                Report {
                    stalled: true,
                    duration_secs: 20.0
                },
                true
            ),
            Action::Disconnect { seconds: 20 }
        );
    }

    #[test]
    fn test_recovery_flow() {
        let mut detector = AudioStallDetector::new();
        let mut policy = AudioStallPolicy::new();
        let t0 = Instant::now();

        // Initial packet
        assert_eq!(detector.poll(t0, Some(t0)), None);

        // Stall to 5.02s
        let t_stall = t0 + Duration::from_millis(5020);
        let r = detector.poll(t_stall, Some(t0)).unwrap();
        assert_eq!(policy.on_report(r, true), Action::StallingChanged(true));

        // After stall, one new packet arrives at t=5.50s -> does NOT recover
        let t_pkt1 = t0 + Duration::from_millis(5500);
        assert_eq!(detector.poll(t_pkt1, Some(t_pkt1)), None);
        assert!(detector.is_stalled());

        // Second new packet arrives on later tick at t=5.52s -> recovers!
        let t_pkt2 = t0 + Duration::from_millis(5520);
        let rec = detector.poll(t_pkt2, Some(t_pkt2)).expect("recovery report");
        assert_eq!(
            rec,
            Report {
                stalled: false,
                duration_secs: 0.0
            }
        );
        assert_eq!(
            policy.on_report(rec, true),
            Action::StallingChanged(false)
        );
        assert!(!detector.is_stalled());
        assert_eq!(detector.count(), 0);

        // Verify count reset: next stall starts again at 1.6667s gap
        let t_next_stall_before = t_pkt2 + Duration::from_millis(1660);
        assert_eq!(detector.poll(t_next_stall_before, Some(t_pkt2)), None);

        let t_next_stall_after = t_pkt2 + Duration::from_millis(1680);
        let next_r = detector
            .poll(t_next_stall_after, Some(t_pkt2))
            .expect("new stall report");
        assert!(next_r.stalled);
        assert!((next_r.duration_secs - 1.68).abs() < 1e-4);
        assert_eq!(detector.count(), 1);
    }

    #[test]
    fn test_audio_not_expected() {
        let mut policy = AudioStallPolicy::new();

        // Stalled reports produce Action::None indefinitely when audio_expected is false
        for d in [1.6667, 3.3333, 5.0, 10.0, 20.0, 30.0, 60.0] {
            assert_eq!(
                policy.on_report(
                    Report {
                        stalled: true,
                        duration_secs: d
                    },
                    false
                ),
                Action::None,
                "Failed for duration {d}"
            );
        }

        // Recovery report still passes even when audio_expected is false
        assert_eq!(
            policy.on_report(
                Report {
                    stalled: false,
                    duration_secs: 0.0
                },
                false
            ),
            Action::None // stalling was false so Action::None
        );

        // If it was stalling when audio_expected became false, recovery still clears it
        let mut policy2 = AudioStallPolicy::new();
        assert_eq!(
            policy2.on_report(
                Report {
                    stalled: true,
                    duration_secs: 5.0
                },
                true
            ),
            Action::StallingChanged(true)
        );
        assert_eq!(
            policy2.on_report(
                Report {
                    stalled: false,
                    duration_secs: 0.0
                },
                false
            ),
            Action::StallingChanged(false)
        );
    }
}
