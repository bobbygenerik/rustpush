use rustpush::ltr_ack::{encode_psfb_loss, encode_psfb_ltr_ack};

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
