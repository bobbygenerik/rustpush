#![allow(dead_code)]

#[path = "../src/video_decode_progress.rs"]
mod video_decode_progress;

use video_decode_progress::VideoDecodeProgress;

#[test]
fn ordinary_loss_requests_ltr_refresh_type_2_after_prior_output() {
    let progress = VideoDecodeProgress::default();
    progress.record_output_kind(true, true);
    progress.rearm();
    assert_eq!(progress.random_access_request_type(true), 2);
}

#[test]
fn decoder_reset_or_failure_requests_keyframe_fir_type_0_not_ltr_refresh() {
    let progress = VideoDecodeProgress::default();
    progress.record_output_kind(true, true);

    // Decoder reset / failure sets needs_intraframe -> must request keyframe (fir_type = 0).
    progress.rearm_intraframe();
    assert_eq!(progress.random_access_request_type(true), 0);

    // Cold decoder before any output also requests fir_type = 0 once reset/failure has armed needs_intraframe.
    let cold = VideoDecodeProgress::default();
    cold.rearm_intraframe();
    assert_eq!(cold.random_access_request_type(true), 0);

    // LTR-only refresh output while needs_intraframe is set cannot clear it -> still fir_type = 0.
    progress.record_output_kind(false, true);
    assert_eq!(progress.random_access_request_type(true), 0);

    // Real keyframe output clears needs_intraframe, so subsequent ordinary loss requests fir_type = 2.
    progress.record_output_kind(true, true);
    progress.rearm();
    assert_eq!(progress.random_access_request_type(true), 2);
}
