//! Apple AVConference 1:1 audio OWRD tracking and wire Q13 packing.
//!
//! Replaces the ad-hoc `ReceiveClock` and 0.25 s baseline cap with Apple's
//! `JLagProcessor` state machine (iOS 27.0.1 `24A446`, `AVConference`).

use crate::jlag_processor::{JLagProcessor, LagPacket};

/// Packs a raw OWRD value in seconds into the Q13 wire `u16` (`owrd`).
///
/// Evidence in iOS 27.0.1 (`24A446`, `AVConference`):
/// - `_RTPSetAFRCProcessingDelay` (`0x205fa57c8..0x205fa57dc`):
///   `fcvtzu w9, d0, #0xd` scales by `2^13 = 8192` to 32-bit `u32`;
///   negative values clamp to `0` (`csel w9, wzr, w9, mi`);
///   `NaN` selects `0xffff` (`fcmp d0, d0; csel w8, w8, w9, vs`).
/// - `_VCMediaControlInfoSerializeWithData` (`0x2064c20bc..0x2064c20d0`):
///   clamps any 32-bit value `>= 0xffff` to `0xffff` (`cmp w9, #0xffff; csel w9, w9, w12, cc`)
///   before writing big-endian `u16` on the wire.
pub fn pack_q13_delay(delay: f64) -> u16 {
    if delay.is_nan() {
        return u16::MAX;
    }
    if delay < 0.0 {
        return 0;
    }
    let scaled = delay * 8192.0;
    if scaled >= f64::from(u16::MAX) {
        u16::MAX
    } else {
        scaled as u16
    }
}

/// Per-stream wrapper around `JLagProcessor` for 1:1 incoming audio RTP.
pub struct AudioOwrdTracker {
    processor: JLagProcessor,
    ts_rate: u32,
    samples_per_frame: u32,
    initialized: bool,
    has_recorded_owrd: bool,
}

impl AudioOwrdTracker {
    pub fn new(ts_rate: u32, samples_per_frame: u32) -> Self {
        Self {
            processor: JLagProcessor::new(),
            ts_rate,
            samples_per_frame,
            initialized: false,
            has_recorded_owrd: false,
        }
    }

    /// Feeds one authenticated incoming audio RTP packet and returns raw OWRD
    /// in seconds, or `f64::NAN` until `JLagProcessor::record_owrd` has run.
    pub fn observe_packet(
        &mut self,
        sequence_number: u16,
        timestamp: u32,
        arrival_sec: f64,
        has_field0x28: bool,
    ) -> f64 {
        let mut packet = LagPacket {
            sequence_number,
            timestamp,
            remote_time: 0.0,
            arrival_sec,
            field18: self.samples_per_frame as i32,
            field20: 1,
            flag24: 0,
            has_field0x28,
        };

        if !self.initialized {
            self.processor
                .new_stream(Some(&packet), self.ts_rate, self.samples_per_frame);
            // Apple runs `_JLagProcessor_NewStream` + `_JLagProcessor_Process(..., state = 1)`
            // on the first packet (`SETTLE`), anchoring `ref_*`/`last_lag` without calling `RecordOWRD`.
            let _ = self.processor.process(
                Some(&mut packet),
                self.samples_per_frame as i32,
                0,
                1,
            );
            self.initialized = true;
            return self.owrd_or_nan();
        }

        let pre_short = self.processor.owrd_short;
        let pre_long = self.processor.owrd_long;

        // Approximation: `VCJitterBuffer` state machine is not yet wired into rustpush,
        // so pass `state = 5` (`RUN`) for every packet after the first.
        // Consequence: Apple remains in `SETTLE` (`state = 1`, `0xffff` before first `RecordOWRD`)
        // while initial jitter-buffer fill completes, whereas rustpush begins running `RecordOWRD`
        // starting on packet 2 when not in spike or rejected as an outlier.
        let lag_result = self.processor.process(
            Some(&mut packet),
            self.samples_per_frame as i32,
            0,
            5,
        );

        if !lag_result.is_nan()
            && self.processor.in_spike == 0
            && self.processor.packet_counter != 0
            && (lag_result - pre_long) <= 5.0
            && (pre_short - lag_result) <= 5.0
        {
            self.has_recorded_owrd = true;
        }

        self.owrd_or_nan()
    }

    pub fn owrd_or_nan(&self) -> f64 {
        if self.has_recorded_owrd {
            f64::from(self.processor.get_owrd())
        } else {
            f64::NAN
        }
    }

    pub fn processor(&self) -> &JLagProcessor {
        &self.processor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_q13_delay_matches_apple_clamping_and_sentinel() {
        // Direct fixed-point conversion at 2^13 = 8192 ticks/s (`0x205fa57d0`):
        assert_eq!(pack_q13_delay(0.25), 2048);
        assert_eq!(pack_q13_delay(1.0), 8192);
        // Negative clamps to 0 (`0x205fa57d4`):
        assert_eq!(pack_q13_delay(-0.25), 0);
        // NaN preserves 0xffff sentinel (`0x205fa57d8..0x205fa57dc`):
        assert_eq!(pack_q13_delay(f64::NAN), u16::MAX);
        assert_eq!(pack_q13_delay(0.0), 0);
        // Values >= 0xffff (8.0 s -> 65536 ticks) clamp to 0xffff at serialization (`0x2064c20c0..0x2064c20c4`):
        assert_eq!(pack_q13_delay(8.0), u16::MAX);
        assert_eq!(pack_q13_delay(7.9999), u16::MAX);
        assert_eq!(pack_q13_delay(f64::INFINITY), u16::MAX);
    }

    #[test]
    fn first_packet_before_record_owrd_returns_nan_and_packs_0xffff() {
        let mut tracker = AudioOwrdTracker::new(24_000, 480);
        assert!(tracker.owrd_or_nan().is_nan());
        let first = tracker.observe_packet(1, 480, 0.0, false);
        assert!(first.is_nan());
        assert_eq!(pack_q13_delay(first), u16::MAX);
    }
}
