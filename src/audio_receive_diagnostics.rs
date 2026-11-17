//! Bounded, metadata-only receive measurements. These do not drive OWRD or playback.
//! Frame counts describe the existing length-prefixed framing, not decoded audio
//! or Apple's DTX/repair classifications. Unknown framing stays unknown.

use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameLayout {
    pub frames: usize,
    pub empty_frames: usize,
    pub min_bytes: usize,
    pub max_bytes: usize,
}

pub fn frame_layout(mut payload: &[u8]) -> Option<FrameLayout> {
    let mut layout = FrameLayout { frames: 0, empty_frames: 0, min_bytes: usize::MAX, max_bytes: 0 };
    while let Some((&length, rest)) = payload.split_first() {
        let length = usize::from(length);
        payload = rest.get(length..)?;
        layout.frames += 1;
        layout.empty_frames += usize::from(length == 0);
        layout.min_bytes = layout.min_bytes.min(length);
        layout.max_bytes = layout.max_bytes.max(length);
    }
    (layout.frames != 0).then_some(layout)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PacketFormat {
    pub configuration: u64,
    pub stream_group: u32,
    pub negotiated_stream_id: Option<u32>,
    pub outer_payload: u8,
    pub primary_payload: Option<u8>,
    // None means the RED packet could not be split; zero means no redundant blocks.
    pub red_blocks: Option<usize>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ClockSnapshot {
    pub ts_rate: u32,
    pub samples_per_frame: u32,
    pub ref_sequence: u16,
    pub ref_timestamp: u32,
    pub ref_arrival_sec: f64,
    pub lag_sec: f64,
    pub owrd_sec: f64,
    pub owrd_q13: u16,
    pub filter_short: f64,
    pub filter_long: f64,
    pub in_spike: i32,
    pub packet_counter: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct PacketSample {
    pub sequence: u16,
    pub timestamp: u32,
    pub arrival_sec: f64,
    pub marker: bool,
    pub payload_bytes: usize,
    pub primary_bytes: Option<usize>,
    pub format: PacketFormat,
    pub layout: Option<FrameLayout>,
    pub clock: ClockSnapshot,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PacketDelta {
    pub sequence_step: Option<i16>,
    pub timestamp_step: Option<i32>,
    pub arrival_step_sec: Option<f64>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WindowSummary {
    pub elapsed_ms: u64,
    pub packets: u32,
    // Index is the number of length-prefixed frames, not frames inferred from RTP gaps.
    pub frame_counts: [u32; 5],
    pub other_frames: u32,
    pub unknown_frames: u32,
    pub changed_packets: u32,
    pub nonconsecutive_sequences: u32,
    pub marker_packets: u32,
    pub payload_bytes: u64,
}

pub struct Observation {
    pub delta: PacketDelta,
    pub changed: bool,
    pub detail: bool,
    pub summary: Option<WindowSummary>,
}

#[derive(Default)]
pub struct AudioPacketDiagnostics {
    previous: Option<PacketSample>,
    window: WindowSummary,
    window_start: Option<Duration>,
    last_burst: Option<Duration>,
    burst_remaining: u8,
}

impl AudioPacketDiagnostics {
    pub fn observe(&mut self, now: Duration, sample: PacketSample) -> Observation {
        let delta = self.previous.map(|previous| PacketDelta {
            sequence_step: Some(sample.sequence.wrapping_sub(previous.sequence) as i16),
            timestamp_step: Some(sample.timestamp.wrapping_sub(previous.timestamp) as i32),
            arrival_step_sec: Some(sample.arrival_sec - previous.arrival_sec),
        }).unwrap_or_default();
        let shape = |packet: PacketSample| (packet.format, packet.layout.map(|l| (l.frames, l.empty_frames)));
        let changed = self.previous.is_none_or(|previous| shape(previous) != shape(sample));
        self.previous = Some(sample);
        // Eight packet samples at startup or a format/frame-count change, with
        // at most one burst start per five seconds even under alternating input.
        if changed && self.last_burst.is_none_or(|last| now.saturating_sub(last) >= Duration::from_secs(5)) {
            self.last_burst = Some(now);
            self.burst_remaining = 8;
        }
        let detail = self.burst_remaining != 0;
        self.burst_remaining = self.burst_remaining.saturating_sub(1);

        self.window.packets += 1;
        self.window.payload_bytes += sample.payload_bytes as u64;
        self.window.changed_packets += u32::from(changed);
        self.window.nonconsecutive_sequences += u32::from(delta.sequence_step.is_some_and(|step| step != 1));
        self.window.marker_packets += u32::from(sample.marker);
        match sample.layout {
            Some(layout) if layout.frames < self.window.frame_counts.len() => self.window.frame_counts[layout.frames] += 1,
            Some(_) => self.window.other_frames += 1,
            None => self.window.unknown_frames += 1,
        }
        let start = *self.window_start.get_or_insert(now);
        let summary = if now.saturating_sub(start) >= Duration::from_secs(1) {
            self.window.elapsed_ms = now.saturating_sub(start).as_millis() as u64;
            self.window_start = Some(now);
            Some(std::mem::take(&mut self.window))
        } else { None };
        Observation { delta, changed, detail, summary }
    }
}
