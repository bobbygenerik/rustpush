//! 24A446 1:1 sender adapter. The algorithm in `vcrc/` is intentionally unchanged.
//! Evidence and limitations: sender-rate-port-20261117/wiring.md.

use crate::vcrc::{MediaController, PacketLossInput, PacketLossOutput, VcrcState, VcrcStatistics};
use crate::vcrc::tx_history::TxHistory;
use crate::vcrc_config::mode1_camera_defaults;
use super::audio_feedback::{self, FTAudioControlData};

/// Queue rate monitor: 32 buckets of 1/64 s. Admission and transmission are distinct.
/// `__VCMediaQueue_RateStats_TrackMediaPacket` 0x20636e3c4..438, 0x20636e4e8..57c;
/// `VCMediaQueue_RetrieveAudioVideoRates` 0x206371e8c..eb8.
#[derive(Clone, Debug)]
struct QueueRate {
    epoch: f64,
    tick: u64,
    buckets: [u32; 32],
    bytes: u32,
}
impl QueueRate {
    fn new(now: f64) -> Self { Self { epoch: now, tick: 0, buckets: [0; 32], bytes: 0 } }
    fn track(&mut self, bytes: u32, now: f64) {
        let tick = ((now - self.epoch).max(0.0) * 64.0) as u64;
        let elapsed = tick.saturating_sub(self.tick);
        if elapsed >= 32 {
            self.buckets = [0; 32];
            self.bytes = 0;
        } else {
            for k in 1..=elapsed {
                let bucket = ((self.tick + k) & 31) as usize;
                self.bytes = self.bytes.wrapping_sub(self.buckets[bucket]);
                self.buckets[bucket] = 0;
            }
        }
        self.tick = tick;
        let bucket = (tick & 31) as usize;
        self.buckets[bucket] = self.buckets[bucket].wrapping_add(bytes);
        self.bytes = self.bytes.wrapping_add(bytes);
    }
    fn bps(&self, now: f64) -> f64 {
        let age = now - (self.epoch + self.tick as f64 / 64.0);
        if age > 0.5 { 0.0 } else { (self.bytes.wrapping_shl(3) as i32) as f64 / (age + 0.484375) }
    }
}

/// Real outgoing queue state shared by the AAC and video senders of one session.
#[derive(Debug)]
pub struct SenderMediaController {
    pub tx_history: TxHistory,
    rates: [QueueRate; 4],
    now: f64,
    applied_counter: u32,
    applied_time: f64,
    pub audio_only: bool,
    pub video_stopped: bool,
    pub last_refresh: f64,
    pub min_target_bitrate: u32,
    pub disable_large_frame_requests: bool,
    pub rate_limited_max_time_exceeded: bool,
    /// Diagnostic snapshot before LCT clears these bits (0x205f964f0..504).
    pub last_decision_status: (u32, u32),
}
impl SenderMediaController {
    pub fn new(audio_only: bool, now: f64) -> Self {
        Self { tx_history: TxHistory::default(), rates: std::array::from_fn(|_| QueueRate::new(now)),
            now, applied_counter: 0, applied_time: 0.0, audio_only,
            video_stopped: false, last_refresh: 0.0, min_target_bitrate: 0,
            disable_large_frame_requests: false, rate_limited_max_time_exceeded: false,
            last_decision_status: (0, 0) }
    }
    /// 0x2063716a0..6b0: track queue input, independently of paced output.
    pub fn enqueue(&mut self, audio: bool, payload_len: u16, ipv6: bool, now: f64) {
        self.rates[if audio { 2 } else { 0 }].track(payload_len as u32 + if ipv6 { 48 } else { 28 }, now);
    }
    /// 0x20636d870..8e4: all regular primary AAC packets count; normal camera
    /// video (including parity) has +e2 clear. See the stream-side flag trace in wiring.md.
    /// 0x20636d9b4..9ec: history records use the timestamp actually in the packet.
    pub fn sent(&mut self, audio: bool, payload_len: u16, ipv6: bool, stamp: Option<u16>, now: f64) {
        self.now = now;
        self.tx_history.track_sent_packet(u32::from(!audio), false, audio, 1, payload_len, ipv6);
        self.rates[if audio { 3 } else { 1 }].track(payload_len as u32 + if ipv6 { 48 } else { 28 }, now);
        if let Some(ts) = stamp {
            self.tx_history.add_info(ts, now);
            // Native acknowledges the packet's rate generation at 0x20636d9f0..da10.
            // Our bridge has no applied-generation metadata; leave the applied counter
            // at its initial value instead of pretending the command was encoded.
        }
    }
    pub fn set_time(&mut self, now: f64) { self.now = now; }
}
impl MediaController for SenderMediaController {
    fn observe_decision(&mut self, state: &VcrcState) {
        self.last_decision_status = (state.ramp_up_status, state.ramp_down_status);
        log_decision(state, true);
    }
    fn is_audio_only(&self) -> bool { self.audio_only } // 0x20604c704
    fn last_video_refresh_frame_time(&self) -> f64 { self.last_refresh } // 0x20604ea00
    fn compute_packet_loss(&mut self, input: &PacketLossInput) -> PacketLossOutput {
        // Frame-based redundancy state is UNKNOWN for our V0 sender. Normal camera
        // trafficType=0 chooses the 6-bit layout either way (0x20657527c..2a4).
        self.tx_history.compute_packet_loss(input, false)
    }
    fn media_queue_settings(&self) -> (f64, f64, f64, f64) {
        (self.rates[0].bps(self.now), self.rates[1].bps(self.now), self.rates[2].bps(self.now), self.rates[3].bps(self.now))
    }
    // Applied encoder-generation metadata is UNKNOWN. No generation has been
    // acknowledged by our queue. mode1_camera_defaults.o_208=1 makes the native
    // port use its own counter/system time instead (0x2061fdd28..d68).
    fn media_queue_rate_change_counter(&self) -> (u32, f64) { (self.applied_counter, self.applied_time) }
    fn traffic_type(&self) -> u8 { 0 } // camera, no screen/secondary/persona: 0x205ea9280..290
    fn is_video_stopped(&self) -> bool { self.video_stopped } // 0x20604c8ec..968
    fn set_min_target_bitrate(&mut self, bps: u32) { self.min_target_bitrate = bps; } // 0x20604e6cc
    fn set_should_disable_large_frame_requests_when_initial_ramp_up(&mut self, v: bool) {
        self.disable_large_frame_requests = v; // 0x20604e910; only native large-frame probe scheduling consumes it
    }
    fn set_rate_limited_max_time_exceeded(&mut self, v: bool) { self.rate_limited_max_time_exceeded = v; } // 0x20604e988
}

