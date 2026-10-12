//! Bounded video NACK recovery. Policy comes from iOS 27.0.1 / 24A446
//! VCNackGeneratorConfig (0x20626aba0) and VCNackGenerator (0x20626b474).
//! All times are monotonic offsets supplied by the media worker.
use std::{collections::VecDeque, time::Duration};

const MAX_INFLIGHT: usize = 20;
const MAX_RETRIES: u8 = 10;
const MAX_AGE: Duration = Duration::from_secs(1);
const TOKEN_RATE: f64 = 20.0;
const TOKEN_BURST: f64 = 40.0;

#[derive(Debug)]
struct Missing {
    sequence: u16,
    detected: Duration,
    last_sent: Option<Duration>,
    sends: u8,
}

pub(super) struct VideoNackGenerator {
    highest: Option<u16>,
    recent: VecDeque<u16>,
    missing: VecDeque<Missing>,
    retry_seconds: f64,
    delay_seconds: f64,
    estimated_rtt: f64,
    estimated_deviation: f64,
    tokens: f64,
    token_time: Duration,
}

impl Default for VideoNackGenerator {
    fn default() -> Self {
        Self {
            highest: None,
            recent: VecDeque::new(),
            missing: VecDeque::new(),
            retry_seconds: 0.2,
            delay_seconds: 0.01,
            // VCSRTTEstimator init, 0x20626d0e8: 150 ms / 20 ms.
            estimated_rtt: 0.15,
            estimated_deviation: 0.02,
            tokens: TOKEN_BURST,
            token_time: Duration::ZERO,
        }
    }
}

impl VideoNackGenerator {
    /// Call only after SRTP and control-extension validation. Parity may reuse
    /// the previous primary sequence; duplicates never create a new gap.
    pub(super) fn observe(&mut self, sequence: u16, now: Duration, retransmitted: bool) {
        self.fulfill(sequence, now, retransmitted);
        let Some(highest) = self.highest else {
            self.highest = Some(sequence);
            return;
        };
        let jump = sequence.wrapping_sub(highest);
        if jump == 0 || jump >= 0x8000 { return; }
        // Native keeps a 128-entry RTP history. Never expand an arbitrarily
        // large discontinuity into requests; retain only the latest 20 gaps.
        if usize::from(jump - 1) <= self.recent.len() {
            for offset in 1..jump {
                let sequence = highest.wrapping_add(offset);
                if !self.recent.contains(&sequence)
                    && !self.missing.iter().any(|entry| entry.sequence == sequence) {
                    self.missing.push_back(Missing {
                        sequence, detected: now, last_sent: None, sends: 0,
                    });
                }
            }
            while self.missing.len() > MAX_INFLIGHT { self.missing.pop_front(); }
        }
        self.highest = Some(sequence);
    }

    /// A late authenticated packet or successful FEC repair cancels requests.
    /// Only an explicitly marked relay retransmission updates the RTT model.
    pub(super) fn fulfill(&mut self, sequence: u16, now: Duration, retransmitted: bool) {
        if !self.recent.contains(&sequence) {
            self.recent.push_back(sequence);
            if self.recent.len() > 128 { self.recent.pop_front(); }
        }
        let Some(index) = self.missing.iter().position(|entry| entry.sequence == sequence) else { return; };
        let entry = self.missing.remove(index).unwrap();
        if retransmitted {
            // Native fulfillment measures from gapDetectedTime, not lastSendTime.
            let sample = now.saturating_sub(entry.detected).as_secs_f64();
            self.estimated_rtt = self.estimated_rtt * 0.85 + sample * 0.15;
            self.estimated_deviation = self.estimated_deviation * 0.95
                + (sample - self.estimated_rtt).abs() * 0.05;
            self.retry_seconds = self.estimated_rtt;
            self.delay_seconds = self.estimated_deviation * 0.25;
        }
    }

    fn refill(&mut self, now: Duration) {
        self.tokens = (self.tokens + now.saturating_sub(self.token_time).as_secs_f64() * TOKEN_RATE)
            .min(TOKEN_BURST);
        self.token_time = now;
    }

    fn due_at(&self, entry: &Missing) -> Duration {
        let initial = entry.detected + Duration::from_secs_f64(self.delay_seconds);
        entry.last_sent.map(|sent| (sent + Duration::from_secs_f64(self.retry_seconds)).max(initial))
            .unwrap_or(initial)
    }

    /// Returns due sequences without committing a send. A full control queue
    /// must not consume retry attempts or a token.
    pub(super) fn ready(&mut self, now: Duration) -> Vec<u16> {
        self.refill(now);
        self.missing.retain(|entry| now.saturating_sub(entry.detected) <= MAX_AGE
            && entry.sends <= MAX_RETRIES);
        if self.tokens < 1.0 { return vec![]; }
        self.missing.iter().filter(|entry| self.due_at(entry) <= now)
            .map(|entry| entry.sequence).collect()
    }

