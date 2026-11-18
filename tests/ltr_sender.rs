use std::time::{Duration, Instant};

use rustpush::ltr_tx::{
    DEFAULT_LTR_RTT_TIMEOUT_MS, LTR_BIT_MARK, LTR_BIT_REFRESH, LtrTxTracker, TX_LTR_RING_CAPACITY,
};

#[test]
fn ring_tracks_marked_frames_and_matches_ack_by_rtp_ts() {
    let t0 = Instant::now();
    let mut tracker = LtrTxTracker::new();
    assert_eq!(tracker.rtt_or_timeout_ms(), DEFAULT_LTR_RTT_TIMEOUT_MS);
    assert_eq!(tracker.select_refresh_token(t0), None);

    // Non-marked P-frame does not enter the 100-entry TX ring.
    tracker.record_sent(false, 0, Some(99), 1000, t0);
    assert_eq!(tracker.pending_len(), 0);

    // Marked LTR frame (bit 0) enters the ring.
    tracker.record_sent(false, LTR_BIT_MARK, Some(10), 2000, t0);
    assert_eq!(tracker.pending_len(), 1);

    // Unmatched RTP timestamp returns None.
    assert_eq!(tracker.ack_rtp_timestamp(9999, t0 + Duration::from_millis(50)), None);
    assert_eq!(tracker.pending_len(), 1);

    // Matching ACK transitions slot to acked and sets initial RTT EWMA = 80 ms.
    let ev = tracker
        .ack_rtp_timestamp(2000, t0 + Duration::from_millis(80))
        .expect("matching ACK");
    assert_eq!(ev.token, 10);
    assert_eq!(ev.rtp_ts, 2000);
    assert_eq!(ev.rtt_ms, 80);
    assert_eq!(tracker.rtt_or_timeout_ms(), 80);
    assert_eq!(tracker.pending_len(), 0);
    assert_eq!(tracker.select_refresh_token(t0 + Duration::from_millis(81)), Some(10));
}

#[test]
fn ewma_uses_875_125_and_prunes_older_and_2x_timed_out_entries() {
    let t0 = Instant::now();
    let mut tracker = LtrTxTracker::new();

    // Seed initial RTT = 100 ms with token 1 @ rtp_ts 1000.
    tracker.record_sent(false, LTR_BIT_MARK, Some(1), 1000, t0);
    let ev1 = tracker.ack_rtp_timestamp(1000, t0 + Duration::from_millis(100)).unwrap();
    assert_eq!(ev1.rtt_ms, 100);

    // Mark token 2 (older), token 3 (stale older unacked), token 4 (newer).
    let t1 = t0 + Duration::from_millis(110);
    tracker.record_sent(false, LTR_BIT_MARK, Some(2), 2000, t1);
    tracker.record_sent(false, LTR_BIT_MARK, Some(3), 3000, t1 + Duration::from_millis(33));
    tracker.record_sent(false, LTR_BIT_MARK | LTR_BIT_REFRESH, Some(4), 4000, t1 + Duration::from_millis(66));

    // Acking token 4 at +180ms sample:
    // sample = (110 + 180) - (110 + 66) = 114ms; EWMA = 0.875*100 + 0.125*114 = 101.75 -> 102ms.
    let ev4 = tracker
        .ack_rtp_timestamp(4000, t1 + Duration::from_millis(180))
        .unwrap();
    assert_eq!(ev4.token, 4);
    assert_eq!(ev4.rtt_ms, 102);
    // Older unacked tokens 2 and 3 (order < max_acked_order) are timed out / pruned immediately.
    assert_eq!(tracker.pending_len(), 0);
    assert_eq!(tracker.select_refresh_token(t1 + Duration::from_millis(180)), Some(4));

    // Verify 2 * rtt_or_timeout_ms expiry (= 204 ms).
    let t2 = t1 + Duration::from_millis(200);
    tracker.record_sent(false, LTR_BIT_MARK, Some(5), 5000, t2);
    tracker.expire_stale_pending(t2 + Duration::from_millis(200));
    assert_eq!(tracker.pending_len(), 1);
    tracker.expire_stale_pending(t2 + Duration::from_millis(205));
    assert_eq!(tracker.pending_len(), 0);
}

#[test]
fn keyframe_resets_rings_and_capacity_evicts_at_100() {
    let t0 = Instant::now();
    let mut tracker = LtrTxTracker::new();

    for i in 0..(TX_LTR_RING_CAPACITY as u32 + 5) {
        tracker.record_sent(false, LTR_BIT_MARK, Some(i), 1000 + i * 800, t0);
    }
    assert_eq!(tracker.pending_len(), TX_LTR_RING_CAPACITY);
    // Oldest 5 entries (tokens 0..5) were evicted.
    assert_eq!(tracker.ack_rtp_timestamp(1000, t0 + Duration::from_millis(30)), None);
    // Token 5 @ 1000 + 5*800 = 5000 still present.
    assert_eq!(
        tracker.ack_rtp_timestamp(5000, t0 + Duration::from_millis(30)).map(|e| e.token),
        Some(5)
    );
    assert_eq!(tracker.select_refresh_token(t0 + Duration::from_millis(30)), Some(5));

    // IDR resets both pending and acked rings (0x206022f60) before recording if IDR itself marks LTR.
    tracker.record_sent(true, 0, None, 99_999, t0 + Duration::from_millis(40));
    assert_eq!(tracker.pending_len(), 0);
    assert_eq!(tracker.acked_len(), 0);
    assert_eq!(tracker.select_refresh_token(t0 + Duration::from_millis(40)), None);
}
