use std::collections::{HashMap, HashSet};

/// Lost reference frames invalidate subsequent dependent frames. Keep the
/// damaged chain out of the decoder until an intact random-access frame.
#[derive(Default)]
pub(super) struct VideoReferenceChain {
    waiting: HashSet<(u64, u32)>,
    started: HashSet<(u64, u32)>,
}

impl VideoReferenceChain {
    pub(super) fn retain_participants(&mut self, active: &HashSet<u64>) {
        self.waiting.retain(|(participant, _)| active.contains(participant));
        self.started.retain(|(participant, _)| active.contains(participant));
    }

    pub(super) fn invalidate(&mut self, stream: (u64, u32)) {
        self.waiting.insert(stream);
    }

    pub(super) fn accept(&mut self, stream: (u64, u32), configuration: bool, keyframe: bool, lost: u16) -> bool {
        self.accept_with_ltr(stream, configuration, keyframe, 0, lost)
    }

    pub(super) fn accept_with_ltr(
        &mut self,
        stream: (u64, u32),
        configuration: bool,
        keyframe: bool,
        ltr_bits: u8,
        lost: u16,
    ) -> bool {
        self.accept_with_ltr_decision(stream, configuration, keyframe, ltr_bits, lost).0
    }

    pub(super) fn accept_with_ltr_decision(
        &mut self,
        stream: (u64, u32),
        configuration: bool,
        keyframe: bool,
        ltr_bits: u8,
        lost: u16,
    ) -> (bool, bool) {
        if configuration { return (true, false); }
        if lost > 0 || !self.started.contains(&stream) { self.waiting.insert(stream); }
        let is_ltr_refresh = (ltr_bits & 0x02) != 0 && self.started.contains(&stream);
        if keyframe || is_ltr_refresh {
            self.started.insert(stream);
            self.waiting.remove(&stream);
        }
        (!self.waiting.contains(&stream), is_ltr_refresh)
    }

    pub(super) fn is_not_waiting(&self, stream: (u64, u32)) -> bool {
        self.started.contains(&stream) && !self.waiting.contains(&stream)
    }
}

/// Measurement-only observer for FaceTime type-6 `frameSequenceNumber` gaps
/// while the reference chain is active and not waiting.
#[derive(Default)]
pub struct FrameSeqGapTracker {
    last_seq: HashMap<(u64, u32), u16>,
    gap_count: u64,
}

impl FrameSeqGapTracker {
    /// Records `seq` for `stream`. When a non-refresh frame arrives while the
    /// reference chain is not waiting and `seq != prev.wrapping_add(1)`, returns
    /// `Some((prev, seq, should_log))` (logging first 5, then every 50th).
    pub fn observe(
        &mut self,
        stream: (u64, u32),
        seq: u16,
        chain_not_waiting: bool,
        is_refresh: bool,
    ) -> Option<(u16, u16, bool)> {
        let prev = self.last_seq.insert(stream, seq)?;
        if chain_not_waiting && !is_refresh && seq != prev.wrapping_add(1) {
            let n = self.gap_count;
            self.gap_count = n.wrapping_add(1);
            let should_log = n < 5 || n % 50 == 0;
            return Some((prev, seq, should_log));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loss_waits_for_keyframe_and_configuration_cannot_release_dependents() {
        let mut gate = VideoReferenceChain::default();
        let s = (1, 0);
        assert!(!gate.accept(s, false, false, 0));
        assert!(gate.accept(s, true, false, 0));
        assert!(!gate.accept(s, false, false, 0));
        assert!(gate.accept(s, false, true, 0));
        assert!(gate.accept(s, false, false, 0));
        assert!(!gate.accept(s, false, false, 3));
        assert!(gate.accept(s, true, false, 0));
        assert!(!gate.accept(s, false, false, 0));
        assert!(gate.accept(s, false, true, 4));
        assert!(gate.accept(s, false, false, 0));
    }
    #[test]
    fn loss_on_one_stream_does_not_block_another() {
        let mut gate = VideoReferenceChain::default();
        assert!(gate.accept((1, 0), false, true, 0));
        assert!(gate.accept((2, 0), false, true, 0));
        assert!(!gate.accept((1, 0), false, false, 2));
        assert!(gate.accept((2, 0), false, false, 0));
    }
    #[test]
    fn frame_seq_gap_tracker_logs_only_when_chain_not_waiting_with_throttle() {
        let mut tracker = FrameSeqGapTracker::default();
        let s = (1, 0);
        assert_eq!(tracker.observe(s, 65535, true, false), None);
        // Normal wraparound 65535 -> 0 is not a gap.
        assert_eq!(tracker.observe(s, 0, true, false), None);
        // Gap while waiting is suppressed.
        assert_eq!(tracker.observe(s, 5, false, false), None);
        // Gap on keyframe/refresh is suppressed.
        assert_eq!(tracker.observe(s, 10, true, true), None);
        // First 5 gaps log (count 0..4), 6th..50th suppressed, 51st (count 50) logs.
        let mut logged = Vec::new();
        let mut cur = 10u16;
        for i in 0..52u16 {
            let next = cur.wrapping_add(2);
            let res = tracker.observe(s, next, true, false).unwrap();
            assert_eq!((res.0, res.1), (cur, next));
            if res.2 {
                logged.push(i);
            }
            cur = next;
        }
        assert_eq!(logged, vec![0, 1, 2, 3, 4, 50]);
    }
}
