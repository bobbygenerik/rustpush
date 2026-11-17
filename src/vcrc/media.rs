//! The `VCRateControlMediaController` calls the algorithm makes. Apple's implementations sit
//! on the sender's media queue; porting them is a separate step (port-notes.md). Passing no
//! media controller follows Apple's null-pointer branches exactly.

/// Inputs `UpdateInternalStatistics` hands to `VCRateControlMediaController_ComputePacketLoss`
/// 0x20657519c: echoed timestamp, received counts, total kilobytes, burst loss,
/// jitter-queue size, traffic type.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PacketLossInput {
    pub echoed_timestamp: u16,
    pub audio_received_packets: u32,
    pub video_received_packets: u32,
    pub total_received_kbytes: u32,
    pub packet_burst_loss: u16,
    pub receive_jitter_queue_size: u32,
    /// `[mediaController trafficType]` (selector stub 0x2080b3f40).
    pub traffic_type: u8,
}

/// What `ComputePacketLoss` writes through the pointers it is given (mostBurstLoss,
/// packetLossRate, packetLossRateAudio, packetLossRateVideo). `None` leaves a field unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PacketLossOutput {
    pub most_burst_loss: Option<u32>,
    pub packet_loss_rate: Option<f64>,
    pub packet_loss_rate_audio: Option<f64>,
    pub packet_loss_rate_video: Option<f64>,
}

pub trait MediaController {
    /// `VCRateControlMediaController_IsAudioOnly` 0x20604c700.
    fn is_audio_only(&self) -> bool;
    /// `VCRateControlMediaController_LastVideoRefreshFrameTime` 0x20604e9fc, seconds.
    fn last_video_refresh_frame_time(&self) -> f64;
    /// `VCRateControlMediaController_ComputePacketLoss` 0x20657519c.
    fn compute_packet_loss(&mut self, input: &PacketLossInput) -> PacketLossOutput;
    /// `VCRateControlMediaController_MediaQueueSettings` 0x206574d78:
    /// (inVideoBitrate, outVideoBitrate, inAudioBitrate, outAudioBitrate), bps.
    fn media_queue_settings(&self) -> (f64, f64, f64, f64);
    /// `VCRateControlMediaController_MediaQueueRateChangeCounter` 0x206574ff8:
    /// (rate-change counter the queue has applied, Unix time it was applied).
    fn media_queue_rate_change_counter(&self) -> (u32, f64);
    /// `[mediaController trafficType]` (stub 0x2080b3f40).
    fn traffic_type(&self) -> u8;
    /// `VCRateControlMediaController_IsVideoStopped` 0x20604c8e8: any of the controller's
    /// flag bytes +0x38, +0x39, +0x3a set.
    fn is_video_stopped(&self) -> bool;
    /// `VCRateControlMediaController_SetMinTargetBitrate` 0x20604e6c8 (Configure).
    fn set_min_target_bitrate(&mut self, bps: u32);
    /// `VCRateControlMediaController_SetShouldDisableLargeFrameRequestsWhenInitialRampUp`
    /// 0x20604e90c (PostStateTransitionUpdates).
    fn set_should_disable_large_frame_requests_when_initial_ramp_up(&mut self, v: bool);
    /// `VCRateControlMediaController_SetRateLimitedMaxTimeExceeded` 0x20604e984.
    fn set_rate_limited_max_time_exceeded(&mut self, v: bool);
}
