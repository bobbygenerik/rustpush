use std::time::{Duration, Instant};

use rustpush::ltr_ack::{
    encode_psfb_loss, encode_psfb_ltr_ack, should_send_ltr_ack, VideoLossRecoveryGate,
};

/// Golden-byte test for PSFB AFB Type 5 (LTR ACK) derived by hand from
/// `__RTCPAddPSFBAlfbPacket` (`0x205f6a9ec`) & `__RTCPFinalizePacket` (`0x205f6a954`).
#[test]
fn psfb_type5_ltr_ack_golden_bytes() {
    let bytes = encode_psfb_ltr_ack(0x1122_3344, 0x5566_7788, 0x99aa_bbcc).unwrap();
    let expected: [u8; 20] = [
        0x8f, 0xce, 0x00, 0x04, // V=2, P=0, FMT=15, PT=206, length=4 (0x205f6aa14..24, 0x205f6a9cc..dc)
        0x11, 0x22, 0x33, 0x44, // localSSRC BE (0x205f6aa3c..40)
        0x55, 0x66, 0x77, 0x88, // remoteSSRC BE (0x205f6aa44..4c)
        0x00, 0x00, 0x00, 0x05, // AFB subtype 5 BE (0x205f6aa38..54)
        0x99, 0xaa, 0xbb, 0xcc, // ltrTimestamp BE (0x205f6aab4..c8)
    ];
    assert_eq!(bytes.as_slice(), &expected);
}

/// Golden-byte test for PSFB AFB Type 6 (FEC / Loss Feedback) derived by hand from
/// `__RTCPAddPSFBAlfbPacket` (`0x205f6a9ec`) & `__RTCPFinalizePacket` (`0x205f6a954`).
#[test]
fn psfb_type6_loss_golden_bytes() {
    let bytes = encode_psfb_loss(0x1122_3344, 0x5566_7788, 0xdead_beef, 0x0102, 12, 3).unwrap();
    let expected: [u8; 24] = [
        0x8f, 0xce, 0x00, 0x05, // V=2, P=0, FMT=15, PT=206, length=5 (0x205f6aa14..24, 0x205f6a9cc..dc)
        0x11, 0x22, 0x33, 0x44, // localSSRC BE (0x205f6aa3c..40)
        0x55, 0x66, 0x77, 0x88, // remoteSSRC BE (0x205f6aa44..4c)
        0x00, 0x00, 0x00, 0x06, // AFB subtype 6 BE (0x205f6aa38..54)
        0xde, 0xad, 0xbe, 0xef, // timestamp BE (0x205f6aa68..8c)
        0x01, 0x02,             // receivedPackets BE (0x205f6aa90..98)
        0x0c,                   // frameSize / sizeInPackets (0x205f6aa9c..a0)
        0x03,                   // lostPackets (0x205f6aaa4..b0)
    ];
    assert_eq!(bytes.as_slice(), &expected);
}

/// Admission rules from `_VideoPacketBuffer_GetNextFrame` (`0x205fdd47c..49c`) and
/// `__VideoReceiver_DecoderCallback` (`0x205fefa88..fcb4`).
#[test]
fn ltr_ack_admission_and_withhold_rules() {
    // 1:1 + bit0 of ltr_bits set + nonzero ltr_timestamp + decoded ok -> Some(token)
    assert_eq!(should_send_ltr_ack(true, 0x1, Some(0x1234_5678), true), Some(0x1234_5678));
    // Withheld when decode failed / dropped (`0x205fefaa4`, `0x205fefa90`)
    assert_eq!(should_send_ltr_ack(true, 0x1, Some(0x1234_5678), false), None);
    // Withheld when ltr_timestamp == 0 (`0x205fefcb4`)
    assert_eq!(should_send_ltr_ack(true, 0x1, Some(0), true), None);
    assert_eq!(should_send_ltr_ack(true, 0x1, None, true), None);
    // Withheld when ltr_bits bit0 is clear (`0x205fdd488`)
    assert_eq!(should_send_ltr_ack(true, 0x2, Some(0x1234_5678), true), None);
    // Withheld when not 1:1
    assert_eq!(should_send_ltr_ack(false, 0x1, Some(0x1234_5678), true), None);
}

/// Loss recovery wait before FIR (`_VideoPacketBuffer_GetNextFrame` `0x205fddc68..0x205fddc78`:
/// `(now - last_refresh_req) > 0.25` s) and FIR repeat throttle (`0x2060e7328`: `0.5` s).
#[test]
fn loss_recovery_waits_250ms_before_fir_then_throttles_500ms() {
    let mut gate = VideoLossRecoveryGate::default();
    let t0 = Instant::now();
    // First loss event arms the 250 ms wait (NACK runs first; FIR withheld)
    assert!(!gate.allow_fir_after_loss(t0));
    // At exactly 250 ms (`b.le` at 0x205fddc78), still withheld
    assert!(!gate.allow_fir_after_loss(t0 + Duration::from_millis(250)));
    // Strictly > 250 ms -> FIR admitted
    assert!(gate.allow_fir_after_loss(t0 + Duration::from_millis(251)));
    // Immediately or at 499 ms after FIR -> throttled by 500 ms (`0x2060e7328`)
    assert!(!gate.allow_fir_after_loss(t0 + Duration::from_millis(251) + Duration::from_millis(499)));
    // At >= 500 ms -> admitted again
    assert!(gate.allow_fir_after_loss(t0 + Duration::from_millis(251) + Duration::from_millis(500)));
    // Reset on recovery restarts the 250 ms wait on the next loss
    gate.reset();
    let t1 = t0 + Duration::from_secs(2);
    assert!(!gate.allow_fir_after_loss(t1));
    assert!(gate.allow_fir_after_loss(t1 + Duration::from_millis(251)));
}
