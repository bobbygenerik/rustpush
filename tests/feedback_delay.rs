use rustpush::feedback_delay::{ntp_send_timestamp, pack_queuing_delay};

#[test]
fn queuing_delay_matches_rtp_set_afrc_processing_delay() {
    // _RTPSetAFRCProcessingDelay (0x205fa57f0..0x205fa57f4): enqueue == 0.0 -> 0
    assert_eq!(pack_queuing_delay(0.0, 100.0), 0);

    // _RTPSetAFRCProcessingDelay (0x205fa57e0..0x205fa57ec):
    // 0.0004 s * 1024 = 0.4096 -> fcvtzu = 0 -> clamped to >= 1
    assert_eq!(pack_queuing_delay(10.0, 10.0004), 1);

    // 0.0200 s * 1024 = 20.48 -> fcvtzu = 20
    assert_eq!(pack_queuing_delay(10.0, 10.0200), 20);

    // 100.0 s * 1024 = 102400 -> clamped to wire u16::MAX (0xffff)
    assert_eq!(pack_queuing_delay(10.0, 110.0), 0xffff);
}

#[test]
fn send_timestamp_matches_micro_to_ntp_middle32_shift6_and_wraps() {
    // _TimeZeroInit (0x205f90238..0x205f90258) + _MicroToNTP (0x205f90154)
    // + _NTPToMiddle32 (0x205f902c8) + _RTPSetAFRCProcessingDelay (0x205fa580c):
    // NTP offset = 2208988800.0; 1700000000 + 2208988800 = 3908988800 = 61077950 * 64.
    // Case 1: +1.25 s -> 1.25 * 1024 = 1280 (0x0500)
    assert_eq!(ntp_send_timestamp(1_700_000_001.25), 1280);

    // Case 2 (before 64 s wrap): +63 + 1023/1024 s -> 63*1024 + 1023 = 65535 (0xffff)
    assert_eq!(ntp_send_timestamp(1_700_000_063.999_023_437_5), 0xffff);

    // Case 3 (after 64 s wrap): +64.5 s -> 0.5 * 1024 = 512 (0x0200)
    assert_eq!(ntp_send_timestamp(1_700_000_064.5), 512);
}
