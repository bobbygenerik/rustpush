// iOS 27 AVConference: VC...FindPacketInTxHistory 0x205f5ab54,
// AddInfoToOneToOneTxHistory 0x206372c30. Echoed audio send-time token identifies a snapshot
// of cumulative VIDEO transmissions; token distance is not the denominator.
use std::collections::VecDeque;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LossSample { pub expected: u64, pub lost: u64 }

struct Snapshot {
    sequence: u16,
    sent: u64,
    time: Duration,
    received: Option<u64>,
}

#[derive(Default)]
pub(super) struct SendHistory {
    snapshots: VecDeque<Snapshot>,
    last_sequence: Option<u16>,
    received: Option<u64>,
}

impl SendHistory {
    pub(super) fn reset(&mut self) { *self = Self::default(); }

    pub(super) fn record_audio(&mut self, sequence: u16, video_sent: u64, time: Duration) {
        if self.snapshots.back().is_some_and(|last| video_sent < last.sent) { self.reset(); }
        self.snapshots.push_back(Snapshot { sequence, sent: video_sent, time, received: None });
        if self.snapshots.len() > 512 { self.snapshots.pop_front(); }
    }

    pub(super) fn observe(&mut self, sequence: u16, received: u16, now: Duration) -> Option<LossSample> {
        // Unknown/stale feedback cannot advance the receive-count baseline.
        let index = self.snapshots.iter().rposition(|item| item.sequence == sequence)?;
        let current = &self.snapshots[index];
        if now < current.time || now - current.time > Duration::from_secs(5) { return None }
        if self.last_sequence.is_some_and(|last| {
            let delta = sequence.wrapping_sub(last); delta == 0 || delta >= 0x8000
        }) { return None }
        let raw = u64::from(received & 0x0fff);
        let count = if let Some(last) = self.received {
            let delta = raw.wrapping_sub(last & 0x0fff) & 0x0fff;
            if delta >= 0x0800 { return None }
            last + delta
        } else { raw };
        self.snapshots[index].received = Some(count);
        self.received = Some(count);
        self.last_sequence = Some(sequence);

        let current_time = self.snapshots[index].time;
        // Native considers three nearby matched endpoints on either side of
        // the one-second interval and keeps the minimum nonnegative loss.
        let recent: Vec<_> = self.snapshots.iter().take(index + 1).rev()
            .filter(|item| item.received.is_some()).take(3).collect();
        let older: Vec<_> = self.snapshots.iter().take(index).rev()
            .filter(|item| item.received.is_some()
                && current_time.saturating_sub(item.time) > Duration::from_secs(1))
            .take(3).collect();
        let mut best: Option<LossSample> = None;
        for new in recent {
            for old in &older {
                let elapsed = new.time.saturating_sub(old.time);
                if elapsed <= Duration::from_secs(1) || elapsed > Duration::from_secs(5) { continue }
                let Some(expected) = new.sent.checked_sub(old.sent).filter(|value| *value != 0) else { continue };
                let Some(received) = new.received?.checked_sub(old.received?) else { continue };
                let sample = LossSample { expected, lost: expected.saturating_sub(received) };
                if best.is_none_or(|previous| u128::from(sample.lost) * u128::from(previous.expected)
                    < u128::from(previous.lost) * u128::from(sample.expected)) {
                    best = Some(sample);
                }
            }
        }
        best
    }
}
