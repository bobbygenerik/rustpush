//! Object state, one field per Apple ivar (offsets: sender-rate-port-20261117/ivars.md).
//! Only fields the ported functions touch are present; more are added as functions are
//! ported. Initial values: zero, as the ObjC allocation leaves them, until the init/Configure
//! functions are ported (chunk 4) and set their values.

use crate::vcrc_config::VcrcConfig;

/// `_owrdList` (+0x528, 0x670 bytes): ring of OWRD samples used by
/// VCRateControlUtil_CalculateCongestionMetrics 0x205f038d8.
#[derive(Clone, Debug, PartialEq)]
pub struct OwrdList {
    /// +0x000: sample times, 100 f64.
    pub times: [f64; 100],
    /// +0x320: sample values (owrd), 100 f64.
    pub values: [f64; 100],
    /// +0x640 u32: front (oldest) index.
    pub front: u32,
    /// +0x644 i32: rear (newest) index.
    pub rear: i32,
    /// +0x648 u32: sample count.
    pub count: u32,
    /// +0x650 f64: NOWRD over the config+0x120 window.
    pub nowrd: f64,
    /// +0x658 f64: NOWRD over the config+0x128 window.
    pub nowrd_short: f64,
    /// +0x660 f64: accumulated NOWRD.
    pub nowrd_acc: f64,
    /// +0x668 u8: set when the window span is below config+0x138 (CalculateNOWRDDetailed).
    pub span_too_short: bool,
}

impl Default for OwrdList {
    fn default() -> Self {
        Self { times: [0.0; 100], values: [0.0; 100], front: 0, rear: 0, count: 0,
            nowrd: 0.0, nowrd_short: 0.0, nowrd_acc: 0.0, span_too_short: false }
    }
}

/// One 32-byte `_lossEventBuffer` record (+0xbc8, 64 records), as written by
/// VCRateControlAlgorithmBasePriv_UpdateLossEvent 0x20617aa94.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LossEvent {
    /// +0x00 f64: rateControlTime.
    pub time: f64,
    /// +0x08 f64: packetLossRate.
    pub packet_loss_rate: f64,
    /// +0x10 f64: packetLossRateVideo if shouldConsiderVideoInLossEvent, else 0.
    pub packet_loss_rate_video: f64,
    /// +0x18 u8: PLR rose by more than 0.001 (0x2066a3958) over a non-zero previous PLR.
    pub bad_trend: bool,
    /// +0x1a u16 (low half of currentECNStats).
    pub ecn_low: u16,
    /// +0x1c u16 (high half of currentECNStats).
    pub ecn_high: u16,
}

