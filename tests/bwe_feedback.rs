use rustpush::avconference::{
    is_probing_slice_payload, pack_bandwidth_estimate_u16, VideoBweProbeReceiver, REPORT_REAL_BWE,
};

fn hevc_fu_payload(start: bool, len: usize) -> Vec<u8> {
    let mut p = vec![0u8; len.max(4)];
    // NAL header type 49 (FU): (0x62 & 0x7e) == 0x62
    p[0] = 0x62;
    p[1] = 0x01;
    // FU header: S bit (0x80) | fu_type = 19 (IDR_W_RADL)
    p[2] = if start { 0x93 } else { 0x13 };
    p[3] = 0x80;
    p
}

fn hevc_single_p_payload(len: usize) -> Vec<u8> {
    let mut p = vec![0u8; len.max(4)];
    // NAL header type 1 (TRAIL_R): (1 << 1) = 0x02
    p[0] = 0x02;
    p[1] = 0x01;
    p[2] = 0xe0;
    p[3] = 0x00;
    p
}

#[test]
fn report_real_bwe_flag_is_enabled() {
    assert!(REPORT_REAL_BWE);
}

#[test]
fn no_burst_returns_zero_and_not_stable() {
    let mut rx = VideoBweProbeReceiver::new();
    let snap = rx.snapshot(10.0);
    assert_eq!(snap.packed_u16, 0);
    assert_eq!(snap.raw_kbps, 0);
    assert!(!snap.stable);

    // Single-packet P-frames do not pass `_ReadFrameSliceHeader` `{2, 3, 6}` and leave BWE 0.
    let p_frame = hevc_single_p_payload(400);
    assert!(rx.observe_packet(100, &p_frame, 90_000, true, 412, 10.5).is_none());
    let snap_after = rx.snapshot(10.5);
    assert_eq!(snap_after.packed_u16, 0);
    assert!(!snap_after.stable);
}

/// Exact derivation from Apple `CalculateBandwidthEstimation` (`0x2063ba414`),
/// `UpdateBandwidthEstimation` (`0x2063b9ecc`), and `ComputeBWEstimation` (`0x205ed47b4`):
/// - Packet 1 (`1200 B` at `t = 10.000 s`) opens sequence (`reference_packet_arrival_time = 10.000`,
///   Packet 1 stored in `early_bytes_in_probing_sequence`, `total_bytes_in_probing_sequence = 0`).
/// - Packet 2 (`1200 B` at `t = 10.010 s`) adds `1200 B`.
/// - Packet 3 (`1200 B` at `t = 10.020 s`, `M = 1`) adds `1200 B` -> `totalBytes = 2400 B` (`> 250 B`),
///   `duration = 10.020 - 10.000 = 0.020 s` (`>= 0.008 s`).
/// - `sample_bps = (2400 * 8) / 0.020 = 960_000.0 bps` (`<= 6_500_000.0` cap).
/// - Burst 1 (`estimated_bandwidth == 0.0`): `estimated_bandwidth = 960_000.0`,
///   `bandwidth_estimation_state = 0` (`isBWEStable = false`).
///   Quantized: `(960_000 / 1000) & 0x7ffff8 = 960`, bit 0 = `0` -> wire `960`.
/// - Burst 2 (`>= 2.0 s` later at `t = 12.000..12.020 s`, same `2400 B / 0.020 s`):
///   `UpdateBandwidthEstimation` runs EWMA -> `estimated_bandwidth = 960_000.0`,
///   `bandwidth_estimation_state = 1` (`isBWEStable = true`), bit 0 = `1` -> wire `961`.
#[test]
fn one_qualifying_burst_matches_exact_apple_formula() {
    let mut rx = VideoBweProbeReceiver::new();
    let fu_start = hevc_fu_payload(true, 1188);
    let fu_cont = hevc_fu_payload(false, 1188);
    assert!(is_probing_slice_payload(100, &fu_start));

    assert!(rx.observe_packet(100, &fu_start, 90_000, false, 1200, 10.000).is_none());
    assert!(rx.observe_packet(100, &fu_cont, 90_000, false, 1200, 10.010).is_none());
    let burst1 = rx
        .observe_packet(100, &fu_cont, 90_000, true, 1200, 10.020)
        .expect("first burst accepted");

    assert_eq!(burst1.bytes, 2400);
    assert!((burst1.span_ms - 20.0).abs() < 1e-6);
    assert_eq!(burst1.estimate_kbps, 960);
    assert_eq!(burst1.state, 0);

    let snap1 = rx.snapshot(10.020);
    assert_eq!(snap1.raw_kbps, 960);
    assert!(!snap1.stable);
    assert_eq!(snap1.packed_u16, 960);

    // Within 2.0 s cooldown (`11.000 - 10.020 = 0.980 < 2.0`): ignored.
    assert!(rx.observe_packet(100, &fu_start, 93_000, false, 1200, 11.000).is_none());
    assert!(rx.observe_packet(100, &fu_cont, 93_000, true, 1200, 11.020).is_none());

    // After 2.0 s cooldown (`12.020 - 10.020 == 2.000`): transitions to state 1 (stable).
    assert!(rx.observe_packet(100, &fu_start, 96_000, false, 1200, 12.020).is_none());
    assert!(rx.observe_packet(100, &fu_cont, 96_000, false, 1200, 12.030).is_none());
    let burst2 = rx
        .observe_packet(100, &fu_cont, 96_000, true, 1200, 12.040)
        .expect("second burst accepted");
    assert_eq!(burst2.state, 1);

    let snap2 = rx.snapshot(12.040);
    assert_eq!(snap2.raw_kbps, 960);
    assert!(snap2.stable);
    assert_eq!(snap2.packed_u16, 961);
}

