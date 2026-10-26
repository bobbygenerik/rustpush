use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use super::video_decode_progress::VideoDecodeProgress;
use super::video_reference_chain::VideoReferenceChain;
use super::video_no_display::VideoNoDisplay;
use std::time::Instant;

pub(super) type Stream = (u64, u32);

#[derive(Clone, Copy, Debug)]
pub(super) struct FrameReceipt {
    pub stream: Stream,
    pub timestamp: u32,
    configuration: bool,
    keyframe: bool,
}

#[derive(Default)]
struct StreamState {
    incarnation: u64,
    progress: VideoDecodeProgress,
    damaged_through: u64,
    no_display: VideoNoDisplay,
    camera_source: Option<u8>,
    target: Option<(u32, u32)>,
}

#[derive(Default)]
struct State {
    next_receipt: u64,
    active_participants: Option<std::collections::HashSet<u64>>,
    pending: BTreeMap<u64, FrameReceipt>,
    streams: HashMap<Stream, StreamState>,
    chain: VideoReferenceChain,
}

/// App-local receipts bind decoder feedback to the actual input stream/frame.
/// No receipts or adapter events are sent to Apple. See
/// FACETIME_DECODE_OWNERSHIP_2026-10-25.md for native evidence and limits.
#[derive(Default)]
pub(super) struct VideoReceiveRecovery(Mutex<State>);

impl VideoReceiveRecovery {
    // Local memory bound, not an Apple queue-size claim. Normal output feedback
    // retires older inputs; absence of feedback cannot grow this ledger forever.
    const MAX_PENDING: usize = 256;

    pub(super) fn forget_participant(&self, participant: u64) {
        let mut state = self.0.lock().unwrap();
        state.pending.retain(|_, frame| frame.stream.0 != participant);
        state.streams.retain(|stream, _| stream.0 != participant);
        let mut retained: std::collections::HashSet<u64> = state.streams.keys().map(|stream| stream.0).collect();
        retained.remove(&participant);
        state.chain.retain_participants(&retained);
        // Preserve active membership and the receipt sequence. This is an AVC
        // incarnation change, not a synthetic leave/rejoin signaling message.
    }

    pub(super) fn retain_participants(&self, active: &std::collections::HashSet<u64>) {
        let mut state = self.0.lock().unwrap();
        state.pending.retain(|_, frame| active.contains(&frame.stream.0));
        state.streams.retain(|(participant, _), _| active.contains(participant));
        state.chain.retain_participants(active);
        state.active_participants = Some(active.clone());
        // Keep next_receipt monotonic: a participant can rejoin with the same
        // stream ID while Android still has callbacks from its old decoder.
    }

    pub(super) fn receive(&self, stream: Stream, timestamp: u32,
        configuration: bool, keyframe: bool, lost: u16) -> Option<u64> {
        let mut state = self.0.lock().unwrap();
        if state.active_participants.as_ref().is_some_and(|active| !active.contains(&stream.0)) {
            return None;
        }
        // Never reuse a receipt in this session, including at integer exhaustion.
        let receipt = state.next_receipt.checked_add(1).filter(|id| *id <= i64::MAX as u64)?;
        state.next_receipt = receipt;
        state.streams.entry(stream).or_insert_with(|| StreamState { incarnation: receipt, ..Default::default() });
        if lost > 0 {
            // prev_dropped describes loss before this reassembled frame. An
            // intact random-access frame at this receipt can still recover it.
            state.damage(stream, receipt - 1);
            state.chain.invalidate(stream);
        }
        if !state.chain.accept(stream, configuration, keyframe, lost) { return None; }
        if state.pending.len() == Self::MAX_PENDING { state.pending.pop_first(); }
        state.pending.insert(receipt, FrameReceipt { stream, timestamp, configuration, keyframe });
        Some(receipt)
    }

    pub(super) fn decoded(&self, receipt: u64) -> Option<FrameReceipt> {
        self.decoded_at(receipt, Instant::now())
    }