/// `VCRateControlAlgorithmLayeredContinuousTier` instance state (Base + StabilizedNOWRD + LCT).
#[derive(Clone, Debug)]
pub struct VcrcState {
    /// +0x28 `_config`.
    pub config: VcrcConfig,
    // ---- VCRateControlAlgorithmBase ----
    /// +0x460 `_paused` (atomic byte; VCRateControlAlgorithm_IsPaused 0x20617a3f8).
    pub paused: bool,
    /// +0x468 `_pauseStartTime`.
    pub pause_start_time: f64,
    /// +0x470 `_state`.
    pub state: i32,
    /// +0x474 `_rampUpStatus`.
    pub ramp_up_status: u32,
    /// +0x478 `_rampDownStatus`.
    pub ramp_down_status: u32,
    /// +0x47c `_currentTierIndex`.
    pub current_tier_index: i32,
    /// +0x488 `_targetBitrateContinuous`.
    pub target_bitrate_continuous: f64,
    /// +0x490 `_targetBitrate`.
    pub target_bitrate: u32,
    /// +0x494 `_targetBitrateTier`.
    pub target_bitrate_tier: u32,
    /// +0x4a0 `_actualBitrate`.
    pub actual_bitrate: u32,
    /// +0x4a4 `_rateChangeCounter`.
    pub rate_change_counter: u32,
    /// +0x4b8 `_rateControlTime`.
    pub rate_control_time: f64,
    /// +0x4c0 `_rampUpFrozenTime`.
    pub ramp_up_frozen_time: f64,
    /// +0x4c8 `_isCongested`.
    pub is_congested: bool,
    /// +0x4d0 `_isNewRateSentOut`.
    pub is_new_rate_sent_out: bool,
    /// +0x4d1 `_isSendBitrateLimited`.
    pub is_send_bitrate_limited: bool,
    /// +0x4d2 `_isFirstInitialRampUpDone`.
    pub is_first_initial_ramp_up_done: bool,
    /// +0x4d3 `_isFirstTimestampArrived`.
    pub is_first_timestamp_arrived: bool,
    /// +0x4d4 `_enableRampUpStuckFix`.
    pub enable_ramp_up_stuck_fix: bool,
    /// +0x4e8 `_lastCongestionTime`.
    pub last_congestion_time: f64,
    /// +0x4f0 `_lastRampDownTimeDueToFeedback`.
    pub last_ramp_down_time_due_to_feedback: f64,
    /// +0x508 `_remoteBandwidthEstimation` (kbps).
    pub remote_bandwidth_estimation: u32,
    /// +0x510 `_lastNoOvershootBWEstimationTime`.
    pub last_no_overshoot_bw_estimation_time: f64,
    /// +0x518 `_firstBelowNoRampUpBandwidthTime`.
    pub first_below_no_ramp_up_bandwidth_time: f64,
    /// +0x520 `_belowNoRampUpBandwidth`.
    pub below_no_ramp_up_bandwidth: bool,
    /// +0x521 `_isOvershoot`.
    pub is_overshoot: bool,
    /// +0x522 `_maxBandwidthEstimationTier` (u8).
    pub max_bandwidth_estimation_tier: u8,
    /// +0x528 `_owrdList`.
    pub owrd_list: OwrdList,
    /// +0xb98 `_isOWRDListReady`.
    pub is_owrd_list_ready: bool,
    /// +0xb99 `_isOWRDConstant`.
    pub is_owrd_constant: bool,
    /// +0xba0 `_owrd`.
    pub owrd: f64,
    /// +0xba8 `_nowrd`.
    pub nowrd: f64,
    /// +0xbb0 `_nowrdShort`.
    pub nowrd_short: f64,
    /// +0xbb8 `_nowrdAcc`.
    pub nowrd_acc: f64,
    /// +0xbc0 `_lastOWRDChangeTime`.
    pub last_owrd_change_time: f64,
    /// +0xbc8 `_lossEventBuffer` (64 records).
    pub loss_event_buffer: [LossEvent; 64],
    /// +0x13c8 `_lossEventBufferIndex`.
    pub loss_event_buffer_index: u32,
    /// +0x13d8 `_firstLossEventRampDownTime`.
    pub first_loss_event_ramp_down_time: f64,
    /// +0x13ec `_currentECNStats`.
    pub current_ecn_stats: u32,
    /// +0x13f0 `_mostBurstLoss`.
    pub most_burst_loss: u32,
    /// +0x13f4 `_receiveJitterQueueSize`.
    pub receive_jitter_queue_size: u32,
    /// +0x13f8 `_packetLossRate`.
    pub packet_loss_rate: f64,
    /// +0x1400 `_previousPacketLossRate`.
    pub previous_packet_loss_rate: f64,
    /// +0x1408 `_previousPacketLossRateAvgFactor`.
    pub previous_packet_loss_rate_avg_factor: f64,
    /// +0x1418 `_packetLossRateAvgLong` (UpdateLossEvent seeds it when it reads -1.0).
    pub packet_loss_rate_avg_long: f64,
    /// +0x1438 `_consecutiveZeroPLRCount`.
    pub consecutive_zero_plr_count: u32,
    /// +0x1440 `_packetLossRateAudio`.
    pub packet_loss_rate_audio: f64,
    /// +0x1448 `_packetLossRateVideo`.
    pub packet_loss_rate_video: f64,
    /// +0x1450 `_mostRecentRoundTripTime`.
    pub most_recent_round_trip_time: f64,
    /// +0x1458 `_roundTripTime`.
    pub round_trip_time: f64,
    /// +0x1460 `_roundTripTimeMinEnvelope`.
    pub round_trip_time_min_envelope: f64,
    /// +0x1468 `_worstRecentRoundTripTime`.
    pub worst_recent_round_trip_time: f64,
    /// +0x1470 `_worstRecentBurstLoss`.
    pub worst_recent_burst_loss: u32,
    /// +0x1478 `_shouldConsiderVideoInLossEvent`.
    pub should_consider_video_in_loss_event: bool,
    /// +0x149b `_abnormalNetworkDetected`.
    pub abnormal_network_detected: bool,
    /// +0x14a0 `_ecnCERatio`.
    pub ecn_ce_ratio: f64,
    /// +0x14f0 `_firstFeedbackArrivalTime`.
    pub first_feedback_arrival_time: f64,
    // ---- VCRateControlAlgorithmStabilizedNOWRD ----
    /// +0x152a `_echoedTimestamp` (u16).
    pub echoed_timestamp: u16,
    /// +0x152c `_isStable`.
    pub is_stable: bool,
    /// +0x152d `_shouldRampUpSkipNOWRDStabilization`.
    pub should_ramp_up_skip_nowrd_stabilization: bool,
    /// +0x1530 `_sendBitrateLimitedStartTime`.
    pub send_bitrate_limited_start_time: f64,
    /// +0x1538 `_inVideoBitrate`.
    pub in_video_bitrate: f64,
    /// +0x1540 `_outVideoBitrate`.
    pub out_video_bitrate: f64,
    /// +0x1548 `_inAudioBitrate`.
    pub in_audio_bitrate: f64,
    /// +0x1550 `_outAudioBitrate`.
    pub out_audio_bitrate: f64,
    /// +0x15b0 `_lastFeedbackTime`.
    pub last_feedback_time: f64,
    /// +0x15b8 `_lastFeedbackTargetBitrate`.
    pub last_feedback_target_bitrate: u32,
    /// +0x15bc `_previousTimestamp` (u16).
    pub previous_timestamp: u16,
    /// +0x15be `_queuingDelayTimestamp` (u16).
    pub queuing_delay_timestamp: u16,
    /// +0x15c0 `_receiveTimestamp` (u32; may hold ts + 0x10000).
    pub receive_timestamp: u32,
    /// +0x15c4 `_timestampWrapAroundCounter`.
    pub timestamp_wrap_around_counter: u32,
    /// +0x15c8 `_lastRateChangeCounter`.
    pub last_rate_change_counter: u32,
    /// +0x15cc `_newOWRDSampleCollected`.
    pub new_owrd_sample_collected: u32,
    /// +0x15d0 `_isRemoteBandwidthEstimationStable`.
    pub is_remote_bandwidth_estimation_stable: bool,
    /// +0xb5e8 `_firstOWRDFrozenTime`.
    pub first_owrd_frozen_time: f64,
    /// +0xb600 `_stabilizationTime`.
    pub stabilization_time: f64,
    /// +0xb608 `_rateChangeSystemTime` (Unix seconds; `micro()` in Apple).
    pub rate_change_system_time: f64,
    /// +0xb618 `_lastAllowRampUpTime`.
    pub last_allow_ramp_up_time: f64,
    /// +0xb620 `_isRampUpSettling`.
    pub is_ramp_up_settling: bool,
    /// +0xb628 `_rampUpSettleDuration`.
    pub ramp_up_settle_duration: f64,
    /// +0xb630 `_shouldSuppressRampDown`.
    pub should_suppress_ramp_down: bool,
    /// +0xb634 `_rampDownSuppressionBitrateThreshold`.
    pub ramp_down_suppression_bitrate_threshold: u32,
    /// +0xb648 `_isWaitingForBasebandRampDown`.
    pub is_waiting_for_baseband_ramp_down: bool,
    /// +0xb664 `_packetBurstLoss`.
    pub packet_burst_loss: u32,
    /// +0xb670 `_previousRoundTripTime`.
    pub previous_round_trip_time: f64,
    /// +0xb678 `_lastNetworkUnstableTime`.
    pub last_network_unstable_time: f64,
    // ---- added for chunk 3 (ShouldRampDown / ShouldRampUp and their helpers) ----
    /// +0x480 `_previousTierIndex`.
    pub previous_tier_index: i32,
    /// `Base._lastLossEventRampDownTime`.
    pub last_loss_event_ramp_down_time: f64,
    /// `Base._didMBLRampDown`.
    pub did_mbl_ramp_down: bool,
    /// `Base._shouldConsiderCongestionLevelForNOWRDAcc` (set by Configure; chunk 4).
    pub should_consider_congestion_level_for_nowrd_acc: bool,
    /// +0x13e0 `_packetLossRateBadTrendCount`.
    pub packet_loss_rate_bad_trend_count: i32,
    /// +0x13e4 `_lossEventBandwidthLimit`.
    pub loss_event_bandwidth_limit: u32,
    /// +0x13e8 `_lossEventBandwidthConfidence`.
    pub loss_event_bandwidth_confidence: u32,
    /// +0x1410 `_packetLossRateThreshold`.
    pub packet_loss_rate_threshold: f64,
    /// +0x1420 `_packetLossRateAvgLongPrevious`.
    pub packet_loss_rate_avg_long_previous: f64,
    /// +0x1428 `_packetLossRateAvgLongAtLBAStart`.
    pub packet_loss_rate_avg_long_at_lba_start: f64,
    /// +0x149a `_didLBARampDown`.
    pub did_lba_ramp_down: bool,
    /// `StabilizedNOWRD._fastRampDownBitrateRange`.
    pub fast_ramp_down_bitrate_range: u32,
    /// `StabilizedNOWRD._previousNOWRD`.
    pub previous_nowrd: f64,
    /// `StabilizedNOWRD._lastPositiveOWRDTime`.
    pub last_positive_owrd_time: f64,
    /// `StabilizedNOWRD._additionalTierForRampDown`.
    pub additional_tier_for_ramp_down: u32,
    /// +0x1430 `_packetLossRateAvgLongAtLBAExit`.
    pub packet_loss_rate_avg_long_at_lba_exit: f64,
    /// `Base._lastRampDownTimeDueToBaseband`.
    pub last_ramp_down_time_due_to_baseband: f64,
    /// `StabilizedNOWRD._extendedRampUpFrozenExitTime`.
    pub extended_ramp_up_frozen_exit_time: f64,
    /// `StabilizedNOWRD._basebandAdaptationEnabled` (cellular modem feedback; not available).
    pub baseband_adaptation_enabled: bool,
    /// `StabilizedNOWRD._basebandAdditionalTiersForRampUp`.
    pub baseband_additional_tiers_for_ramp_up: u32,
    /// `StabilizedNOWRD._withBTCoex`.
    pub with_bt_coex: bool,
    /// `StabilizedNOWRD._bytesInFlightAdaptationEnabled`.
    pub bytes_in_flight_adaptation_enabled: bool,
    /// `StabilizedNOWRD._bytesInFlight`.
    pub bytes_in_flight: u32,
    /// `StabilizedNOWRD._allowRampUpBasedOnBytesInFlight`.
    pub allow_ramp_up_based_on_bytes_in_flight: bool,
    /// `StabilizedNOWRD._history` (1024 × 40-byte records).
    pub history: Box<[HistoryRecord; 1024]>,
    /// `StabilizedNOWRD._currentHistoryIndex`.
    pub current_history_index: u32,
    /// `StabilizedNOWRD._currentHistorySize`.
    pub current_history_size: u32,
    // ---- added for chunk 4 (LayeredContinuousTier state machine, init, Configure) ----
    /// +0x4d8 `_consecutiveRampDown`.
    pub consecutive_ramp_down: u32,
    /// +0x4e0 `_lastTimeStartRampingDown`.
    pub last_time_start_ramping_down: f64,
    /// `Base._rampUpCounter`.
    pub ramp_up_counter: u32,
    /// `Base._rampUpSpeedAdjustmentFactor`.
    pub ramp_up_speed_adjustment_factor: f64,
    /// +0x147c `_rateSharingCount` (Base init: 1).
    pub rate_sharing_count: i32,
    /// +0x1480 `_rateSharingFactor` (Base init: 1.0).
    pub rate_sharing_factor: f64,
    /// `StabilizedNOWRD._rampUpFrozenDuration`.
    pub ramp_up_frozen_duration: f64,
    /// `StabilizedNOWRD._shouldConsiderCongestionLevelForBIF` (StabilizedNOWRD init: 1).
    pub should_consider_congestion_level_for_bif: bool,
    /// `StabilizedNOWRD._bytesInFlightState` (Configure: -1).
    pub bytes_in_flight_state: i32,
    /// `StabilizedNOWRD._bytesInFlightThreshold`.
    pub bytes_in_flight_threshold: u32,
    /// +0xb6b8 `_bandwidthWall` (f64).
    pub bandwidth_wall: f64,
    /// +0xb6c0 `_bandwidthWallConfidence` (32-bit, compared signed).
    pub bandwidth_wall_confidence: i32,
    /// +0xb6c8 `_speedFactor`.
    pub speed_factor: f64,
    /// +0xb6d0 `_previousRateControlTime`.
    pub previous_rate_control_time: f64,
    /// +0xb6d8 `_congestionSeverity` (LCT init: 1.0).
    pub congestion_severity: f64,
    /// +0xb6e0 `_shouldRampUp`.
    pub should_ramp_up_flag: bool,
    /// +0xb6e1 `_shouldRampDown`.
    pub should_ramp_down_flag: bool,
    /// Stand-in for `arc4random_uniform` in GetRampUpFrozenDuration (xorshift64 state).
    /// Only matters when config+0x185 == 1 (0 on the static profile).
    pub rng: u64,
}

