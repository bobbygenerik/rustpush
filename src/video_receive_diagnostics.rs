//! Bounded, local observations of transport versus reassembly. These counters
//! never participate in feedback, codec negotiation, or frame delivery.
use std::{collections::HashSet, time::Duration};

const WINDOW: Duration = Duration::from_secs(2);
const MAX_FRAME_IDS: usize = 256;

#[derive(Debug, Default)]
pub(super) struct ReceiveSnapshot {
    pub elapsed_ms: u128,
    pub primary_packets: u64,
    pub parity_packets: u64,
    pub payload_bytes: u64,
    pub primary_timestamps: usize,
    pub declared_frames: usize,
    pub missing_declared_frames: Option<usize>,
    pub assembled_frames: u64,
    pub assembled_bytes: u64,
    pub assembly_dropped_packets: u64,
    pub last_rtp_timestamp: Option<u32>,
    pub last_footer_removed: usize,
    pub last_footer_control: Option<u8>,
    pub bounded: bool,
}

#[derive(Default)]
pub(super) struct VideoReceiveWindow {
    start: Option<Duration>,
    timestamps: HashSet<u32>,
    frame_ids: HashSet<u16>,
    first_frame_id: Option<u16>,
    frame_span: usize,
    snapshot: ReceiveSnapshot,
}

impl VideoReceiveWindow {
    pub fn packet(&mut self, now: Duration, timestamp: u32, frame_id: Option<u16>,
                  parity: bool, payload_bytes: usize) -> Option<ReceiveSnapshot> {
        let report = self.start.and_then(|start| now.checked_sub(start))
            .filter(|elapsed| *elapsed >= WINDOW)
            .map(|elapsed| {
                self.snapshot.elapsed_ms = elapsed.as_millis();
                self.snapshot.primary_timestamps = self.timestamps.len();
                self.snapshot.declared_frames = self.frame_ids.len();
                self.snapshot.missing_declared_frames = (!self.snapshot.bounded
                    && self.first_frame_id.is_some())
                    .then(|| self.frame_span.saturating_sub(self.frame_ids.len()));
                let report = std::mem::take(&mut self.snapshot);
                self.timestamps.clear();
                self.frame_ids.clear();
                self.first_frame_id = None;
                self.frame_span = 0;
                self.start = Some(now);
                report
            });
        self.start.get_or_insert(now);
        self.snapshot.payload_bytes += payload_bytes as u64;
        if parity {
            self.snapshot.parity_packets += 1;
        } else {
            self.snapshot.primary_packets += 1;
            self.snapshot.last_rtp_timestamp = Some(timestamp);
            if self.timestamps.len() < MAX_FRAME_IDS {
                self.timestamps.insert(timestamp);
            } else if !self.timestamps.contains(&timestamp) {
                self.snapshot.bounded = true;
            }
            if let Some(frame_id) = frame_id {
                if self.frame_ids.len() < MAX_FRAME_IDS {
                    self.frame_ids.insert(frame_id);
                } else if !self.frame_ids.contains(&frame_id) {
                    self.snapshot.bounded = true;
                }
                let first = *self.first_frame_id.get_or_insert(frame_id);
                let forward = frame_id.wrapping_sub(first);
                if forward < 0x8000 {
                    self.frame_span = self.frame_span.max(usize::from(forward) + 1);
                } else {
                    // A frame older than the window's first observation makes
                    // a span-based gap estimate ambiguous; retain raw counts.
                    self.snapshot.bounded = true;
                }
            }
        }
        report
    }

    pub fn frame(&mut self, bytes: usize, dropped_packets: u16,
                 footer_removed: usize, footer_control: Option<u8>) {
        self.snapshot.assembled_frames += 1;
        self.snapshot.assembled_bytes += bytes as u64;
        self.snapshot.assembly_dropped_packets += u64::from(dropped_packets);
        self.snapshot.last_footer_removed = footer_removed;
        self.snapshot.last_footer_control = footer_control;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separates_transport_frames_from_reassembly_and_parity() {
        let mut window = VideoReceiveWindow::default();
        // Six arriving pictures, two packets each, but only two assembled.
        for frame in 0..6u16 {
            for packet in 0..2 {
                assert!(window.packet(Duration::from_millis(u64::from(frame) * 300 + packet),
                    u32::from(frame) * 800, Some(frame + 40), false, 100).is_none());
            }
            window.packet(Duration::from_millis(u64::from(frame) * 300 + 2),
                999, Some(999), true, 40);
            if frame % 3 == 0 { window.frame(180, 0, 11, Some(0x16)); }
        }
        let report = window.packet(WINDOW, 9000, Some(46), false, 100).unwrap();
        assert_eq!((report.primary_packets, report.parity_packets), (12, 6));
        assert_eq!((report.primary_timestamps, report.declared_frames), (6, 6));
        assert_eq!(report.missing_declared_frames, Some(0));
        assert_eq!((report.assembled_frames, report.assembled_bytes), (2, 360));
        assert_eq!(report.payload_bytes, 1440);
        assert_eq!((report.last_footer_removed, report.last_footer_control), (11, Some(0x16)));
        // The packet triggering the report belongs to the new interval.
        let next = window.packet(WINDOW * 2, 10000, Some(47), false, 50).unwrap();
        assert_eq!((next.primary_packets, next.assembled_frames, next.payload_bytes), (1, 0, 100));
    }

    #[test]
    fn sequence_wrap_and_reordering_do_not_invent_frame_gaps() {
        let mut window = VideoReceiveWindow::default();
        for (index, id) in [65534, 0, 65535, 2, 0].into_iter().enumerate() {
            window.packet(Duration::from_millis(index as u64 * 100), id as u32,
                Some(id), false, 1);
        }
        let report = window.packet(WINDOW, 3, Some(3), false, 1).unwrap();
        assert_eq!(report.declared_frames, 4);
        assert_eq!(report.missing_declared_frames, Some(1));
        assert!(!report.bounded);
    }

    #[test]
    fn unknown_or_over_capacity_frame_ids_do_not_claim_zero_loss() {
        let mut window = VideoReceiveWindow::default();
        window.packet(Duration::ZERO, 0, None, false, 1);
        let report = window.packet(WINDOW, 1, None, false, 1).unwrap();
        assert_eq!(report.missing_declared_frames, None);
        for id in 0..300 {
            window.packet(WINDOW, id, Some(id as u16), false, 1);
        }
        let report = window.packet(WINDOW * 2, 301, Some(301), false, 1).unwrap();
        assert!(report.bounded);
        assert_eq!(report.missing_declared_frames, None);
        assert!(report.primary_timestamps <= MAX_FRAME_IDS);
        assert!(report.declared_frames <= MAX_FRAME_IDS);
    }
}