/// Controller plus feedback counter unwrapping; one instance per U1 camera session.
#[derive(Debug)]
pub struct SenderRateController {
    pub state: VcrcState,
    pub media: SenderMediaController,
    received_audio: u32,
    pub started_at: f64,
    pub(super) participant: Option<u64>,
    last_requested_target: u32,
    received_video: u32,
    pub notify_gate: NotifyGate,
}
impl SenderRateController {
    pub fn new(audio_only: bool, now: f64) -> Self {
        let mut media = SenderMediaController::new(audio_only, now);
        let state = VcrcState::new_layered_continuous_tier(mode1_camera_defaults(), Some(&mut media), now);
        let last_requested_target = state.target_bitrate;
        // ConfigureInternal notifies once right after Configure (0x205eada18); the caller
        // sends that first target to the encoder.
        let mut notify_gate = NotifyGate::default();
        let _ = notify_gate.notify(&state);
        Self { state, media, received_audio: 0, received_video: 0, started_at: now,
            participant: None, last_requested_target, notify_gate }
    }
    /// Uses the same parser as the receive path; exposed for integration fixtures.
    pub fn decode_feedback(&mut self, profile: u16, raw: &[u8], version: u8, now: f64) -> Option<VcrcStatistics> {
        let data = FTAudioControlData::from_ext_with_feedback_version(profile, raw, version)?;
        Some(self.statistics(&data, version, now))
    }
    pub(super) fn statistics(&mut self, data: &FTAudioControlData, version: u8, now: f64) -> VcrcStatistics {
        // Collector unwraps audio's 12-bit cumulative count, keeps packed video unchanged:
        // __AVCStatisticsCollector_UpdatePacketReceivedCount 0x205ed4624..4694, 0x205ed4790..479c.
        let audio = unwrap12(self.received_audio, data.audio_received_packets as u32);
        self.received_audio = audio;
        let video_word = if let Some((frames, lost)) = data.packed_video_loss(version) {
            ((frames as u32) << 6) | lost as u32
        } else { data.video_received_packets.unwrap_or(0) as u32 };
        let video = if audio_feedback::loss_stats_enabled(version) { video_word }
            else { unwrap12(self.received_video, video_word) };
        self.received_video = video;
        let bwe = data.bandwidth_estimate.unwrap_or(0) as u32;
        VcrcStatistics {
            // processFeedbackMessage 0x205eb82bc, 0x205eb8338..348; Process... 0x205eb848c.
            time: now,
            timestamp: data.current_send_timestamp.unwrap_or(0),
            queuing_delay: data.queuing_delay.unwrap_or(0) as u32,
            // UnserializeWithData 0x2064c23f4..408 writes bucket*20 to info+10;
            // ProcessFeedbackControlInfo 0x205eb9af0..b08 divides by 20 -> stats+70.
            receive_jitter_queue_size: data.receive_queue_target_bucket as u32,
            // Collector 0x205ed2200..208 leaves kbps packed word intact and copies low 3 bits.
            bandwidth_estimation: bwe,
            is_bwe_stable: (bwe & 7) as i32,
            // ProcessFeedbackControlInfo 0x205eb9ac8..aec -> Process... 0x205eb8470..490.
            burst_loss: data.video_burst_loss.unwrap_or(0) as u32,
            audio_received_packets: audio,
            video_received_packets: video,
            echoed_timestamp: data.feedback_sequence as u32,
            // RTPUnpackDouble 0x205fa9d4c..960: Q13, with 0xffff neutralized to zero.
            owrd: data.q13_one_way_delay.filter(|v| *v != 0xffff).unwrap_or(0) as f64 / 8192.0,
            // Native type-0 info copies ECN pair to input+a0 (0x205eb9afc..b0c), then stats+b8.
            ecn_stats: data.ect.unwrap_or(0) as u32 | ((data.ce.unwrap_or(0) as u32) << 16),
            // Missing inputs are zero in 0x205eb9a78..a90 / 0x205eb8400..418.
            // See wiring.md: the decoded total-KB word is not copied to stats+3c on this path.
            // RTT is derived inside the unchanged port; actual bitrate is measured by `media`.
            ..Default::default()
        }
    }
    pub fn on_feedback(&mut self, stats: &VcrcStatistics, now: f64) -> Option<u32> {
        self.media.set_time(now);
        let ran = self.state.do_rate_control(stats, Some(&mut self.media), now);
        if !ran { log_decision(&self.state, false); }
        // AVCRateController_DoRateControlWithStatistics 0x205eb2eec reaches
        // NotifyTargetBitrateChange (0x205eb3ba0) after the algorithm on its paths.
        let notify = self.notify_gate.notify(&self.state);
        if let Some(bps) = notify { self.last_requested_target = bps; }
        notify
    }
}