/// One `_history` record, as UpdateVCRateControlHistory 0x2061fe564 writes it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HistoryRecord {
    /// +0x00 `_owrd`.
    pub owrd: f64,
    /// +0x08 `_roundTripTime`.
    pub round_trip_time: f64,
    /// +0x10 `_roundTripTimeMinEnvelope`.
    pub round_trip_time_min_envelope: f64,
    /// +0x18 `_packetLossRate`.
    pub packet_loss_rate: f64,
    /// +0x20 `_currentTierIndex`.
    pub tier_index: i32,
    /// +0x24: the function's second argument (w1).
    pub arg: u32,
}

impl VcrcState {
    /// All ivars zero (ObjC allocation), config as given. Not yet Apple's Configure/StateEnter.
    pub fn new(config: VcrcConfig) -> Self {
        Self {
            config,
            paused: false, pause_start_time: 0.0, state: 0, ramp_up_status: 0, ramp_down_status: 0,
            current_tier_index: 0, target_bitrate_continuous: 0.0, target_bitrate: 0,
            target_bitrate_tier: 0, actual_bitrate: 0, rate_change_counter: 0,
            rate_control_time: 0.0, ramp_up_frozen_time: 0.0, is_congested: false,
            is_new_rate_sent_out: false, is_send_bitrate_limited: false,
            is_first_initial_ramp_up_done: false, is_first_timestamp_arrived: false,
            enable_ramp_up_stuck_fix: false, last_congestion_time: 0.0,
            last_ramp_down_time_due_to_feedback: 0.0, remote_bandwidth_estimation: 0,
            last_no_overshoot_bw_estimation_time: 0.0, first_below_no_ramp_up_bandwidth_time: 0.0,
            below_no_ramp_up_bandwidth: false, is_overshoot: false, max_bandwidth_estimation_tier: 0,
            owrd_list: OwrdList::default(), is_owrd_list_ready: false, is_owrd_constant: false,
            owrd: 0.0, nowrd: 0.0, nowrd_short: 0.0, nowrd_acc: 0.0, last_owrd_change_time: 0.0,
            loss_event_buffer: [LossEvent::default(); 64], loss_event_buffer_index: 0,
            first_loss_event_ramp_down_time: 0.0, current_ecn_stats: 0, most_burst_loss: 0,
            receive_jitter_queue_size: 0, packet_loss_rate: 0.0, previous_packet_loss_rate: 0.0,
            previous_packet_loss_rate_avg_factor: 0.0, packet_loss_rate_avg_long: 0.0,
            consecutive_zero_plr_count: 0, packet_loss_rate_audio: 0.0, packet_loss_rate_video: 0.0,
            most_recent_round_trip_time: 0.0, round_trip_time: 0.0, round_trip_time_min_envelope: 0.0,
            worst_recent_round_trip_time: 0.0, worst_recent_burst_loss: 0,
            should_consider_video_in_loss_event: false, abnormal_network_detected: false,
            ecn_ce_ratio: 0.0, first_feedback_arrival_time: 0.0,
            echoed_timestamp: 0, is_stable: false, should_ramp_up_skip_nowrd_stabilization: false,
            send_bitrate_limited_start_time: 0.0, in_video_bitrate: 0.0, out_video_bitrate: 0.0,
            in_audio_bitrate: 0.0, out_audio_bitrate: 0.0, last_feedback_time: 0.0,
            last_feedback_target_bitrate: 0, previous_timestamp: 0, queuing_delay_timestamp: 0,
            receive_timestamp: 0, timestamp_wrap_around_counter: 0, last_rate_change_counter: 0,
            new_owrd_sample_collected: 0, is_remote_bandwidth_estimation_stable: false,
            first_owrd_frozen_time: 0.0, stabilization_time: 0.0, rate_change_system_time: 0.0,
            last_allow_ramp_up_time: 0.0, is_ramp_up_settling: false, ramp_up_settle_duration: 0.0,
            should_suppress_ramp_down: false, ramp_down_suppression_bitrate_threshold: 0,
            is_waiting_for_baseband_ramp_down: false, packet_burst_loss: 0,
            previous_round_trip_time: 0.0, last_network_unstable_time: 0.0,
            previous_tier_index: 0, last_loss_event_ramp_down_time: 0.0, did_mbl_ramp_down: false,
            should_consider_congestion_level_for_nowrd_acc: false,
            packet_loss_rate_bad_trend_count: 0, loss_event_bandwidth_limit: 0,
            loss_event_bandwidth_confidence: 0, packet_loss_rate_threshold: 0.0,
            packet_loss_rate_avg_long_previous: 0.0, packet_loss_rate_avg_long_at_lba_start: 0.0,
            did_lba_ramp_down: false, fast_ramp_down_bitrate_range: 0, previous_nowrd: 0.0,
            last_positive_owrd_time: 0.0, additional_tier_for_ramp_down: 0,
            packet_loss_rate_avg_long_at_lba_exit: 0.0, last_ramp_down_time_due_to_baseband: 0.0,
            extended_ramp_up_frozen_exit_time: 0.0, baseband_adaptation_enabled: false,
            baseband_additional_tiers_for_ramp_up: 0, with_bt_coex: false,
            bytes_in_flight_adaptation_enabled: false, bytes_in_flight: 0,
            allow_ramp_up_based_on_bytes_in_flight: false,
            history: Box::new([HistoryRecord::default(); 1024]),
            current_history_index: 0, current_history_size: 0,
            consecutive_ramp_down: 0, last_time_start_ramping_down: 0.0, ramp_up_counter: 0,
            ramp_up_speed_adjustment_factor: 0.0, rate_sharing_count: 0, rate_sharing_factor: 0.0,
            ramp_up_frozen_duration: 0.0, should_consider_congestion_level_for_bif: false,
            bytes_in_flight_state: 0, bytes_in_flight_threshold: 0, bandwidth_wall: 0.0,
            bandwidth_wall_confidence: 0, speed_factor: 0.0, previous_rate_control_time: 0.0,
            congestion_severity: 0.0, should_ramp_up_flag: false, should_ramp_down_flag: false,
            rng: 0x9e3779b97f4a7c15,
        }
    }

    /// `config+8` tier table lookup.
    pub fn tier(&self, index: i32) -> u32 {
        self.config.o_8[index as usize]
    }
}