    fn decoded_at(&self, receipt: u64, now: Instant) -> Option<FrameReceipt> {
        let mut state = self.0.lock().unwrap();
        let frame = state.pending.remove(&receipt)?;
        let stream = state.streams.get_mut(&frame.stream)?;
        if frame.configuration || receipt <= stream.damaged_through { return None; }
        stream.progress.record_output(frame.keyframe);
        if !stream.progress.is_recent() { return None; }
        stream.no_display.progress(now);
        // Outputs may be reordered. Retire older receipts without accepting
        // them as later progress or allowing duplicate callbacks to heal loss.
        state.pending.retain(|id, item| item.stream != frame.stream || *id > receipt);
        Some(frame)
    }

    pub(super) fn dropped(&self, receipt: u64) -> bool {
        let mut state = self.0.lock().unwrap();
        let Some(frame) = state.pending.get(&receipt).copied() else { return false; };
        let through = if frame.configuration { receipt - 1 } else { receipt };
        state.damage(frame.stream, through);
        state.chain.invalidate(frame.stream);
        // A later keyframe can already be in the serial Dart/native queue.
        // Keep its receipt usable; late notification of an earlier drop must
        // not invalidate that newer repair or stall packets queued behind it.
        if state.pending.iter().any(|(id, item)| *id > through
            && item.stream == frame.stream && item.keyframe && !item.configuration) {
            state.chain.accept(frame.stream, false, true, 0);
        }
        true
    }

    pub(super) fn decoder_reset(&self, receipt: u64) -> bool {
        let mut state = self.0.lock().unwrap();
        let Some(frame) = state.pending.get(&receipt).copied() else { return false; };
        // This frame belongs to the new decoder. Invalidate earlier outputs,
        // while allowing this frame itself to establish a new reference.
        state.damage(frame.stream, receipt - 1);
        state.chain.invalidate(frame.stream);
        if frame.keyframe && !frame.configuration {
            state.chain.accept(frame.stream, false, true, 0);
        }
        true
    }

    pub(super) fn observe_target(&self, stream: Stream, target: Option<(u32, u32)>, source: Option<u8>) {
        let mut state = self.0.lock().unwrap();
        let Some(item) = state.streams.get_mut(&stream) else { return; };
        if target.is_some() && item.target != target {
            item.no_display.set_expected(false, Instant::now());
            item.target = target;
        }
        if let Some(source) = source {
            if item.camera_source != Some(source) { item.no_display.set_expected(false, Instant::now()); }
            item.camera_source = Some(source);
        }
    }

    // Only the mapped U1 camera stream is eligible. Group selection and screen
    // ownership are deliberately not inferred from an arbitrary stream ID.
    pub(super) fn poll_no_display(&self, expected_participant: Option<u64>) -> Vec<(Stream, u64, (u32, u32), u64)> {
        self.poll_no_display_at(expected_participant, Instant::now())
    }

    fn poll_no_display_at(&self, expected_participant: Option<u64>, now: Instant) -> Vec<(Stream, u64, (u32, u32), u64)> {
        let mut state = self.0.lock().unwrap();
        let mut due = Vec::new();
        for (&stream, item) in &mut state.streams {
            let expected = expected_participant == Some(stream.0) && stream.1 == 0
                && item.camera_source == Some(0) && item.target.is_some();
            item.no_display.set_expected(expected, now);
            if let Some(ticket) = item.no_display.poll(now) {
                due.push((stream, item.incarnation, item.target.unwrap(), ticket));
            }
        }
        due
    }

    pub(super) fn dispatch_no_display(&self, stream: Stream, incarnation: u64, target: (u32, u32), ticket: u64) -> Option<u32> {
        self.dispatch_no_display_at(stream, incarnation, target, ticket, Instant::now())
    }

    fn dispatch_no_display_at(&self, stream: Stream, incarnation: u64, target: (u32, u32), ticket: u64, now: Instant) -> Option<u32> {
        let mut state = self.0.lock().unwrap();
        let item = state.streams.get_mut(&stream)?;
        if item.incarnation != incarnation || item.target != Some(target)
            || item.camera_source != Some(0) || !item.no_display.dispatch(ticket, now) { return None; }
        // Native no-display reason0x1c is type3, with a separate type4 startup
        // override until this exact stream has produced an accepted output.
        Some(item.progress.request_type(3, true))
    }

