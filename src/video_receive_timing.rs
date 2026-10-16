//! Local diagnostics only. Tracks real transport arrivals, never synthetic FEC
//! packets. Missing/evicted tracking remains unknown rather than becoming zero.
use std::{collections::HashMap, time::Instant};

const CAPACITY: usize = 256;

#[derive(Clone, Copy, Debug)]
pub(super) struct FrameArrival {
    pub first: Instant,
    pub last: Instant,
    pub first_primary: Option<Instant>,
    pub last_primary: Option<Instant>,
    pub max_dispatch_us: u64,
    pub primary_packets: u32,
    pub parity_packets: u32,
}

#[derive(Default)]
pub(super) struct VideoReceiveTiming {
    pending: HashMap<u32, FrameArrival>,
}

impl VideoReceiveTiming {
    pub fn packet(&mut self, timestamp: u32, arrival: Instant, dispatch: Instant, parity: bool) {
        if !self.pending.contains_key(&timestamp) && self.pending.len() >= CAPACITY {
            if let Some(oldest) = self.pending.iter().min_by_key(|(_, frame)| frame.last)
                .map(|(timestamp, _)| *timestamp) {
                self.pending.remove(&oldest);
            }
        }
        let frame = self.pending.entry(timestamp).or_insert(FrameArrival {
            first: arrival, last: arrival, first_primary: None, last_primary: None,
            max_dispatch_us: 0, primary_packets: 0, parity_packets: 0,
        });
        frame.first = frame.first.min(arrival);
        frame.last = frame.last.max(arrival);
        frame.max_dispatch_us = frame.max_dispatch_us.max(
            dispatch.saturating_duration_since(arrival).as_micros().min(u64::MAX as u128) as u64);
        if parity {
            frame.parity_packets = frame.parity_packets.saturating_add(1);
        } else {
            frame.primary_packets = frame.primary_packets.saturating_add(1);
            frame.first_primary = Some(frame.first_primary.map_or(arrival, |old| old.min(arrival)));
            frame.last_primary = Some(frame.last_primary.map_or(arrival, |old| old.max(arrival)));
        }
    }

    pub fn complete(&mut self, timestamp: u32) -> Option<FrameArrival> {
        self.pending.remove(&timestamp)
    }
}

/// A fixed wall/monotonic anchor allows comparison with Dart and Android logs.
/// Wall clock changes during a call must not masquerade as packet queue delay.
pub(super) fn unix_us(at: Instant, anchor: Instant, anchor_unix_us: u64) -> u64 {
    if at >= anchor {
        anchor_unix_us.saturating_add(at.duration_since(anchor).as_micros().min(u64::MAX as u128) as u64)
    } else {
        anchor_unix_us.saturating_sub(anchor.duration_since(at).as_micros().min(u64::MAX as u128) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn separates_dispatch_delay_from_reassembly_wait_and_primary_from_parity() {
        let t = Instant::now();
        let mut tracker = VideoReceiveTiming::default();
        tracker.packet(50, t, t + Duration::from_millis(120), true);
        tracker.packet(50, t + Duration::from_millis(2), t + Duration::from_millis(122), false);
        tracker.packet(50, t + Duration::from_millis(3), t + Duration::from_millis(123), false);
        let frame = tracker.complete(50).unwrap();
        assert_eq!((frame.primary_packets, frame.parity_packets), (2, 1));
        assert_eq!(frame.max_dispatch_us, 120_000);
        assert_eq!(frame.first_primary, Some(t + Duration::from_millis(2)));
        assert_eq!(frame.last, t + Duration::from_millis(3));
        assert!(tracker.complete(50).is_none());
    }

    #[test]
    fn timestamp_wrap_reordering_and_eviction_keep_unknowns_explicit() {
        let t = Instant::now();
        let mut tracker = VideoReceiveTiming::default();
        tracker.packet(u32::MAX, t + Duration::from_millis(4), t + Duration::from_millis(5), false);
        tracker.packet(u32::MAX, t, t + Duration::from_millis(6), false);
        let frame = tracker.complete(u32::MAX).unwrap();
        assert_eq!(frame.first, t);
        assert_eq!(frame.max_dispatch_us, 6000);
        for i in 0..300 {
            tracker.packet(i, t + Duration::from_millis(i as u64), t + Duration::from_millis(i as u64), false);
        }
        assert_eq!(tracker.pending.len(), CAPACITY);
        assert!(tracker.complete(0).is_none());
        assert_eq!(tracker.complete(299).unwrap().primary_packets, 1);
    }

    #[test]
    fn fixed_anchor_handles_packets_parsed_before_worker_start() {
        let t = Instant::now();
        assert_eq!(unix_us(t - Duration::from_millis(4), t, 1_000_000), 996_000);
        assert_eq!(unix_us(t + Duration::from_millis(4), t, 1_000_000), 1_004_000);
    }
}
