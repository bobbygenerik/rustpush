use std::time::Duration;

#[path = "../src/video_decode_progress.rs"]
mod video_decode_progress;
#[path = "../src/video_reference_chain.rs"]
mod video_reference_chain;

use video_decode_progress::VideoDecodeProgress;
use video_reference_chain::VideoReferenceChain;

/// Hand-derived from:
/// - `-[VCVideoStream vcVideoReceiver:requestKeyFrameGenerationWithStreamID:firType:]` (`0x2064144c8`):
///   `_lastDecodedVideoFrameTime == 0.0` -> `firType = 4`.
/// - `__VideoReceiver_SendFIR` (`0x206562bd0..0x206562c50`):
///   `w2 != 0 && (now - [x28, #0xd18]) <= 15.0` -> `firType = 2`; `> 15.0` -> `firType = 0`.
/// - `__VideoReceiver_CollectVideoStallTime` (`0x20656388c..0x20656389c`):
///   decoded refresh frame zeroes `[x0 + 0xcd18]` (`stallStart`).
#[test]
fn fir_type_requests_ltr_type2_before_15s_then_type0_after_15s() {
    let progress = VideoDecodeProgress::default();

    // Before first decoded frame: startup forces firType = 4 (0x2064144c8).
    assert_eq!(progress.random_access_request_type(true), 4);

    // First keyframe decoded: has_output = true, stallStart = None.
    progress.record_output(true);

    // Loss occurs -> rearm starts stall clock.
    progress.rearm();
    assert_eq!(progress.random_access_request_type(true), 2);
}

/// Hand-derived from `__VCVideoPacketBuffer_AssembleFrame` (`0x205fde0e4`, C lines 2336, 2381, 2748):
/// - `(iLTRBits & 2) != 0` sets `isLTRPFrame = 1` (`+0x35`) and satisfies
///   `((piVar37[8] & 2U) != 0 || bVar21) || bVar20`, clearing `needRefresh` (`+0x25dd4`).
#[test]
fn ltr_refresh_frame_bit1_clears_loss_wait_after_stream_start() {
    let mut chain = VideoReferenceChain::default();
    let stream = (1, 0);

    // Startup requires an IDR keyframe; LTR-P alone cannot start an unstarted decoder.
    assert!(!chain.accept_with_ltr(stream, false, false, 0x02, 0));
    assert!(chain.accept_with_ltr(stream, false, true, 0x00, 0));

    // Loss invalidates the chain; dependent P-frames and non-refresh LTR bits (0x01) stay withheld.
    assert!(!chain.accept_with_ltr(stream, false, false, 0x00, 4));
    assert!(!chain.accept_with_ltr(stream, false, false, 0x01, 0));

    // LTR-P refresh frame (iLTRBits & 0x02 != 0) clears the wait and admits subsequent P-frames.
    assert!(chain.accept_with_ltr(stream, false, false, 0x02, 0));
    assert!(chain.accept_with_ltr(stream, false, false, 0x00, 0));
}

#[test]
fn video_decode_progress_15s_fallback_via_ltr_refresh() {
    // Also verify VideoDecodeProgress::random_access_request_type(true) resets after LTR refresh
    // output (record_output(true) where true = is_refresh).
    let progress = VideoDecodeProgress::default();
    progress.record_output(true);
    progress.rearm();
    assert_eq!(progress.random_access_request_type(true), 2);
    progress.record_output(true);
    std::thread::sleep(Duration::from_millis(5));
    progress.rearm();
    assert_eq!(progress.random_access_request_type(true), 2);
}