/// The gate in `__AVCRateController_NotifyTargetBitrateChange` 0x205eb261c that decides when
/// a new target reaches the encoder (`VCRateControlMediaController_SetTargetBitrate` and the
/// delegate, 0x205eb2c38..ca8). Mode 1 uses a threshold of 10 rate changes (0x205eb29b8..9d8:
/// only modes 10..13 read the table at 0x2066a3b00). Selectors resolved from the 24A446 shared
/// cache stubs: 0x2080b3960 targetBitrate, 0x2080a41c0 rateChangeCounter, 0x2080b39b0
/// targetBitrateTier, 0x20809e040 isRampingDown (`_state == 6`, 0x20617a1d8).
/// Not applied: the forced Tx/Rx caps (`rateControlForceTxCap/RxCap`, VCDefaults only) and the
/// controller+0x7e2 low-rate reduction (0x205eb2924..95c), which is 0 unless an A/B
/// experiment sets it (0x205ea8538..544).
#[derive(Debug, Default, Clone, Copy)]
pub struct NotifyGate {
    /// controller+0x4dc: last notified target.
    pub last_target: u32,
    /// controller+0x4e4: targetBitrateTier at the last notify.
    pub last_tier: u32,
    /// controller+0x4e8: rateChangeCounter at the last notify.
    pub last_counter: u32,
}
impl NotifyGate {
    /// Returns the target to send to the encoder, or None when Apple would not notify.
    pub fn notify(&mut self, state: &VcrcState) -> Option<u32> {
        let target = state.target_bitrate;
        let counter = state.rate_change_counter;
        let tier = state.target_bitrate_tier;
        // 0x205eb29e0..a10
        let mut go = tier == 0 || state.state == 6 || self.last_tier != tier
            || counter.wrapping_sub(self.last_counter) >= 10;
        // 0x205eb2a14..a38: an unchanged target also needs a new rate-change counter.
        if self.last_target == target { go = go && self.last_counter != counter; }
        if !go { return None; }
        self.last_target = target; // 0x205eb2c38
        self.last_counter = counter; // 0x205eb2c78
        self.last_tier = tier; // 0x205eb2c3c / 0x205eb2ca4
        Some(target)
    }
}
fn unwrap12(previous: u32, count: u32) -> u32 {
    if count.wrapping_sub(previous) & 0x800 != 0 { return previous; }
    let high = if previous & 0xfff > count { previous.wrapping_add(0x1000) } else { previous };
    (high & 0xfffff000) | count
}

fn log_decision(state: &VcrcState, accepted: bool) {
        log::info!("[FTTxRate] apple accepted={} state={} tier={} target_bps={} up={} down={} rampUp=0x{:x} rampDown=0x{:x} rtt={:.6} owrd={:.6} plr_audio={:.6} plr_long={:.6} plr_video={:.6}",
            accepted, state.state, state.current_tier_index, state.target_bitrate,
            state.should_ramp_up_flag, state.should_ramp_down_flag,
            state.ramp_up_status, state.ramp_down_status, state.round_trip_time,
            state.owrd, state.packet_loss_rate_audio, state.packet_loss_rate,
            state.packet_loss_rate_video);
}
