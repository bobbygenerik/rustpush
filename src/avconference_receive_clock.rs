//! Receive-delay clock for a single authenticated audio SSRC.
//! iOS 27 VCRateControlOWRDEstimator.relativeSendTimeWithTimestamp:timestampRate:
//! (0x205fc0648) handles wrapping and excludes duplicate/out-of-order samples.
//! This implements that counter invariant, not Apple's full spike/reset policy.
use std::time::Duration;

/// RTPSetAFRCProcessingDelay (0x205fa57c4) preserves NaN as unknown and packs
/// nonnegative relative delay at 8192 ticks/second. Loss does not cap the value.
/// Keep the existing u16 overflow saturation until the full wire range is mapped.
pub(super) fn pack_q13_delay(delay: f64) -> u16 {
    if delay.is_nan() { return u16::MAX; }
    (delay.max(0.0) * 8192.0) as u16
}

#[derive(Default)]
pub(super) struct ReceiveClock {
    previous: Option<u32>,
    ticks: u64,
    first_received: Duration,
}

impl ReceiveClock {
    pub(super) fn lag(&mut self, timestamp: u32, received: Duration) -> Option<f64> {
        let Some(previous) = self.previous else {
            self.previous = Some(timestamp);
            self.first_received = received;
            return Some(0.0);
        };
        let delta = timestamp.wrapping_sub(previous);
        // The RTP half-range rule distinguishes forward wrap from late packets.
        // Do not let a duplicate/retransmission change the delay estimate.
        if delta == 0 || delta >= 0x8000_0000 { return None; }
        self.previous = Some(timestamp);
        self.ticks += u64::from(delta);
        let received_elapsed = received.saturating_sub(self.first_received).as_secs_f64();
        // The negotiated ELD path uses a 24 kHz RTP clock, as before this fix.
        Some(received_elapsed - self.ticks as f64 / 24_000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measured_delay_is_not_clipped_to_the_old_zero_loss_cap() {
        // Direct iOS 27 fixed-point conversion: 250ms -> 2048, 1s -> 8192.
        assert_eq!(pack_q13_delay(0.25), 2048);
        assert_eq!(pack_q13_delay(1.0), 8192);
        assert_eq!(pack_q13_delay(-0.25), 0);
    }

    #[test]
    fn unknown_delay_survives_negative_clamping() {
        // f64::max(NaN, 0) returns zero; test NaN before that operation.
        assert_eq!(pack_q13_delay(f64::NAN), u16::MAX);
        assert_eq!(pack_q13_delay(0.0), 0);
    }

    #[test]
    fn ordinary_clock_preserves_delay_growth() {
        let mut clock = ReceiveClock::default();
        assert_eq!(clock.lag(42, Duration::from_secs(3)), Some(0.0));
        let lag = clock.lag(522, Duration::from_millis(3045)).unwrap();
        assert!((lag - 0.025).abs() < 1e-12);
    }

    #[test]
    fn wrap_does_not_fabricate_a_fifty_hour_delay() {
        let mut clock = ReceiveClock::default();
        let first = u32::MAX - 239;
        clock.lag(first, Duration::ZERO);
        assert_eq!(clock.lag(240, Duration::from_millis(20)), Some(0.0));
        // The replaced raw subtraction reports a huge positive lag here.
        let old_lag = 0.020 - (240.0 - f64::from(first)) / 24_000.0;
        assert!(old_lag > 178_000.0);
        assert_eq!(clock.lag(720, Duration::from_millis(40)), Some(0.0));
    }

    #[test]
    fn late_and_duplicate_samples_do_not_rewind_the_clock() {
        let mut clock = ReceiveClock::default();
        clock.lag(u32::MAX - 239, Duration::ZERO);
        clock.lag(240, Duration::from_millis(20));
        assert_eq!(clock.lag(u32::MAX - 239, Duration::from_millis(30)), None);
        assert_eq!(clock.lag(240, Duration::from_millis(35)), None);
        assert_eq!(clock.lag(720, Duration::from_millis(40)), Some(0.0));
    }

    #[test]
    fn zero_is_a_valid_initial_timestamp() {
        let mut clock = ReceiveClock::default();
        assert_eq!(clock.lag(0, Duration::from_secs(7)), Some(0.0));
        assert_eq!(clock.lag(480, Duration::from_millis(7020)), Some(0.0));
    }

    #[test]
    fn independent_streams_have_independent_origins() {
        let mut clocks = std::collections::HashMap::<(u64, u32), ReceiveClock>::new();
        clocks.entry((1, 10)).or_default().lag(100, Duration::ZERO);
        assert_eq!(clocks.entry((1, 11)).or_default().lag(500_000, Duration::from_secs(30)), Some(0.0));
        assert_eq!(clocks.entry((2, 10)).or_default().lag(123, Duration::from_secs(35)), Some(0.0));
        assert_eq!(clocks.entry((1, 10)).or_default().lag(580, Duration::from_millis(20)), Some(0.0));
    }

    #[test]
    fn long_call_can_cross_multiple_wraps() {
        let mut clock = ReceiveClock::default();
        clock.lag(0, Duration::ZERO);
        let step = 24_000_u32 * 3_600;
        for hour in 1..=120_u64 {
            let ticks = hour * u64::from(step);
            assert_eq!(clock.lag(ticks as u32, Duration::from_secs(hour * 3_600)), Some(0.0));
        }
        assert!(clock.ticks > u64::from(u32::MAX) * 2);
    }
}
