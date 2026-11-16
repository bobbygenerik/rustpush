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