#[test]
fn small_200b_burst_and_5ms_burst_are_rejected() {
    let mut rx = VideoBweProbeReceiver::new();
    let fu_small = hevc_fu_payload(true, 88);

    // 200 B burst across 2 packets (`total_bytes = 100 <= 250` on packet 2): rejected cleanly.
    assert!(rx.observe_packet(100, &fu_small, 10_000, false, 100, 10.000).is_none());
    assert!(rx.observe_packet(100, &fu_small, 10_000, true, 100, 10.020).is_none());
    assert_eq!(rx.snapshot(10.020).packed_u16, 0);

    // Establish steady 960 kbps estimate (two bursts spaced >= 2.0 s) cleanly after cooldown.
    let fu_big = hevc_fu_payload(true, 1188);
    assert!(rx.observe_packet(100, &fu_big, 20_000, false, 1200, 13.000).is_none());
    assert!(rx.observe_packet(100, &fu_big, 20_000, false, 1200, 13.010).is_none());
    assert!(rx.observe_packet(100, &fu_big, 20_000, true, 1200, 13.020).is_some());
    assert!(rx.observe_packet(100, &fu_big, 25_000, false, 1200, 15.050).is_none());
    assert!(rx.observe_packet(100, &fu_big, 25_000, false, 1200, 15.060).is_none());
    assert!(rx.observe_packet(100, &fu_big, 25_000, true, 1200, 15.070).is_some());
    assert_eq!(rx.snapshot(15.070).packed_u16, 961);

    // 200 B burst at `t = 17.500`: rejected, estimate remains `961`.
    assert!(rx.observe_packet(100, &fu_small, 30_000, false, 100, 17.500).is_none());
    assert!(rx.observe_packet(100, &fu_small, 30_000, true, 100, 17.520).is_none());
    assert_eq!(rx.snapshot(17.520).packed_u16, 961);

    // 5 ms burst (< 8 ms `_minEstimationWindowDuration`, raw 3840 kbps) at `t = 20.000`:
    // `over_range_count` becomes 1 (< 3), so `CalculateBandwidthEstimation` (`0x2063ba754`)
    // passes 0.0 to `UpdateBandwidthEstimation` -> leaves `estimated_bandwidth` unchanged at
    // 960 kbps (does not climb toward 3840 kbps) while setting `state = 6` (`isBWEStable = 0`, wire `960`).
    assert!(rx.observe_packet(100, &fu_big, 40_000, false, 1200, 20.000).is_none());
    assert!(rx.observe_packet(100, &fu_big, 40_000, false, 1200, 20.002).is_none());
    assert!(rx.observe_packet(100, &fu_big, 40_000, true, 1200, 20.005).is_none());
    let after_5ms = rx.snapshot(20.005);
    assert_eq!(after_5ms.raw_kbps, 960);
    assert_eq!(after_5ms.state, 6);
    assert!(!after_5ms.stable);
    assert_eq!(after_5ms.packed_u16, 960);
}

#[test]
fn expires_to_zero_after_30_seconds() {
    let mut rx = VideoBweProbeReceiver::new();
    let fu_big = hevc_fu_payload(true, 1188);

    assert!(rx.observe_packet(100, &fu_big, 50_000, false, 1200, 10.000).is_none());
    assert!(rx.observe_packet(100, &fu_big, 50_000, false, 1200, 10.010).is_none());
    assert!(rx.observe_packet(100, &fu_big, 50_000, true, 1200, 10.020).is_some());

    // Before 30.0 s (`39.999 - 10.020 = 29.979 s`): still valid.
    assert_eq!(rx.snapshot(39.999).packed_u16, 960);

    // At/after 30.0 s (`40.020 - 10.020 = 30.000 s`): expires to 0 per `0x205ed3910..930`.
    let expired = rx.snapshot(40.020);
    assert_eq!(expired.packed_u16, 0);
    assert!(!expired.stable);
}

/// `_VCRateControlSetFeedbackControlInfo` (`0x205eba31c..324`) packs quantized kbps with `bit 0 = isBWEStable`
/// via `strh w8, [sp, #0xa]`; `_VCMediaControlInfoFaceTimeAudio_SerializeBuffer` (`0x2064c1f48` at
/// `0x2064c2090..20a8`) clamps to `0xffff` and writes big-endian `u16`.
#[test]
fn packing_round_trips_against_quoted_apple_layout() {
    assert_eq!(pack_bandwidth_estimate_u16(0, false), 0);
    assert_eq!(pack_bandwidth_estimate_u16(0, true), 0);

    // 3845 kbps -> quantized multiple of 8 is `3845 & !7 = 3840`; stable sets bit 0 -> 3841 (`0x0f01`).
    let unstable_u16 = pack_bandwidth_estimate_u16(3845, false);
    assert_eq!(unstable_u16, 3840);
    assert_eq!(unstable_u16 & 0x07, 0);
    assert_eq!(unstable_u16 & !0x07, 3840);

    let stable_u16 = pack_bandwidth_estimate_u16(3845, true);
    assert_eq!(stable_u16, 3841);
    let be_bytes = stable_u16.to_be_bytes();
    assert_eq!(be_bytes, [0x0f, 0x01]);
    let decoded = u16::from_be_bytes(be_bytes);
    assert_eq!(decoded & 1, 1);
    assert_eq!(decoded & !7, 3840);

    // Values >= 65536 kbps clamp to 0xffff (`0x2064c2094..209c`).
    assert_eq!(pack_bandwidth_estimate_u16(70_000, true), u16::MAX);
}
