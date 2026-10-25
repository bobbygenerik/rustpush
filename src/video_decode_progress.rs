use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Decoder output, rather than receipt of compressed packets, proves recovery.
#[derive(Default)]
pub(super) struct VideoDecodeProgress {
    state: Mutex<DecodeState>,
}

#[derive(Default)]
struct DecodeState {
    last_healthy_output: Option<Instant>,
    waiting_for_random_access: bool,
    // Distinct from recency: loss rearms recovery but is not stream startup.
    has_output: bool,
    loss_recovery_started: Option<Instant>,
}

impl VideoDecodeProgress {
    pub(super) fn record_output(&self, keyframe: bool) {
        let mut state = self.state.lock().unwrap();
        state.record_output_at(keyframe, Instant::now());
    }

    pub(super) fn is_recent(&self) -> bool {
        self.state.lock().unwrap().last_healthy_output
            .map(|at| at.elapsed() < Duration::from_secs(2))
            .unwrap_or(false)
    }

    pub(super) fn rearm(&self) {
        let mut state = self.state.lock().unwrap();
        state.rearm_at(Instant::now());
    }

    pub(super) fn request_type(&self, requested: u32, one_to_one: bool) -> u32 {
        let state = self.state.lock().unwrap();
        state.request_type_at(requested, one_to_one, Instant::now())
    }

    pub(super) fn random_access_request_type(&self, one_to_one: bool) -> u32 {
        // iOS27 distinguishes ordinary refresh (type0) from LTR repair (type2).
        // Our 1:1 loss gate requires random access, but no decoded LTR anchor
        // is acknowledged/tracked end to end. Request the ordinary mode now;
        // waiting 15 seconds on type2 cannot satisfy that local gate reliably.
        // See FACETIME_LTR_ACK_2026-10-24.md for the native callback/wire trace.
        // Preserve the separate group path until its ownership is mapped.
        self.request_type(if one_to_one { 0 } else { 2 }, one_to_one)
    }
}

impl DecodeState {
    fn record_output_at(&mut self, keyframe: bool, now: Instant) {
        self.has_output = true;
        // Concealment output from an old damaged chain cannot finish recovery.
        if self.waiting_for_random_access && !keyframe { return; }
        self.waiting_for_random_access = false;
        self.loss_recovery_started = None;
        self.last_healthy_output = Some(now);
    }

    fn rearm_at(&mut self, now: Instant) {
        // iOS27 HandleStallStart preserves an active stall's start and requires
        // prior output. Repeated loss must not postpone the LTR fallback.
        if self.has_output && self.loss_recovery_started.is_none() {
            self.loss_recovery_started = Some(now);
        }
        self.last_healthy_output = None;
        self.waiting_for_random_access = true;
    }

