use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Decoder output, rather than receipt of compressed packets, proves recovery.
#[derive(Default)]
pub(super) struct VideoDecodeProgress {
    state: Mutex<(Option<Instant>, bool)>,
    // Distinct from recency: loss rearms recovery but is not stream startup.
    has_output: AtomicBool,
}

impl VideoDecodeProgress {
    pub(super) fn record_output(&self, keyframe: bool) {
        self.has_output.store(true, Ordering::Relaxed);
        let mut state = self.state.lock().unwrap();
        // Concealment output from an old damaged chain cannot finish recovery.
        if state.1 && !keyframe { return; }
        state.1 = false;
        state.0 = Some(Instant::now());
    }

    pub(super) fn is_recent(&self) -> bool {
        self.state.lock().unwrap().0
            .map(|at| at.elapsed() < Duration::from_secs(2))
            .unwrap_or(false)
    }

    pub(super) fn rearm(&self) {
        *self.state.lock().unwrap() = (None, true);
    }

    pub(super) fn request_type(&self, requested: u32, one_to_one: bool) -> u32 {
        // iOS27 VCVideoStream coerces requests to type4 while
        // _lastDecodedVideoFrameTime is zero. Our output state is per session,
        // so apply this only to 1:1 startup; group/stream resets need per-stream
        // decoder ownership before they can use the same native rule.
        if one_to_one && !self.has_output.load(Ordering::Relaxed) { 4 } else { requested }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_receipt_without_output_does_not_complete_recovery() {
        assert!(!VideoDecodeProgress::default().is_recent());
    }

    #[test]
    fn output_completes_recovery_until_it_stalls() {
        let progress = VideoDecodeProgress::default();
        progress.record_output(true);
        assert!(progress.is_recent());
        progress.state.lock().unwrap().0 = Some(Instant::now() - Duration::from_secs(3));
        assert!(!progress.is_recent());
    }

    #[test]
    fn loss_rearms_until_new_output_arrives() {
        let progress = VideoDecodeProgress::default();
        progress.record_output(true);
        progress.rearm();
        assert!(!progress.is_recent());
        progress.record_output(false);
        assert!(!progress.is_recent());
        progress.record_output(true);
        assert!(progress.is_recent());
    }

    #[test]
    fn one_to_one_startup_requests_native_type_four() {
        let progress = VideoDecodeProgress::default();
        assert_eq!(progress.request_type(2, true), 4);
        progress.record_output(true);
        assert_eq!(progress.request_type(2, true), 2);
    }

    #[test]
    fn rearmed_recovery_keeps_requested_type_after_output() {
        let progress = VideoDecodeProgress::default();
        progress.record_output(true);
        progress.rearm();
        assert!(!progress.is_recent());
        assert_eq!(progress.request_type(2, true), 2);
    }

    #[test]
    fn concealed_output_ends_startup_without_completing_recovery() {
        let progress = VideoDecodeProgress::default();
        progress.rearm();
        progress.record_output(false);
        assert!(!progress.is_recent());
        assert_eq!(progress.request_type(2, true), 2);
    }

    #[test]
    fn group_requests_preserve_existing_type() {
        assert_eq!(VideoDecodeProgress::default().request_type(2, false), 2);
    }
}
