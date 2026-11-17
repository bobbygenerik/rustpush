//! The `VCStatistics` fields `VCRateControlAlgorithmStabilizedNOWRDPriv_UpdateInternalStatistics`
//! (0x2061fd3b4) reads, at their native offsets. Units are Apple's.

/// One feedback report as the algorithm sees it (statistics type 2, "VCRC").
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VcrcStatistics {
    /// +0x08 f64: local arrival time of the report, Unix seconds.
    pub time: f64,
    /// +0x18 u16: the report's timestamp, 1/1024 s ticks, wraps every 64 s.
    pub timestamp: u16,
    /// +0x1c (low 16 bits used): queuing delay echoed by the receiver, 1/1024 s ticks.
    pub queuing_delay: u32,
    /// +0x20 u32: receiver's bandwidth estimate, kbps (0 = none).
    pub bandwidth_estimation: u32,
    /// +0x24 i32: receiver's bandwidth estimate is stable (non-zero).
    pub is_bwe_stable: i32,
    /// +0x28 u32: burst loss.
    pub burst_loss: u32,
    /// +0x30 u32: most burst loss.
    pub most_burst_loss: u32,
    /// +0x34 u32: audio packets received (as reported).
    pub audio_received_packets: u32,
    /// +0x38 u32: video packets received / packed video loss word (as reported).
    pub video_received_packets: u32,
    /// +0x3c u32: total kilobytes received.
    pub total_received_kbytes: u32,
    /// +0x44 (low 16 bits used): our send timestamp echoed back, 1/1024 s ticks.
    pub echoed_timestamp: u32,
    /// +0x50 f64: one-way relative delay, seconds.
    pub owrd: f64,
    /// +0x58 f64: packet loss rate, used only when there is no media controller.
    pub packet_loss_rate: f64,
    /// +0x60 u32: actual bitrate, bps.
    pub actual_bitrate: u32,
    /// +0x68 f64: round-trip time, seconds (0 = derive from timestamps).
    pub round_trip_time: f64,
    /// +0x70 u32: receiver jitter-queue size.
    pub receive_jitter_queue_size: u32,
    /// +0xb8 u32: ECN statistics, stored as-is into the loss-event record at +0x1a by
    /// UpdateLossEvent 0x20617aa94 (record +0x1a = low half, +0x1c = high half).
    pub ecn_stats: u32,
}
