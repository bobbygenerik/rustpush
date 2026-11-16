use std::time::{Duration, Instant};

pub use crate::avconference::{encode_psfb_loss, encode_psfb_ltr_ack};

/// Admission rule from `_VideoPacketBuffer_GetNextFrame` (`0x205fdd47c..0x205fdd49c`,
/// `(ltr_bits & 1) != 0`) and `__VideoReceiver_DecoderCallback` (`0x205fefa88..0x205fefcb4`,
/// decoded output valid without error and `ltr_timestamp != 0`).
pub fn should_send_ltr_ack(
    is_u1: bool,
    ltr_bits: u8,
    ltr_timestamp: Option<u32>,
    decoded_ok: bool,
) -> Option<u32> {
    if !is_u1 || !decoded_ok || (ltr_bits & 1) == 0 {
        return None;
    }
    ltr_timestamp.filter(|&ts| ts != 0)
}

/// Loss-recovery order & FIR timing from `_VideoPacketBuffer_GetNextFrame`
/// (`0x205fddc68..0x205fddc78`: wait strictly > 0.25 s before issuing refresh/FIR so
/// NACK/FEC can recover first) and `-[VCVideoReceiverDefault handleRequestingKeyFrameGenerationWithStreamID:firType:]`
/// (`0x2060e7328`: 0.5 s per-stream FIR repeat throttle).
#[derive(Default)]
pub struct VideoLossRecoveryGate {
    loss_started_at: Option<Instant>,
    last_fir_at: Option<Instant>,
}

impl VideoLossRecoveryGate {
    /// 0.25 s (`0x205fddc70: fmov d1, #0.25000000; fcmp d0, d1; b.le`)
    pub const LOSS_WAIT_BEFORE_FIR: Duration = Duration::from_millis(250);
    /// 0.5 s (`0x2060e7328: fmov d0, #0.50000000; b.mi`)
    pub const FIR_REPEAT_THROTTLE: Duration = Duration::from_millis(500);

    pub fn allow_fir_after_loss(&mut self, now: Instant) -> bool {
        let start = *self.loss_started_at.get_or_insert(now);
        let waited = now.checked_duration_since(start)
            .is_some_and(|d| d > Self::LOSS_WAIT_BEFORE_FIR);
        if !waited {
            return false;
        }
        if let Some(last) = self.last_fir_at {
            if !now.checked_duration_since(last).is_some_and(|d| d >= Self::FIR_REPEAT_THROTTLE) {
                return false;
            }
        }
        self.last_fir_at = Some(now);
        true
    }

    pub fn reset(&mut self) {
        self.loss_started_at = None;
    }
}