    fn request_type_at(&self, requested: u32, one_to_one: bool, now: Instant) -> u32 {
        // iOS27 VCVideoStream coerces requests to type4 while
        // _lastDecodedVideoFrameTime is zero. Our output state is per session,
        // so apply this only to 1:1 startup; group/stream resets need per-stream
        // decoder ownership before they can use the same native rule.
        if one_to_one && !self.has_output { return 4; }
        // SendFIR stops selecting LTR when an active stall exceeds 15 seconds;
        // the ordinary reliable-control branch then supplies type0. Restrict
        // this adapter to 1:1 loss recovery until group decoder ownership and
        // the separate no-display/type3 path are mapped.
        if one_to_one && requested == 2
            && self.loss_recovery_started
                .and_then(|start| now.checked_duration_since(start))
                .is_some_and(|elapsed| elapsed > Duration::from_secs(15)) {
            return 0;
        }
        requested
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_access_recovery_requests_ordinary_refresh_immediately() {
        let progress = VideoDecodeProgress::default();
        progress.record_output(true);
        progress.rearm();
        assert_eq!(progress.random_access_request_type(true), 0);
        assert!(!progress.is_recent());
    }

    #[test]
    fn random_access_retries_do_not_switch_back_to_ltr() {
        let progress = VideoDecodeProgress::default();
        progress.record_output(true);
        progress.rearm();
        progress.record_output(false);
        for _ in 0..3 {
            assert_eq!(progress.random_access_request_type(true), 0);
        }
        assert!(!progress.is_recent());
    }

    #[test]
    fn random_access_requests_preserve_startup_and_group_behavior() {
        let progress = VideoDecodeProgress::default();
        assert_eq!(progress.random_access_request_type(true), 4);
        assert_eq!(progress.random_access_request_type(false), 2);
        progress.record_output(true);
        progress.rearm();
        assert_eq!(progress.random_access_request_type(false), 2);
    }

    #[test]
    fn packet_receipt_without_output_does_not_complete_recovery() {
        assert!(!VideoDecodeProgress::default().is_recent());
    }

    #[test]
    fn output_completes_recovery_until_it_stalls() {
        let progress = VideoDecodeProgress::default();
        progress.record_output(true);
        assert!(progress.is_recent());
        progress.state.lock().unwrap().last_healthy_output = Some(Instant::now() - Duration::from_secs(3));
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

    #[test]
    fn one_to_one_loss_escalates_after_native_ltr_window() {
        let progress = VideoDecodeProgress::default();
        progress.record_output(true);
        progress.rearm();
        progress.state.lock().unwrap().loss_recovery_started =
            Some(Instant::now() - Duration::from_millis(15_050));
        assert_eq!(progress.request_type(2, true), 0);
    }

    #[test]
    fn ltr_timeout_is_strictly_greater_than_fifteen_seconds() {
        let now = Instant::now();
        let mut state = DecodeState::default();
        state.record_output_at(true, now);
        state.rearm_at(now);
        for ms in [0, 499, 500, 14999, 15000] {
            assert_eq!(state.request_type_at(2, true, now + Duration::from_millis(ms)), 2);
        }
        assert_eq!(state.request_type_at(2, true, now + Duration::from_millis(15001)), 0);
    }

    #[test]
    fn repeated_loss_does_not_restart_active_recovery_clock() {
        let now = Instant::now();
        let mut state = DecodeState::default();
        state.record_output_at(true, now);
        for seconds in 0..=16 {
            state.rearm_at(now + Duration::from_secs(seconds));
        }
        assert_eq!(state.loss_recovery_started, Some(now));
        assert_eq!(state.request_type_at(2, true, now + Duration::from_secs(16)), 0);
    }

    #[test]
    fn recovered_random_access_starts_new_loss_with_a_fresh_window() {
        let now = Instant::now();
        let mut state = DecodeState::default();
        state.record_output_at(true, now);
        state.rearm_at(now);
        assert_eq!(state.request_type_at(2, true, now + Duration::from_secs(16)), 0);
        state.record_output_at(true, now + Duration::from_secs(17));
        assert_eq!(state.loss_recovery_started, None);
        state.rearm_at(now + Duration::from_secs(18));
        assert_eq!(state.request_type_at(2, true, now + Duration::from_secs(33)), 2);
        assert_eq!(state.request_type_at(2, true, now + Duration::from_secs(34)), 0);
    }

    #[test]
    fn concealed_output_does_not_cancel_active_loss_escalation() {
        let now = Instant::now();
        let mut state = DecodeState::default();
        state.record_output_at(true, now);
        state.rearm_at(now);
        state.record_output_at(false, now + Duration::from_secs(14));
        assert_eq!(state.loss_recovery_started, Some(now));
        assert!(state.waiting_for_random_access);
        assert_eq!(state.request_type_at(2, true, now + Duration::from_secs(16)), 0);
    }

    #[test]
    fn fallback_does_not_change_group_or_other_request_types() {
        let now = Instant::now();
        let mut state = DecodeState::default();
        state.record_output_at(true, now);
        state.rearm_at(now);
        let later = now + Duration::from_secs(16);
        assert_eq!(state.request_type_at(2, false, later), 2);
        for requested in [0, 1, 3, 4] {
            assert_eq!(state.request_type_at(requested, true, later), requested);
        }
    }

    #[test]
    fn startup_loss_does_not_create_a_post_output_stall() {
        let now = Instant::now();
        let mut state = DecodeState::default();
        state.rearm_at(now);
        assert_eq!(state.loss_recovery_started, None);
        assert_eq!(state.request_type_at(2, true, now + Duration::from_secs(60)), 4);
    }
}
