use std::collections::HashSet;

/// Lost reference frames invalidate subsequent dependent frames. Keep the
/// damaged chain out of the decoder until an intact random-access frame.
#[derive(Default)]
pub(super) struct VideoReferenceChain {
    waiting: HashSet<(u64, u32)>,
    started: HashSet<(u64, u32)>,
}

impl VideoReferenceChain {
    pub(super) fn accept(&mut self, stream: (u64, u32), configuration: bool, keyframe: bool, lost: u16) -> bool {
        if configuration { return true; }
        if lost > 0 || !self.started.contains(&stream) { self.waiting.insert(stream); }
        if keyframe {
            self.started.insert(stream);
            self.waiting.remove(&stream);
        }
        !self.waiting.contains(&stream)
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
}
