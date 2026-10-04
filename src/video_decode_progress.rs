use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Decoder output, rather than receipt of compressed packets, proves recovery.
#[derive(Default)]
pub(super) struct VideoDecodeProgress {
    state: Mutex<(Option<Instant>, bool)>,
}

impl VideoDecodeProgress {
    pub(super) fn record_output(&self, keyframe: bool) {
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
}
