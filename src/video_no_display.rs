use std::time::{Duration, Instant};

/// iOS27's default no-display gates. Adapter progress means confirmed decoder
/// output submitted to a live Surface, not packet arrival or physical scanout.
/// See FACETIME_NO_DISPLAY_2026-10-26.md for native ownership and adapter limits.
#[derive(Default)]
pub(super) struct VideoNoDisplay {
    expected_at: Option<Instant>,
    last_progress: Option<Instant>,
    last_fir: Option<Instant>,
    last_notification: Option<Instant>,
    next_ticket: u64,
    pending: Option<u64>,
}
impl VideoNoDisplay {
    const TIMEOUT: Duration = Duration::from_secs(5);
    pub(super) fn set_expected(&mut self, expected: bool, now: Instant) {
        if expected == self.expected_at.is_some() { return; }
        self.expected_at = expected.then_some(now);
        self.pending = None;
    }
    pub(super) fn progress(&mut self, now: Instant) {
        self.last_progress = Some(self.last_progress.map_or(now, |old| old.max(now)));
        self.pending = None;
    }
    pub(super) fn fir_sent(&mut self, now: Instant) {
        self.last_fir = Some(self.last_fir.map_or(now, |old| old.max(now)));
        self.pending = None;
    }
    fn due(&self, now: Instant) -> bool {
        let Some(expected) = self.expected_at else { return false; };
        let older_than_timeout = |at| now.checked_duration_since(at).is_some_and(|d| d > Self::TIMEOUT);
        older_than_timeout(expected)
            && older_than_timeout(self.last_progress.unwrap_or(expected))
            && self.last_fir.is_none_or(|at| now.checked_duration_since(at).is_some_and(|d| d >= Self::TIMEOUT))
    }
    pub(super) fn poll(&mut self, now: Instant) -> Option<u64> {
        if !self.due(now) || self.last_notification.is_some_and(|at|
            now.checked_duration_since(at).is_none_or(|d| d < Self::TIMEOUT)) { return None; }
        self.next_ticket = self.next_ticket.checked_add(1)?;
        self.last_notification = Some(now);
        self.pending = Some(self.next_ticket);
        self.pending
    }
    pub(super) fn dispatch(&mut self, ticket: u64, now: Instant) -> bool {
        if self.pending != Some(ticket) || !self.due(now) { return false; }
        self.fir_sent(now);
        true
    }
}