    pub(super) fn sent(&mut self, sequences: &[u16], now: Duration) {
        if sequences.is_empty() { return; }
        self.tokens = (self.tokens - 1.0).max(0.0);
        for entry in &mut self.missing {
            if sequences.contains(&entry.sequence) {
                entry.last_sent = Some(now);
                entry.sends += 1;
            }
        }
    }

    pub(super) fn next_deadline(&self, now: Duration) -> Option<Duration> {
        let tokens = (self.tokens + now.saturating_sub(self.token_time).as_secs_f64() * TOKEN_RATE)
            .min(TOKEN_BURST);
        let token_ready = now + Duration::from_secs_f64(((1.0 - tokens) / TOKEN_RATE).max(0.0));
        self.missing.iter().map(|entry| {
            let expiry = entry.detected + MAX_AGE + Duration::from_nanos(1);
            if entry.sends > MAX_RETRIES { now } else { self.due_at(entry).max(token_ready).min(expiry) }
        }).min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ms(value: u64) -> Duration { Duration::from_millis(value) }

    #[test]
    fn reordering_and_fec_cancel_without_feedback() {
        let mut n = VideoNackGenerator::default();
        for sequence in 0..=10 { n.observe(sequence, ms(0), false); }
        n.observe(13, ms(1), false);
        assert!(n.ready(ms(10)).is_empty());
        n.observe(11, ms(5), false);
        n.fulfill(12, ms(8), false);
        assert!(n.ready(ms(11)).is_empty());
        assert_eq!(n.next_deadline(ms(11)), None);
        assert_eq!(n.retry_seconds, 0.2);
    }

    #[test]
    fn repair_seen_before_gap_is_not_requested_again() {
        let mut n = VideoNackGenerator::default();
        n.observe(10, ms(0), false);
        n.fulfill(11, ms(1), false);
        n.observe(12, ms(2), false);
        assert!(n.ready(ms(12)).is_empty());
        // Parity reuses a primary sequence, and old arrivals do not rewind it.
        n.observe(12, ms(3), false);
        n.observe(10, ms(4), false);
        assert_eq!(n.highest, Some(12));
    }

    #[test]
    fn lost_first_request_retries_before_reassembly_deadline() {
        let mut n = VideoNackGenerator::default();
        n.observe(65534, ms(0), false);
        n.observe(1, ms(1), false);
        let first = n.ready(ms(11));
        assert_eq!(first, vec![65535, 0]);
        // Failed local enqueue does not count as a transmitted request.
        assert_eq!(n.ready(ms(12)), first);
        n.sent(&first, ms(12));
        assert!(n.ready(ms(211)).is_empty());
        let retry = n.ready(ms(212));
        assert_eq!(retry, first);
        n.sent(&retry, ms(212));
        n.observe(65535, ms(230), true);
        n.observe(0, ms(230), true);
        assert!(n.ready(ms(240)).is_empty());
        assert!(n.retry_seconds < 0.2);
        assert_eq!(n.highest, Some(1));
    }

    #[test]
    fn expiry_retry_limit_and_bounded_large_gaps() {
        let mut n = VideoNackGenerator::default();
        for sequence in 0..=127 { n.observe(sequence, ms(0), false); }
        n.observe(256, ms(1), false);
        assert_eq!(n.missing.len(), MAX_INFLIGHT);
        assert_eq!(n.missing.front().unwrap().sequence, 236);
        assert!(n.ready(ms(1002)).is_empty());
        n.observe(5000, ms(1003), false);
        assert!(n.missing.is_empty());
        n.observe(5002, ms(1004), false);
        n.retry_seconds = 0.001;
        for time in 1014..1025 {
            let due = n.ready(ms(time));
            assert_eq!(due, vec![5001]);
            n.sent(&due, ms(time));
        }
        assert!(n.ready(ms(1025)).is_empty());
        assert_eq!(n.next_deadline(ms(1025)), None);
    }

    #[test]
    fn token_budget_limits_reports_and_refills() {
        let mut n = VideoNackGenerator::default();
        n.observe(1, ms(0), false);
        n.observe(3, ms(1), false);
        n.tokens = 0.0;
        n.token_time = ms(11);
        assert!(n.ready(ms(11)).is_empty());
        assert_eq!(n.next_deadline(ms(11)), Some(ms(61)));
        assert_eq!(n.ready(ms(61)), vec![2]);
        n.sent(&[2], ms(61));
        assert!(n.tokens < 0.001);
    }

    #[test]
    fn rtt_uses_only_marked_retransmissions_and_native_smoothing() {
        let mut n = VideoNackGenerator::default();
        n.observe(1, ms(0), false);
        n.observe(3, ms(1), false);
        n.fulfill(2, ms(101), true);
        assert!((n.estimated_rtt - 0.1425).abs() < 1e-10);
        assert!((n.delay_seconds - 0.00528125).abs() < 1e-10);
        n.observe(5, ms(102), false);
        n.fulfill(4, ms(110), false);
        assert!((n.estimated_rtt - 0.1425).abs() < 1e-10);
    }
}
