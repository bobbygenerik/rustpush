use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// iOS27 VCVideoReceiverDefault uses _micro (mach_absolute_time) and admits
/// duplicate-stream FIR requests at 0.5s. Keep clock corrections out of this
/// budget. Native admission compares the last requested unsigned-short stream
/// on the receiving object, not every stream against one session timestamp.
/// Unknown receiver metadata retains the old session budget via allow_fir.
#[derive(Default)]
pub(super) struct VideoRecoveryPacing {
    times: Mutex<RecoveryTimes>,
    receivers: Mutex<HashMap<(u64, u32), ReceiverTimes>>,
}

#[derive(Default)]
struct ReceiverTimes {
    stream: Option<u16>,
    times: RecoveryTimes,
}

#[derive(Default)]
struct RecoveryTimes {
    last_fir: Option<Instant>,
}

impl VideoRecoveryPacing {
    pub(super) fn allow_fir(&self) -> bool {
        let mut times = self.times.lock().unwrap();
        times.allow_fir_at(Instant::now())
    }

    pub(super) fn allow_receiver_fir(&self, participant: u64, group: u32, stream: u32) -> bool {
        self.receivers.lock().unwrap().entry((participant, group)).or_default()
            .allow_at(stream as u16, Instant::now())
    }

    pub(super) fn retain_participants(&self, active: &HashSet<u64>) {
        self.receivers.lock().unwrap().retain(|(participant, _), _| active.contains(participant));
    }
}

impl ReceiverTimes {
    fn allow_at(&mut self, stream: u16, now: Instant) -> bool {
        // A stale sample must not regress the shared receiver clock, even
        // when its stream differs from the previous request.
        if self.times.last_fir.is_some_and(|last| now < last) { return false; }
        if self.stream == Some(stream) && !self.times.allow_fir_at(now) { return false; }
        self.stream = Some(stream);
        self.times.last_fir = Some(now);
        true
    }
}

impl RecoveryTimes {
    fn allow_fir_at(&mut self, now: Instant) -> bool {
        if let Some(last) = self.last_fir {
            if !now.checked_duration_since(last)
                .is_some_and(|elapsed| elapsed >= Duration::from_millis(500)) {
                return false;
            }
        }
        self.last_fir = Some(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn distinct_receivers_do_not_suppress_each_other() {
        let pacing = VideoRecoveryPacing::default();
        assert!(pacing.allow_receiver_fir(1, 0x6361_6d65, 7));
        assert!(pacing.allow_receiver_fir(2, 0x6361_6d65, 7));
        assert!(pacing.allow_receiver_fir(1, 0x7363_7265, 7));
        assert!(!pacing.allow_receiver_fir(1, 0x6361_6d65, 7));
    }

    #[test]
    fn changing_stream_bypasses_only_the_previous_stream_gate() {
        let pacing = VideoRecoveryPacing::default();
        assert!(pacing.allow_receiver_fir(1, 0, 7));
        assert!(pacing.allow_receiver_fir(1, 0, 8));
        assert!(pacing.allow_receiver_fir(1, 0, 7));
        assert!(!pacing.allow_receiver_fir(1, 0, 7));
    }

    #[test]
    fn native_unsigned_short_stream_identity_is_used() {
        let pacing = VideoRecoveryPacing::default();
        assert!(pacing.allow_receiver_fir(1, 0, 0x1234_0007));
        assert!(!pacing.allow_receiver_fir(1, 0, 7));
    }

    #[test]
    fn departed_receivers_do_not_leave_old_reservations() {
        let pacing = VideoRecoveryPacing::default();
        assert!(pacing.allow_receiver_fir(1, 0, 7));
        assert!(pacing.allow_receiver_fir(2, 0, 7));
        pacing.retain_participants(&std::collections::HashSet::from([2]));
        assert!(pacing.allow_receiver_fir(1, 0, 7));
        assert!(!pacing.allow_receiver_fir(2, 0, 7));
    }

    #[test]
    fn first_request_is_immediate_and_half_second_boundary_is_admitted() {
        let mut state = RecoveryTimes::default();
        let start = Instant::now();
        assert!(state.allow_fir_at(start));
        assert!(!state.allow_fir_at(start + Duration::from_millis(499)));
        assert!(state.allow_fir_at(start + Duration::from_millis(500)));
        assert!(!state.allow_fir_at(start + Duration::from_millis(500)));
        assert!(state.allow_fir_at(start + Duration::from_millis(1000)));
    }

    #[test]
    fn rejected_attempt_does_not_delay_next_request() {
        let mut state = RecoveryTimes::default();
        let start = Instant::now();
        assert!(state.allow_fir_at(start));
        for ms in [10, 100, 250, 499] {
            assert!(!state.allow_fir_at(start + Duration::from_millis(ms)));
        }
        assert!(state.allow_fir_at(start + Duration::from_millis(500)));
    }

    #[test]
    fn stale_clock_sample_cannot_regress_reservation() {
        let mut state = RecoveryTimes::default();
        let start = Instant::now();
        assert!(state.allow_fir_at(start + Duration::from_secs(1)));
        assert!(!state.allow_fir_at(start));
        assert!(!state.allow_fir_at(start + Duration::from_millis(1499)));
        assert!(state.allow_fir_at(start + Duration::from_millis(1500)));
    }

    #[test]
    fn session_replacement_has_no_old_reservation() {
        let pacing = VideoRecoveryPacing::default();
        assert!(pacing.allow_fir());
        assert!(!pacing.allow_fir());
        assert!(VideoRecoveryPacing::default().allow_fir());
    }

    #[test]
    fn competing_callbacks_reserve_one_request() {
        let pacing = Arc::new(VideoRecoveryPacing::default());
        let barrier = Arc::new(Barrier::new(12));
        let start = Instant::now();
        let callbacks: Vec<_> = (0..12).map(|_| {
            let pacing = Arc::clone(&pacing);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                pacing.times.lock().unwrap().allow_fir_at(start)
            })
        }).collect();
        let admitted = callbacks.into_iter()
            .map(|callback| usize::from(callback.join().unwrap())).sum::<usize>();
        assert_eq!(admitted, 1);
    }
}
