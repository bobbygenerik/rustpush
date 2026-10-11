// AVConference iOS 27 / 24A446: retryCount 0x20618c44c, reliable loop
// 0x20618c4ac. The bound includes the initial send and final confirmation wait.
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub(super) enum RetryMode { Normal, Fast }

impl RetryMode {
    fn attempts(self) -> usize { match self { Self::Normal => 10, Self::Fast => 120 } }
    fn delay(self, attempt: usize) -> Duration {
        Duration::from_millis(match self {
            Self::Fast => 250,
            Self::Normal => match attempt { 0 => 500, 1 => 1000, 2 => 2000, _ => 4000 },
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RetireReason { Exhausted, TimedOut }

#[derive(Debug)]
pub(super) enum ControlAction<T> {
    Send { participant: u64, transaction: u64, payload: T },
    Retired { participant: u64, transaction: u64, reason: RetireReason },
}

struct Pending<T> {
    payload: T,
    attempts: usize,
    mode: RetryMode,
    next: Duration,
    timeout: Option<Duration>,
}

pub(super) struct ControlRetryQueue<T> {
    pending: BTreeMap<(u64, u64), Pending<T>>,
}

impl<T> Default for ControlRetryQueue<T> {
    fn default() -> Self { Self { pending: BTreeMap::new() } }
}

impl<T: Clone> ControlRetryQueue<T> {
    // Initial send belongs to the caller, only after successful admission.
    pub(super) fn insert(&mut self, participant: u64, transaction: u64, payload: T,
        now: Duration, timeout: Option<Duration>, mode: RetryMode) -> bool {
        let key = (participant, transaction);
        if self.pending.contains_key(&key) { return false }
        let timeout = timeout.map(|limit| now.saturating_add(limit));
        let next = now.saturating_add(mode.delay(0));
        self.pending.insert(key, Pending { payload, attempts: 1, mode,
            next: timeout.map_or(next, |limit| next.min(limit)), timeout });
        true
    }

    pub(super) fn next_deadline(&self) -> Option<Duration> {
        self.pending.values().map(|item| item.next).min()
    }

    pub(super) fn poll(&mut self, now: Duration) -> Vec<ControlAction<T>> {
        let mut actions = Vec::new();
        self.pending.retain(|&(participant, transaction), item| {
            if now < item.next { return true }
            let reason = if item.timeout.is_some_and(|limit| now >= limit) {
                Some(RetireReason::TimedOut)
            } else if item.attempts >= item.mode.attempts() {
                Some(RetireReason::Exhausted)
            } else { None };
            if let Some(reason) = reason {
                actions.push(ControlAction::Retired { participant, transaction, reason });
                return false
            }
            actions.push(ControlAction::Send { participant, transaction, payload: item.payload.clone() });
            let next = now.saturating_add(item.mode.delay(item.attempts));
            item.attempts += 1;
            item.next = item.timeout.map_or(next, |limit| next.min(limit));
            true
        });
        actions
    }

    pub(super) fn ack(&mut self, participant: u64, transaction: u64) -> bool {
        self.pending.remove(&(participant, transaction)).is_some()
    }
    pub(super) fn flush_participant(&mut self, participant: u64) -> usize {
        let before = self.len();
        self.pending.retain(|&(owner, _), _| owner != participant);
        before - self.len()
    }
    pub(super) fn flush(&mut self) -> usize {
        let before = self.len(); self.pending.clear(); before
    }
    pub(super) fn len(&self) -> usize { self.pending.len() }
}