    pub(super) fn fir_sent(&self, stream: Stream) {
        if let Some(item) = self.0.lock().unwrap().streams.get_mut(&stream) {
            item.no_display.fir_sent(Instant::now());
        }
    }

    pub(super) fn decoder_is_recent(&self, stream: Stream) -> bool {
        self.0.lock().unwrap().streams.get(&stream)
            .is_some_and(|s| s.progress.is_recent())
    }

    pub(super) fn request_type(&self, stream: Stream, one_to_one: bool) -> u32 {
        self.0.lock().unwrap().streams.get(&stream)
            .map(|state| state.progress.random_access_request_type(one_to_one))
            .unwrap_or_else(|| VideoDecodeProgress::default().random_access_request_type(one_to_one))
    }

    pub(super) fn stream_incarnation(&self, stream: Stream) -> Option<u64> {
        self.0.lock().unwrap().streams.get(&stream).map(|state| state.incarnation)
    }
}

impl State {
    fn damage(&mut self, stream: Stream, through: u64) {
        let entry = self.streams.entry(stream).or_default();
        if through < entry.damaged_through { return; }
        entry.damaged_through = through;
        entry.progress.rearm();
        self.pending.retain(|id, item| item.stream != stream || *id > through);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const A: Stream = (1, 1);
    const B: Stream = (2, 1);

    fn armed_at(now: Instant) -> (VideoReceiveRecovery, u64) {
        let r = VideoReceiveRecovery::default();
        let receipt = r.receive((1,0), 10, false, true, 0).unwrap();
        r.observe_target((1,0), Some((7,0)), Some(0));
        assert!(r.poll_no_display_at(Some(1), now).is_empty());
        (r, receipt)
    }

    #[test]
    fn no_display_repairs_silence_with_the_received_target_and_startup_mode() {
        let now = Instant::now(); let (r, _) = armed_at(now);
        let later = now + std::time::Duration::from_millis(5001);
        let (stream, incarnation, target, ticket) = r.poll_no_display_at(Some(1), later)[0];
        assert_eq!(target, (7,0));
        assert_eq!(r.dispatch_no_display_at(stream, incarnation, target, ticket, later), Some(4));
    }

    #[test]
    fn output_after_watchdog_poll_cancels_queued_repair_and_prior_output_uses_type3() {
        let now = Instant::now(); let (r, receipt) = armed_at(now);
        let later = now + std::time::Duration::from_millis(5001);
        let (stream, incarnation, target, ticket) = r.poll_no_display_at(Some(1), later)[0];
        assert!(r.decoded_at(receipt, later).is_some());
        assert_eq!(r.dispatch_no_display_at(stream, incarnation, target, ticket, later), None);
        let later = later + std::time::Duration::from_millis(5001);
        let (stream, incarnation, target, ticket) = r.poll_no_display_at(Some(1), later)[0];
        assert_eq!(r.dispatch_no_display_at(stream, incarnation, target, ticket, later), Some(3));
    }

    #[test]
    fn unknown_camera_source_and_group_stream_do_not_arm_no_display() {
        let now = Instant::now(); let r = VideoReceiveRecovery::default();
        for stream in [(1,0),(1,7)] { r.receive(stream, 1, false, true, 0); r.observe_target(stream, Some((7,0)), None); }
        r.observe_target((1,7), Some((7,0)), Some(0));
        r.poll_no_display_at(Some(1), now);
        assert!(r.poll_no_display_at(Some(1), now + std::time::Duration::from_secs(30)).is_empty());
        r.observe_target((1,0), Some((7,0)), Some(1));
        assert!(r.poll_no_display_at(Some(1), now + std::time::Duration::from_secs(40)).is_empty());
    }

    #[test]
    fn hidden_or_changed_target_retires_queued_no_display_work() {
        let now = Instant::now(); let (r, _) = armed_at(now);
        let later = now + std::time::Duration::from_millis(5001);
        let (stream, incarnation, target, ticket) = r.poll_no_display_at(Some(1), later)[0];
        r.poll_no_display_at(None, later);
        assert_eq!(r.dispatch_no_display_at(stream, incarnation, target, ticket, later), None);
        r.poll_no_display_at(Some(1), later);
        let later = later + std::time::Duration::from_millis(5001);
        let (_, _, _, ticket) = r.poll_no_display_at(Some(1), later)[0];
        r.observe_target((1,0), Some((8,0)), Some(0));
        assert_eq!(r.dispatch_no_display_at(stream, incarnation, target, ticket, later), None);
    }

    #[test]
    fn avc_incarnation_change_retires_receipts_without_changing_membership() {
        let r = VideoReceiveRecovery::default();
        r.retain_participants(&std::collections::HashSet::from([1,2]));
        let first = r.receive((1,0), 1, false, true, 0).unwrap();
        let other = r.receive((2,0), 1, false, true, 0).unwrap();
        let old_incarnation = r.stream_incarnation((1,0)).unwrap();
        r.forget_participant(1);
        assert!(r.decoded(first).is_none());
        assert!(!r.decoder_reset(first));
        assert!(r.decoded(other).is_some());
        let next = r.receive((1,0), 2, false, true, 0).unwrap();
        assert!(next > other);
        assert_ne!(r.stream_incarnation((1,0)).unwrap(), old_incarnation);
        assert!(r.decoded(next).is_some());
    }

    #[test]
    fn departed_participant_receipts_cannot_heal_a_rejoined_stream() {
        let r = VideoReceiveRecovery::default();
        let old = frame(&r, A, true);
        let incarnation = r.stream_incarnation(A).unwrap();
        let other = frame(&r, B, true);
        r.retain_participants(&std::collections::HashSet::from([2]));
        assert!(r.decoded(old).is_none());
        assert!(!r.dropped(old));
        assert!(!r.decoder_reset(old));
        assert!(r.decoded(other).is_some());
        // Even a late intact keyframe from the departed incarnation is rejected.
        assert!(r.receive(A, 124, false, true, 0).is_none());
        r.retain_participants(&std::collections::HashSet::from([1, 2]));
        assert!(r.receive(A, 124, false, false, 0).is_none());
        let new = frame(&r, A, true);
        assert!(new > old);
        assert_ne!(r.stream_incarnation(A), Some(incarnation));
        assert!(r.decoded(new).is_some());
    }

    #[test]
    fn ending_receiver_clears_health_and_pending_callbacks() {
        let r = VideoReceiveRecovery::default();
        let decoded = frame(&r, A, true);
        assert!(r.decoded(decoded).is_some());
        let queued = frame(&r, B, true);
        r.retain_participants(&std::collections::HashSet::new());
        assert!(!r.decoder_is_recent(A));
        assert!(r.decoded(queued).is_none());
        let state = r.0.lock().unwrap();
        assert!(state.streams.is_empty());
        assert!(state.pending.is_empty());
    }
    fn frame(r: &VideoReceiveRecovery, stream: Stream, key: bool) -> u64 {
        r.receive(stream, 123, false, key, 0).unwrap()
    }
    #[test]
    fn old_keyframe_cannot_complete_a_newer_loss() {
        let r = VideoReceiveRecovery::default();
        let old = frame(&r, A, true);
        assert!(r.receive(A, 124, false, false, 1).is_none());
        assert!(r.decoded(old).is_none());
        assert!(!r.decoder_is_recent(A));
        let repair = frame(&r, A, true);
        assert!(r.decoded(repair).is_some());
        assert!(r.decoder_is_recent(A));
    }
    #[test]
    fn local_drop_blocks_dependents_without_network_loss() {
        let r = VideoReceiveRecovery::default();
        let first = frame(&r, A, true); r.decoded(first).unwrap();
        let lost = frame(&r, A, false);
        assert!(r.dropped(lost));
        assert!(r.receive(A, 125, false, false, 0).is_none());
        assert_eq!(r.request_type(A, true), 0);
        assert!(!r.decoder_is_recent(A));
    }
    #[test]
    fn pending_newer_keyframe_survives_a_late_drop_report() {
        let r = VideoReceiveRecovery::default();
        let lost = frame(&r, A, true);
        let repair = frame(&r, A, true);
        assert!(r.dropped(lost));
        assert!(r.decoded(lost).is_none());
        assert!(r.decoded(repair).is_some());
        assert!(r.decoder_is_recent(A));
    }
    #[test]
    fn stream_progress_and_loss_are_isolated() {
        let r = VideoReceiveRecovery::default();
        let a = frame(&r, A, true); let b = frame(&r, B, true);
        r.decoded(a).unwrap(); r.decoded(b).unwrap();
        let lost = frame(&r, A, false); r.dropped(lost);
        assert!(!r.decoder_is_recent(A)); assert!(r.decoder_is_recent(B));
        assert_eq!(r.request_type(B, true), 0);
        assert_eq!(r.request_type((3, 1), true), 4);
    }
    #[test]
    fn configuration_cannot_count_as_video_output() {
        let r = VideoReceiveRecovery::default();
        let config = r.receive(A, 123, true, false, 0).unwrap();
        assert!(r.decoded(config).is_none());
        assert!(!r.decoder_is_recent(A));
    }
    #[test]
    fn reset_rejects_old_output_and_accepts_current_random_access() {
        let r = VideoReceiveRecovery::default();
        let old = frame(&r, A, true); let current = frame(&r, A, true);
        assert!(r.decoder_reset(current));
        assert!(r.decoded(old).is_none());
        assert!(r.decoded(current).is_some());
    }
    #[test]
    fn duplicates_and_unknown_receipts_cannot_extend_health() {
        let r = VideoReceiveRecovery::default();
        let first = frame(&r, A, true); r.decoded(first).unwrap();
        assert!(r.decoded(first).is_none()); assert!(r.decoded(u64::MAX).is_none());
        assert!(!r.dropped(u64::MAX)); assert!(!r.decoder_reset(0));
    }
    #[test]
    fn ledger_is_bounded_and_expired_receipts_fail_closed() {
        let r = VideoReceiveRecovery::default(); let old = frame(&r, A, true);
        for _ in 0..VideoReceiveRecovery::MAX_PENDING { frame(&r, A, false); }
        assert_eq!(r.0.lock().unwrap().pending.len(), VideoReceiveRecovery::MAX_PENDING);
        assert!(r.decoded(old).is_none());
    }
    #[test]
    fn intact_keyframe_after_missing_packets_remains_usable() {
        let r = VideoReceiveRecovery::default();
        let repair = r.receive(A, 129, false, true, 3).unwrap();
        let decoded = r.decoded(repair).unwrap();
        assert_eq!(decoded.timestamp, 129);
    }
    #[test]
    fn loss_reported_with_configuration_still_invalidates_the_old_chain() {
        let r = VideoReceiveRecovery::default();
        let old = frame(&r, A, true); r.decoded(old).unwrap();
        let config = r.receive(A, 124, true, false, 1).unwrap();
        assert!(r.decoded(config).is_none());
        assert!(!r.decoder_is_recent(A));
        assert!(r.receive(A, 125, false, false, 0).is_none());
        let key = frame(&r, A, true); r.decoded(key).unwrap();
    }
    #[test]
    fn callbacks_cannot_complete_recovery_more_than_once_concurrently() {
        use std::sync::{Arc, Barrier};
        let r = Arc::new(VideoReceiveRecovery::default());
        let receipt = frame(&r, A, true);
        let barrier = Arc::new(Barrier::new(8));
        let workers: Vec<_> = (0..8).map(|_| {
            let r = r.clone(); let barrier = barrier.clone();
            std::thread::spawn(move || { barrier.wait(); r.decoded(receipt).is_some() })
        }).collect();
        assert_eq!(workers.into_iter().filter_map(|w| w.join().ok()).filter(|v| *v).count(), 1);
    }
    #[test]
    fn receipt_exhaustion_never_aliases_an_old_frame() {
        let r = VideoReceiveRecovery::default();
        r.0.lock().unwrap().next_receipt = u64::MAX;
        assert!(r.receive(A, 1, false, true, 0).is_none());
    }
}
