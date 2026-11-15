//! Port of Apple AVConference `VCRateControlBandwidthEstimator`,
//! `VCRateControlBandwidthEstimatorMap`, `SuddenChangeParameters`, and the
//! `AVCStatisticsCollector` BWE readout/quantization helpers from iOS 27.0.1
//! (24A446, iPhone17,5).
//!
//! NOT wired into `avconference.rs` or any live call path in this slice.

use std::collections::HashMap;

/// `SuddenChangeParameters` (AVConference, iOS 27.0.1 24A446).
///
/// Accessors at `0x2063b92dc..0x2063b9344`.
#[derive(Debug, Clone, PartialEq)]
pub struct SuddenChangeParameters {
    /// `_margin` (`+0x08`, `int`)
    pub margin: i32,
    /// `_minSamplesCount` (`+0x0c`, `int`)
    pub min_samples_count: i32,
    /// `_minSamplesCountFastBandwidthEstimation` (`+0x10`, `int`)
    pub min_samples_count_fast_bandwidth_estimation: i32,
    /// `_minMarginRequired` (`+0x14`, `int`)
    pub min_margin_required: i32,
    /// `_percentage` (`+0x18`, `double`)
    pub percentage: f64,
    /// `_durationThresholdUpward` (`+0x20`, `double`)
    pub duration_threshold_upward: f64,
    /// `_durationThresholdDownward` (`+0x28`, `double`)
    pub duration_threshold_downward: f64,
}

impl Default for SuddenChangeParameters {
    fn default() -> Self {
        Self {
            margin: 0,
            min_samples_count: 0,
            min_samples_count_fast_bandwidth_estimation: 0,
            min_margin_required: 0,
            percentage: 0.0,
            duration_threshold_upward: 0.0,
            duration_threshold_downward: 0.0,
        }
    }
}

/// Server-bag parameters read by `-[VCRateControlBandwidthEstimator setServerBag:]`
/// (`0x2063b97c4`) under the `"bwEstimation"` group.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerBagBwEstimationParams {
    /// `"aggModeEnabled"` -> `_fastSuddenBandwidthDetectionEnabled` (`+0x1a4`)
    pub agg_mode_enabled: bool,
    /// `"minProbingSequenceSize"` -> `_minProbingSequenceSize` (`+0x1b0`)
    pub min_probing_sequence_size: u32,
    /// `"suddenBWDPercentage"` -> `_suddenChangeParameters.percentage`
    pub sudden_bwd_percentage: f64,
    /// `"minFastBWDSampleCount"` -> `_suddenChangeParameters.minSamplesCountFastBandwidthEstimation`
    pub min_fast_bwd_sample_count: i32,
}

/// `VCRateControlBandwidthEstimator` (AVConference, iOS 27.0.1 24A446).
///
/// Field names and offsets match `docs/apple-parity/ios27/evidence/rate-control-slice3-20261114/bwe-fields.md`.
#[derive(Debug, Clone)]
pub struct VCRateControlBandwidthEstimator {
    /// `_totalBytesInProbingSequence` (`+0x08`, `u32`)
    pub total_bytes_in_probing_sequence: u32,
    /// `_totalPacketsInProbingSequence` (`+0x0c`, `u32`)
    pub total_packets_in_probing_sequence: u32,
    /// `_lastProbingSequenceReferenceTime` (`+0x10`, `f64`)
    pub last_probing_sequence_reference_time: f64,
    /// `_lastProbingSequenceArrivalTime` (`+0x18`, `f64`)
    pub last_probing_sequence_arrival_time: f64,
    /// `_referencePacketArrivalTime` (`+0x20`, `f64`)
    pub reference_packet_arrival_time: f64,
    /// `_referencePacketTransmissionTime` (`+0x28`, `f64`)
    pub reference_packet_transmission_time: f64,
    /// `_earlyProbingPacketArrivalTime` (`+0x30`, `f64`)
    pub early_probing_packet_arrival_time: f64,
    /// `_earlyBytesInProbingSequence` (`+0x38`, `u32`)
    pub early_bytes_in_probing_sequence: u32,
    /// `_earlyPacketsInProbingSequence` (`+0x3c`, `u32`)
    pub early_packets_in_probing_sequence: u32,
    /// `_earlyProbingPacketSize` (`+0x40`, `u32`)
    pub early_probing_packet_size: u32,
    /// `_isInitialized` (`+0x44`, `bool`)
    pub is_initialized: bool,
    /// `_isFirstPacket` (`+0x45`, `bool`)
    /// Note: in `UpdateBandwidthEstimation` (`0x2063b9f20`), for RAT 5/6 when `false`,
    /// performs initial 8-packet / 7s averaging before setting `true`.
    pub is_first_packet: bool,
    /// `_lastProbingPacketTimestamp` (`+0x48`, `u32`)
    pub last_probing_packet_timestamp: u32,
    /// `_lastProbingSequenceID` (`+0x4c`, `u32`)
    pub last_probing_sequence_id: u32,
    /// `_firstDivergeTime` (`+0x50`, `f64`)
    pub first_diverge_time: f64,
    /// `_bandwidthEstimationState` (`+0x58`, `i32`):
    /// `0` = steady, `>0` = diverging upward count, `<0` = diverging downward count.
    pub bandwidth_estimation_state: i32,
    /// `_divergedBandwidthEstimationCount` (`+0x5c`, `i32`), index into `+0x60` (max 30).
    pub diverged_bandwidth_estimation_count: i32,
    /// `_divergedBandwidthEstimations` (`+0x60..+0x150`, `[f64; 30]`).
    pub diverged_bandwidth_estimations: [f64; 30],
    /// `_overRangeCount` (`+0x150`, `u32`)
    pub over_range_count: u32,
    /// `_totalBandwidth` (`+0x158`, `f64`)
    pub total_bandwidth: f64,
    /// `_suddenChangeParameters` (`+0x160`, `SuddenChangeParameters`)
    pub sudden_change_parameters: SuddenChangeParameters,
    /// `_minPacketsCount` (`+0x168`, `u32`)
    pub min_packets_count: u32,
    /// `_minEstimationWindowDuration` (`+0x170`, `f64`)
    pub min_estimation_window_duration: f64,
    /// `_maxDetectableBandwidth` (`+0x178`, `f64`)
    pub max_detectable_bandwidth: f64,
    /// `_maxOverRangeCount` (`+0x180`, `u32`)
    pub max_over_range_count: u32,
    /// `_estimatedBandwidth` (`+0x188`, `f64`)
    pub estimated_bandwidth: f64,
    /// `_estimatedBandwidthUncapped` (`+0x190`, `f64`)
    pub estimated_bandwidth_uncapped: f64,
    /// `_radioAccessTechnology` (`+0x198`, `u32`)
    pub radio_access_technology: u32,
    /// `_mode` (`+0x19c`, `u32`)
    pub mode: u32,
    /// `_suddenChangeState` (`+0x1a0`, `i32`):
    /// `0` = init, `1` = normal update, `2` = upward ramp, `3` = downward ramp,
    /// `4` = upward sudden change applied, `5` = downward sudden change applied, `6` = ignored (`<= 0`).
    pub sudden_change_state: i32,
    /// `_fastSuddenBandwidthDetectionEnabled` (`+0x1a4`, `bool`)
    pub fast_sudden_bandwidth_detection_enabled: bool,
    /// `_l4sHighDataRateEnabled` (`+0x1a5`, `bool`)
    pub l4s_high_data_rate_enabled: bool,
    /// `_minProbingSequenceSize` (`+0x1b0`, `u32`)
    pub min_probing_sequence_size: u32,
    /// Optional override for `VCDefaults_GetIntValueForKey("forcedInitialBandwidthEstimation", 0)`
    /// (`0x2063ba0f4`). Defaults to `0` (disabled).
    pub forced_initial_bandwidth_estimation: u32,
}

impl Default for VCRateControlBandwidthEstimator {
    fn default() -> Self {
        Self::new()
    }
}

impl VCRateControlBandwidthEstimator {
    /// `-[VCRateControlBandwidthEstimator init]` (`0x2063b934c`).
    pub fn new() -> Self {
        let mut sudden_change_parameters = SuddenChangeParameters::default();
        // 0x2063b9384..0x2063b9398
        sudden_change_parameters.duration_threshold_upward = 0.5;
        sudden_change_parameters.duration_threshold_downward = 10.0;

        Self {
            total_bytes_in_probing_sequence: 0,
            total_packets_in_probing_sequence: 0,
            last_probing_sequence_reference_time: 0.0,
            last_probing_sequence_arrival_time: 0.0,
            reference_packet_arrival_time: 0.0,
            reference_packet_transmission_time: 0.0,
            early_probing_packet_arrival_time: 0.0,
            early_bytes_in_probing_sequence: 0,
            early_packets_in_probing_sequence: 0,
            early_probing_packet_size: 0,
            is_initialized: false,
            is_first_packet: false,
            last_probing_packet_timestamp: 0,
            last_probing_sequence_id: 0,
            first_diverge_time: 0.0,
            bandwidth_estimation_state: 0,
            diverged_bandwidth_estimation_count: 0,
            diverged_bandwidth_estimations: [0.0; 30],
            over_range_count: 0,
            total_bandwidth: 0.0,
            sudden_change_parameters,
            min_packets_count: 0,
            min_estimation_window_duration: 0.0,
            max_detectable_bandwidth: 0.0,
            max_over_range_count: 0,
            estimated_bandwidth: 0.0,
            estimated_bandwidth_uncapped: 0.0,
            radio_access_technology: 0,
            mode: 0,
            sudden_change_state: 0,
            fast_sudden_bandwidth_detection_enabled: false,
            l4s_high_data_rate_enabled: false,
            // 0x2063b939c: mov w8, #0xfa; str w8, [x19, #0x1b0]
            min_probing_sequence_size: 250,
            forced_initial_bandwidth_estimation: 0,
        }
    }

    /// `-[VCRateControlBandwidthEstimator setMode:]` (`0x2063b9470`).
    pub fn set_mode(&mut self, mode: u32) {
        self.mode = mode;
        self.update_qualification_parameters();
    }

    /// `_VCRateControlBandwidthEstimator_SetRadioAccessTechnology` (`0x2063b96cc`).
    pub fn set_radio_access_technology(&mut self, rat: u32) {
        self.radio_access_technology = rat;
        self.update_qualification_parameters();
    }

    /// `_VCRateControlBandwidthEstimator_SetEstimatedBandwidth` (`0x2063b974c`).
    pub fn set_estimated_bandwidth(&mut self, bandwidth: f64) {
        self.estimated_bandwidth = bandwidth;
    }

    /// `_VCRateControlBandwidthEstimator_SetFastSuddenBandwidthDetectionEnabled` (`0x2063b9a98`).
    pub fn set_fast_sudden_bandwidth_detection_enabled(&mut self, enabled: bool) {
        self.fast_sudden_bandwidth_detection_enabled = enabled;
        self.update_qualification_parameters();
    }

    /// `_VCRateControlBandwidthEstimator_SetL4SHighDataRateEnabled` (`0x2063b9b18`).
    pub fn set_l4s_high_data_rate_enabled(&mut self, enabled: bool) {
        self.l4s_high_data_rate_enabled = enabled;
        self.update_qualification_parameters();
    }

    /// `-[VCRateControlBandwidthEstimator setServerBag:]` (`0x2063b97c4`).
    pub fn set_server_bag_params(&mut self, params: &ServerBagBwEstimationParams) {
        self.fast_sudden_bandwidth_detection_enabled = params.agg_mode_enabled;
        self.min_probing_sequence_size = params.min_probing_sequence_size;
        self.sudden_change_parameters.percentage = params.sudden_bwd_percentage;
        self.sudden_change_parameters
            .min_samples_count_fast_bandwidth_estimation = params.min_fast_bwd_sample_count;
    }

    /// `_VCRateControlBandwidthEstimator_UpdateQualificationParameters` (`0x2063b9478`).
    pub fn update_qualification_parameters(&mut self) {
        let rat = self.radio_access_technology;
        let mode = self.mode;

        // 0x2063b9484..0x2063b9490: sub w9, w8, #5; cmp w9, #2; b.hs loc_2063b94c8
        if rat == 5 || rat == 6 {
            // 0x2063b9494..0x2063b94c4
            self.max_detectable_bandwidth = if mode == 3 { 800_000.0 } else { 400_000.0 };
            self.min_estimation_window_duration = 0.002; // 0x3f60624dd2f1a9fc
            self.max_over_range_count = 0;
        } else {
            // 0x2063b94c8..0x2063b95c4
            let mut min_win = 0.008; // 0x3f80624dd2f1a9fc
            let mut max_over_range = if self.fast_sudden_bandwidth_detection_enabled {
                0
            } else {
                3
            };
            let max_bw = match mode {
                3 => 5_000_000.0,
                4 => 6_500_000.0,
                5 => 10_000_000.0,
                8 | 9 => 60_000_000.0, // 0x2066adb38
                10 => f64::from_bits(0x41ae_3edb_e000_0000), // 253_717_984.0
                12 | 13 => {
                    // 0x2063b959c..0x2063b95b0: both minWin (0x2066adb30) and maxBW (0x2066adb38)
                    min_win = 0.002; // 0x2066adb30 = 0x3f60624dd2f1a9fc
                    max_over_range = 1;
                    60_000_000.0 // 0x2066adb38
                }
                _ => {
                    if self.l4s_high_data_rate_enabled {
                        23_000_000.0 // 0x2066adb48
                    } else {
                        6_500_000.0 // 0x2066adb40
                    }
                }
            };
            self.min_estimation_window_duration = min_win;
            self.max_detectable_bandwidth = max_bw;
            self.max_over_range_count = max_over_range;
        }

        // 0x2063b95c8: mov w8, #1; str w8, [x0, #0x168]
        self.min_packets_count = 1;
    }

    /// `_VCRateControlBandwidthEstimator_UpdateBandwidthEstimation` (`0x2063b9ecc`).
    pub fn update_bandwidth_estimation(
        &mut self,
        bandwidth_sample: f64,
        arrival_time: f64,
    ) -> f64 {
        // 0x2063b9ef8..0x2063b9efc: fcmp d0, #0.0; b.le loc_2063b9fa0
        if bandwidth_sample <= 0.0 {
            self.sudden_change_state = 6;
            return bandwidth_sample;
        }

        // 0x2063b9f04..0x2063b9f0c: cap by _maxDetectableBandwidth (+0x178)
        let bwe = if bandwidth_sample < self.max_detectable_bandwidth {
            bandwidth_sample
        } else {
            self.max_detectable_bandwidth
        };

        let rat = self.radio_access_technology;
        let is_rat_5_or_6 = rat == 5 || rat == 6;

        // 0x2063b9f14..0x2063b9f24:
        // If RAT is 5 or 6 AND !is_first_packet (`+0x45`), perform initial averaging (`loc_2063ba210`).
        if is_rat_5_or_6 && !self.is_first_packet {
            // 0x2063ba210..0x2063ba21c: d0 holds _maxDetectableBandwidth (`[x19, #0x178]`)
            let half_max = self.max_detectable_bandwidth * 0.5;
            let sample_for_avg = if bwe < half_max { bwe } else { half_max };
            let prev_state = self.bandwidth_estimation_state;
            let new_state = prev_state + 1;
            self.bandwidth_estimation_state = new_state;
            let total = self.total_bandwidth + sample_for_avg;
            self.total_bandwidth = total;

            // 0x2063ba238..0x2063ba250: cmp w9, #8; b.gt loc_2063ba254; else if (arrival_time - first_diverge_time) >= 7.0 -> loc_2063ba254
            if prev_state > 8 || (arrival_time - self.first_diverge_time) >= 7.0 {
                self.is_first_packet = true;
                self.estimated_bandwidth = total / (new_state as f64);
                self.first_diverge_time = 0.0;
                self.bandwidth_estimation_state = 0;
            }
            self.sudden_change_state = 0;
            return bwe;
        }

        // 0x2063b9f28..0x2063b9ff8: configure SuddenChangeParameters
        // Default: margin = 200000 (0x30d40), minSamplesCount = 3, minMarginRequired = 0,
        // durationThresholdUpward = 0.0, durationThresholdDownward = 0.0
        self.sudden_change_parameters.margin = 0x30d40; // 200_000
        self.sudden_change_parameters.min_samples_count = 3;
        self.sudden_change_parameters.min_margin_required = 0;
        self.sudden_change_parameters.duration_threshold_upward = 0.0;
        self.sudden_change_parameters.duration_threshold_downward = 0.0;

        if rat == 5 || rat == 6 {
            // 0x2063b9f78..0x2063b9f9c
            self.sudden_change_parameters.min_samples_count = 6;
            self.sudden_change_parameters.min_margin_required = 0x186a0; // 100_000
            self.sudden_change_parameters.duration_threshold_upward = 10.0;
            self.sudden_change_parameters.duration_threshold_downward = 7.0;
        } else if rat == 1 {
            // 0x2063b9fcc..0x2063b9ff8
            self.sudden_change_parameters.min_samples_count = 4;
            self.sudden_change_parameters.duration_threshold_upward = 4.0;
            self.sudden_change_parameters.duration_threshold_downward = 4.0;
        }

        // 0x2063b9ffc..0x2063ba0c4: compute down/up divergence flags (w21 = down, w23 = up)
        let pct = self.sudden_change_parameters.percentage;
        let est = self.estimated_bandwidth;
        let min_margin = self.sudden_change_parameters.min_margin_required as f64;
        let margin = self.sudden_change_parameters.margin as f64;

        let mut is_down_pct = if bwe <= (1.0 - pct) * est {
            true
        } else {
            (est - bwe) > margin
        };
        let mut is_up_pct = if bwe >= (1.0 + pct) * est {
            true
        } else {
            (bwe - est) > margin
        };

        if is_down_pct {
            is_down_pct = (est - bwe) > min_margin;
        }
        let is_up = if is_up_pct {
            (bwe - est) > min_margin
        } else {
            false
        };
        let is_down = is_down_pct;

        // 0x2063ba0c8..0x2063ba1a4: initial estimation when est == 0.0
        if est == 0.0 {
            let init_floor = 100_000.0; // 0x2066a3f88
            // 0x2063ba0d8..0x2063ba0dc: fcmp d8, d0; fcsel d0, d0, d8, mi -> MAX(bwe, 100_000.0)
            self.estimated_bandwidth = if bwe < init_floor { init_floor } else { bwe };
            self.sudden_change_state = 0;
            if self.forced_initial_bandwidth_estimation != 0 {
                self.estimated_bandwidth = self.forced_initial_bandwidth_estimation as f64;
            }
            return bwe;
        }

        // 0x2063ba1a8..0x2063ba1ac: neither up nor down -> normal EWMA (`loc_2063ba278`)
        if !is_down && !is_up {
            let updated = if !is_rat_5_or_6 {
                // 0x2063ba280..0x2063ba294: 0.1 * bwe + 0.9 * est
                est * 0.9 + bwe * 0.1
            } else {
                // 0x2063ba298..0x2063ba2f0: drain early diverged array, then 0.075 * bwe + 0.925 * est
                let mut cur_est = est;
                let count = self.diverged_bandwidth_estimation_count;
                if count >= 1 {
                    for i in 0..(count as usize) {
                        cur_est = cur_est * 0.95 + self.diverged_bandwidth_estimations[i] * 0.05;
                        self.estimated_bandwidth = cur_est;
                    }
                }
                self.diverged_bandwidth_estimation_count = 0;
                cur_est * 0.925 + bwe * 0.075
            };
            self.estimated_bandwidth = updated;
            self.bandwidth_estimation_state = 0;
            self.total_bandwidth = 0.0;
            self.sudden_change_state = 1;
            return bwe;
        }

        // 0x2063ba1b0..0x2063ba1d4: for RAT 5/6, record diverged sample up to 30 entries
        if is_rat_5_or_6 {
            let idx = self.diverged_bandwidth_estimation_count;
            if idx <= 29 {
                self.diverged_bandwidth_estimations[idx as usize] = bwe;
                self.diverged_bandwidth_estimation_count = idx + 1;
            }
        }

        // 0x2063ba1d4..0x2063ba310: duration threshold check for RAT in {1, 5, 6} (bitmask 0x62)
        let mut duration_ok = true;
        if rat <= 6 && ((0x62u32 >> rat) & 1) != 0 {
            let dt = arrival_time - self.first_diverge_time;
            let threshold = if self.bandwidth_estimation_state < 0 {
                self.sudden_change_parameters.duration_threshold_downward
            } else {
                self.sudden_change_parameters.duration_threshold_upward
            };
            duration_ok = dt >= threshold;
        }

        // 0x2063ba314..0x2063ba36c: update _bandwidthEstimationState and _suddenChangeState
        let mut state = self.bandwidth_estimation_state;
        if is_down {
            if state < 0 {
                state -= 1;
            } else {
                self.total_bandwidth = 0.0;
                self.first_diverge_time = arrival_time;
                state = -1;
            }
            self.bandwidth_estimation_state = state;
            self.sudden_change_state = 3;
        } else if is_up {
            if state > 0 {
                state += 1;
            } else {
                self.total_bandwidth = 0.0;
                self.first_diverge_time = arrival_time;
                state = 1;
            }
            self.bandwidth_estimation_state = state;
            self.sudden_change_state = 2;
        }

        // 0x2063ba370..0x2063ba378
        self.total_bandwidth += bwe;
        let abs_state = state.unsigned_abs() as i32;

        // 0x2063ba384..0x2063ba410: trigger sudden change if count >= minSamplesCount && duration_ok,
        // or if fast_sudden_bandwidth_detection_enabled && count >= minSamplesCountFastBandwidthEstimation
        let normal_trigger =
            abs_state >= self.sudden_change_parameters.min_samples_count && duration_ok;
        let fast_trigger = self.fast_sudden_bandwidth_detection_enabled
            && abs_state
                >= self
                    .sudden_change_parameters
                    .min_samples_count_fast_bandwidth_estimation;

        if normal_trigger || fast_trigger {
            let mut new_est = self.total_bandwidth / (abs_state as f64);
            // 0x2063ba3ac..0x2063ba3d0: if RAT 5/6 and state >= 0, blend 0.9 * new_est + 0.1 * est
            if is_rat_5_or_6 && state >= 0 {
                new_est = self.estimated_bandwidth * 0.1 + new_est * 0.9;
            }
            self.estimated_bandwidth = new_est;
            // 0x2063ba3d8..0x2063ba3e4: state < 0 -> 5 (down), state >= 0 -> 4 (up)
            self.sudden_change_state = if state < 0 { 5 } else { 4 };
            self.bandwidth_estimation_state = 0;
            self.total_bandwidth = 0.0;
        }

        bwe
    }

    /// `_VCRateControlBandwidthEstimator_CalculateBandwidthEstimationWithSendTime` (`0x2063b9c54`).
    pub fn calculate_bandwidth_estimation_with_send_time(
        &mut self,
        arrival_time: f64,
        send_time: f64,
        packet_size: u32,
        is_probing: bool,
        sequence_id: u32,
        probing_id: u32,
    ) {
        // 0x2063b9c94..0x2063b9cb0
        if !self.is_initialized {
            self.is_initialized = true;
            self.is_first_packet = false;
            self.last_probing_sequence_id = sequence_id.wrapping_sub(is_probing as u32);
            self.first_diverge_time = arrival_time;
            self.bandwidth_estimation_state = 0;
            self.diverged_bandwidth_estimation_count = 0;
        }

        // 0x2063b9cb4..0x2063b9d30
        if is_probing && sequence_id == self.last_probing_sequence_id {
            // Same probing sequence: accumulate bytes and packets
            self.last_probing_sequence_reference_time = arrival_time;
            self.last_probing_sequence_arrival_time = send_time;
            self.total_bytes_in_probing_sequence = self
                .total_bytes_in_probing_sequence
                .wrapping_add(packet_size);
            self.total_packets_in_probing_sequence =
                self.total_packets_in_probing_sequence.wrapping_add(1);
            // 0x2063b9ce4..0x2063b9cf0 -> jumps to log & stores lastProbingPacketTimestamp
            self.last_probing_packet_timestamp = sequence_id;
            return;
        }

        // 0x2063b9cf4..0x2063b9db8: finish previous probing sequence
        let total_bytes = self.total_bytes_in_probing_sequence;
        let total_packets = self.total_packets_in_probing_sequence;
        let (bwe, send_bwe) = if total_bytes > self.min_probing_sequence_size {
            let duration =
                self.last_probing_sequence_reference_time - self.reference_packet_arrival_time;
            let send_duration =
                self.last_probing_sequence_arrival_time - self.reference_packet_transmission_time;
            let bits = (total_bytes << 3) as f64;
            let rx_bwe = if duration > 0.0 {
                bits / duration
            } else {
                self.max_detectable_bandwidth
            };
            let tx_bwe = if send_duration > 0.0 {
                bits / send_duration
            } else {
                self.max_detectable_bandwidth
            };
            (rx_bwe, tx_bwe)
        } else {
            (0.0, 0.0)
        };

        let duration_for_check = if total_bytes > self.min_probing_sequence_size {
            self.last_probing_sequence_reference_time - self.reference_packet_arrival_time
        } else {
            0.0
        };

        // 0x2063b9db8..0x2063b9dd8: reset sequence & start new if is_probing
        self.total_bytes_in_probing_sequence = 0;
        self.total_packets_in_probing_sequence = 0;
        self.last_probing_sequence_id = if is_probing { sequence_id } else { 0 };
        let ref_arr = if is_probing { arrival_time } else { 0.0 };
        let ref_tx = if is_probing { send_time } else { 0.0 };
        self.reference_packet_arrival_time = ref_arr;
        self.reference_packet_transmission_time = ref_tx;
        self.early_probing_packet_size = probing_id;
        self.last_probing_sequence_reference_time = ref_arr;
        self.last_probing_sequence_arrival_time = ref_tx;

        // 0x2063b9ddc..0x2063b9ec8
        if bwe > 0.0 {
            self.estimated_bandwidth_uncapped = bwe;
            let passed_bwe = if self.estimated_bandwidth == 0.0 {
                bwe
            } else if total_packets >= self.min_packets_count
                && total_bytes >= self.min_probing_sequence_size
            {
                if duration_for_check < self.min_estimation_window_duration {
                    self.over_range_count += 1;
                    if self.over_range_count >= self.max_over_range_count {
                        bwe
                    } else {
                        0.0
                    }
                } else {
                    self.over_range_count = 0;
                    bwe
                }
            } else {
                0.0
            };

            // 0x2063b9df8..0x2063b9e18: if send_bwe < 10_000_000.0 && send_bwe > 0.0, pass 0.0
            let final_bwe = if send_bwe < 10_000_000.0 && send_bwe > 0.0 {
                0.0
            } else {
                passed_bwe
            };
            self.update_bandwidth_estimation(final_bwe, arrival_time);
        }

        self.last_probing_packet_timestamp = sequence_id;
    }

    /// `_VCRateControlBandwidthEstimator_CalculateBandwidthEstimation` (`0x2063ba414`).
    pub fn calculate_bandwidth_estimation(
        &mut self,
        sample_time: u32,
        packet_size: u32,
        is_probing: bool,
        packet_id: u32,
        is_last_probing_packet: bool,
        arrival_time: f64,
    ) {
        // 0x2063ba45c..0x2063ba480
        if !self.is_initialized {
            self.is_initialized = true;
            self.is_first_packet = false;
            self.last_probing_sequence_id = if is_probing {
                packet_id.wrapping_sub(1)
            } else {
                sample_time
            };
            self.first_diverge_time = arrival_time;
            self.bandwidth_estimation_state = 0;
            self.diverged_bandwidth_estimation_count = 0;
        }

        let mut total_bytes: u32;
        let mut total_packets: u32;
        let mut duration = 0.0;
        let mut bwe = 0.0;

        // 0x2063ba484..0x2063ba490
        if is_probing && packet_id == self.last_probing_sequence_id {
            // Same probing sequence
            let end_time = if arrival_time < self.last_probing_sequence_reference_time {
                self.last_probing_sequence_reference_time
            } else {
                arrival_time
            };
            self.last_probing_sequence_reference_time = end_time;
            total_bytes = self
                .total_bytes_in_probing_sequence
                .wrapping_add(packet_size);
            total_packets = self.total_packets_in_probing_sequence.wrapping_add(1);
            self.total_bytes_in_probing_sequence = total_bytes;
            self.total_packets_in_probing_sequence = total_packets;

            if is_last_probing_packet && total_bytes > self.min_probing_sequence_size {
                duration = end_time - self.reference_packet_arrival_time;
                bwe = ((total_bytes << 3) as f64) / duration;
                if packet_id == 0 {
                    self.early_probing_packet_arrival_time = 0.0;
                    self.early_bytes_in_probing_sequence = 0;
                    self.early_packets_in_probing_sequence = 0;
                }
                // `mov w8, #1; cbz w23` -> when packet_id != 0, `w8 == 1` skips early update.
            } else {
                // 0x2063ba59c..0x2063ba5b4
                let prev_total_bytes = total_bytes;
                total_packets = 0;
                total_bytes = 0;
                bwe = 0.0;
                if packet_id == 0 {
                    self.early_probing_packet_arrival_time = 0.0;
                    self.early_bytes_in_probing_sequence = 0;
                    self.early_packets_in_probing_sequence = 0;
                } else if prev_total_bytes == 0 {
                    self.early_probing_packet_arrival_time = arrival_time;
                    self.early_bytes_in_probing_sequence = self
                        .early_bytes_in_probing_sequence
                        .wrapping_add(packet_size);
                    self.early_packets_in_probing_sequence =
                        self.early_packets_in_probing_sequence.wrapping_add(1);
                }
            }
        } else {
            // 0x2063ba4e8..0x2063ba598
            let cur_bytes = self.total_bytes_in_probing_sequence;
            if cur_bytes > self.min_probing_sequence_size {
                total_bytes = cur_bytes;
                total_packets = self.total_packets_in_probing_sequence;
                duration = self.last_probing_sequence_reference_time
                    - self.reference_packet_arrival_time;
                bwe = ((total_bytes << 3) as f64) / duration;
            } else {
                total_bytes = 0;
                total_packets = 0;
            }

            let mut next_bytes = 0u32;
            if !is_probing {
                // 0x2063ba57c..0x2063ba588: head is late iff (early_time > 0.0 && arrival_time < early_time)
                let early_t = self.early_probing_packet_arrival_time;
                if early_t > 0.0 && arrival_time < early_t {
                    // 0x2063ba5b8 -> 0x2063ba660
                    next_bytes = self.early_bytes_in_probing_sequence;
                    self.total_bytes_in_probing_sequence = next_bytes;
                    self.total_packets_in_probing_sequence = self.early_packets_in_probing_sequence;
                } else {
                    // 0x2063ba58c..0x2063ba594
                    self.total_bytes_in_probing_sequence = 0;
                    self.total_packets_in_probing_sequence = 0;
                }
            } else {
                // 0x2063ba58c..0x2063ba594
                self.total_bytes_in_probing_sequence = 0;
                self.total_packets_in_probing_sequence = 0;
            }

            // 0x2063ba66c..0x2063ba6a0
            self.last_probing_sequence_id = if is_probing { packet_id } else { sample_time };
            self.reference_packet_arrival_time = arrival_time;
            self.early_probing_packet_size = packet_size;
            self.last_probing_sequence_reference_time = 0.0;

            // 0x2063ba684: cbz w23, loc_2063ba5b0 -> clear early_* only when packet_id == 0
            // 0x2063ba688: cbz w22, loc_2063ba6a4 -> when !is_probing && packet_id != 0, leave early_* unchanged
            if packet_id == 0 {
                self.early_probing_packet_arrival_time = 0.0;
                self.early_bytes_in_probing_sequence = 0;
                self.early_packets_in_probing_sequence = 0;
            } else if is_probing && next_bytes == 0 {
                self.early_probing_packet_arrival_time = arrival_time;
                self.early_bytes_in_probing_sequence = self
                    .early_bytes_in_probing_sequence
                    .wrapping_add(packet_size);
                self.early_packets_in_probing_sequence =
                    self.early_packets_in_probing_sequence.wrapping_add(1);
            }
        }

        // 0x2063ba6a4..0x2063ba780
        if bwe > 0.0 {
            self.estimated_bandwidth_uncapped = bwe;
            let passed_bwe = if self.estimated_bandwidth == 0.0 {
                bwe
            } else if total_packets >= self.min_packets_count
                && total_bytes >= self.min_probing_sequence_size
            {
                if duration < self.min_estimation_window_duration {
                    self.over_range_count += 1;
                    if self.over_range_count >= self.max_over_range_count {
                        bwe
                    } else {
                        0.0
                    }
                } else {
                    self.over_range_count = 0;
                    bwe
                }
            } else {
                0.0
            };
            self.update_bandwidth_estimation(passed_bwe, arrival_time);
        }

        self.last_probing_packet_timestamp = sample_time;
    }

    /// `_VCRateControlBandwidthEstimator_CalculateBandwidthEstimationForBandwidthSample` (`0x2063ba784`).
    pub fn calculate_bandwidth_estimation_for_bandwidth_sample(
        &mut self,
        bandwidth_sample: f64,
        arrival_time: f64,
    ) {
        if bandwidth_sample <= 0.0 {
            return;
        }
        self.estimated_bandwidth_uncapped = bandwidth_sample;
        self.update_bandwidth_estimation(bandwidth_sample, arrival_time);
    }
}

/// `VCRateControlBandwidthEstimatorMap` (AVConference, iOS 27.0.1 24A446).
#[derive(Debug, Clone)]
pub struct VCRateControlBandwidthEstimatorMap {
    /// `_state` (`+0x08`, `u32`): `0` = single/unassigned, `1` = single active, `2` = multi-estimator.
    pub state: u32,
    /// `_mode` (`+0x0c`, `u32`)
    pub mode: u32,
    /// `_radioAccessTechnology` (`+0x10`, `u32`)
    pub radio_access_technology: u32,
    /// `_estimatedBandwidth` (`+0x18`, `f64`)
    pub estimated_bandwidth: f64,
    /// `_estimatedBandwidthUncapped` (`+0x20`, `f64`)
    pub estimated_bandwidth_uncapped: f64,
    /// `_bandwidthEstimators` (`+0x28`, `HashMap<u32, VCRateControlBandwidthEstimator>`)
    pub bandwidth_estimators: HashMap<u32, VCRateControlBandwidthEstimator>,
    /// `_currentActiveEstimatorID` (`+0x30`, `i32`), `-1` initially.
    pub current_active_estimator_id: i32,
    /// `_defaultEstimator` (`+0x38`) key in `bandwidth_estimators`.
    pub default_estimator_id: Option<u32>,
    /// `_fastSuddenBandwidthDetectionEnabled` (`+0x44`, `bool`)
    pub fast_sudden_bandwidth_detection_enabled: bool,
    /// `_l4sHighDataRateEnabled` (`+0x45`, `bool`)
    pub l4s_high_data_rate_enabled: bool,
    /// `_serverBagParams` (`+0x50`)
    pub server_bag_params: Option<ServerBagBwEstimationParams>,
}

impl Default for VCRateControlBandwidthEstimatorMap {
    fn default() -> Self {
        Self::new()
    }
}

impl VCRateControlBandwidthEstimatorMap {
    /// `-[VCRateControlBandwidthEstimatorMap init]` (`0x2063baa50`).
    pub fn new() -> Self {
        Self {
            state: 0,
            mode: 0,
            radio_access_technology: 0,
            estimated_bandwidth: 0.0,
            estimated_bandwidth_uncapped: 0.0,
            bandwidth_estimators: HashMap::new(),
            current_active_estimator_id: -1,
            default_estimator_id: None,
            fast_sudden_bandwidth_detection_enabled: false,
            l4s_high_data_rate_enabled: false,
            server_bag_params: None,
        }
    }

    /// `_VCRateControlBandwidthEstimatorMap_BandwidthEstimator` (`0x2063bab60`).
    pub fn bandwidth_estimator_mut(
        &mut self,
        estimator_id: u32,
        is_probing: bool,
        is_last_probing_packet: bool,
    ) -> Option<&mut VCRateControlBandwidthEstimator> {
        // 0x2063bab84..0x2063bab98:
        // if defaultEstimator != None && (mode != 6 && mode != 7) -> return defaultEstimator
        if self.default_estimator_id.is_some() && self.mode != 6 && self.mode != 7 {
            let def_id = self.default_estimator_id?;
            return self.bandwidth_estimators.get_mut(&def_id);
        }

        // 0x2063baba0..0x2063bac64: state machine
        match self.state {
            0 => {
                if is_probing {
                    self.state = 1;
                    self.current_active_estimator_id = estimator_id as i32;
                }
            }
            1 => {
                if estimator_id as i32 != self.current_active_estimator_id {
                    self.state = 2;
                    return None;
                }
                if is_last_probing_packet || !is_probing {
                    self.state = 0;
                }
            }
            2 => {
                let cond = ((is_last_probing_packet as u32) | ((!is_probing) as u32)) == 1;
                if !cond || (estimator_id as i32 != self.current_active_estimator_id) {
                    return None;
                }
                self.state = 0;
            }
            _ => {}
        }

        // 0x2063bac64..0x2063baddc: lookup or create
        if !self.bandwidth_estimators.contains_key(&estimator_id) {
            let mut est = VCRateControlBandwidthEstimator::new();
            est.set_mode(self.mode);
            est.set_radio_access_technology(self.radio_access_technology);
            est.set_fast_sudden_bandwidth_detection_enabled(
                self.fast_sudden_bandwidth_detection_enabled,
            );
            est.set_l4s_high_data_rate_enabled(self.l4s_high_data_rate_enabled);
            if let Some(ref params) = self.server_bag_params {
                est.set_server_bag_params(params);
            }
            self.bandwidth_estimators.insert(estimator_id, est);
            if self.default_estimator_id.is_none() {
                self.default_estimator_id = Some(estimator_id);
            }
        }
        self.bandwidth_estimators.get_mut(&estimator_id)
    }

    /// `_VCRateControlBandwidthEstimatorMap_DeregisterBandwidthEstimator` (`0x2063bae00`).
    pub fn deregister_bandwidth_estimator(&mut self, estimator_id: u32) {
        if estimator_id as i32 == self.current_active_estimator_id {
            self.state = 0;
        }
        if self.default_estimator_id == Some(estimator_id) {
            self.default_estimator_id = None;
        }
        self.bandwidth_estimators.remove(&estimator_id);
    }

    /// `_VCRateControlBandwidthEstimatorMap_EstimatedBandwidthWithArrivalTime` (`0x2063baf84`).
    pub fn estimated_bandwidth_with_arrival_time(&mut self, arrival_time: f64) -> f64 {
        // 0x2063bafb4..0x2063bafc8: if defaultEstimator != None && (mode < 6 || mode > 8) -> return default.estimatedBandwidth
        if let Some(def_id) = self.default_estimator_id {
            if self.mode < 6 || self.mode > 8 {
                return self
                    .bandwidth_estimators
                    .get(&def_id)
                    .map_or(0.0, |e| e.estimated_bandwidth);
            }
        }

        // 0x2063bafcc..0x2063bb02c & 0x2063bb26c: max across active estimators (< 30.0s stale)
        let mut max_bw = 0.0_f64;
        for est in self.bandwidth_estimators.values() {
            if arrival_time == 0.0
                || (arrival_time - est.last_probing_sequence_arrival_time) < 30.0
            {
                if est.estimated_bandwidth > max_bw {
                    max_bw = est.estimated_bandwidth;
                }
            }
        }
        self.estimated_bandwidth = max_bw;
        max_bw
    }

    /// `_VCRateControlBandwidthEstimatorMap_EstimatedBandwidthUncappedWithArrivalTime` (`0x2063bb0f8`).
    pub fn estimated_bandwidth_uncapped_with_arrival_time(&mut self, arrival_time: f64) -> f64 {
        if let Some(def_id) = self.default_estimator_id {
            if self.mode < 6 || self.mode > 8 {
                return self
                    .bandwidth_estimators
                    .get(&def_id)
                    .map_or(0.0, |e| e.estimated_bandwidth_uncapped);
            }
        }

        let mut max_bw = 0.0_f64;
        for est in self.bandwidth_estimators.values() {
            if arrival_time == 0.0
                || (arrival_time - est.last_probing_sequence_arrival_time) < 30.0
            {
                if est.estimated_bandwidth_uncapped > max_bw {
                    max_bw = est.estimated_bandwidth_uncapped;
                }
            }
        }
        self.estimated_bandwidth_uncapped = max_bw;
        max_bw
    }

    /// `_VCRateControlBandwidthEstimatorMap_bandwidthEstimationState` (`0x2063bb800`).
    pub fn bandwidth_estimation_state(&self) -> i32 {
        if let Some(def_id) = self.default_estimator_id {
            self.bandwidth_estimators
                .get(&def_id)
                .map_or(0, |e| e.bandwidth_estimation_state)
        } else {
            0
        }
    }

    /// `-[VCRateControlBandwidthEstimatorMap setMode:]` (`0x2063bb5a4`).
    pub fn set_mode(&mut self, mode: u32) {
        self.mode = mode;
        for est in self.bandwidth_estimators.values_mut() {
            est.set_mode(mode);
        }
    }

    /// `_VCRateControlBandwidthEstimatorMap_SetRadioAccessTechnology` (`0x2063bb6a0`).
    pub fn set_radio_access_technology(&mut self, rat: u32) {
        self.radio_access_technology = rat;
        for est in self.bandwidth_estimators.values_mut() {
            est.set_radio_access_technology(rat);
        }
    }

    /// `_VCRateControlBandwidthEstimatorMap_SetFastSuddenBandwidthDetectionEnabled` (`0x2063bb828`).
    pub fn set_fast_sudden_bandwidth_detection_enabled(&mut self, enabled: bool) {
        self.fast_sudden_bandwidth_detection_enabled = enabled;
        for est in self.bandwidth_estimators.values_mut() {
            est.set_fast_sudden_bandwidth_detection_enabled(enabled);
        }
    }

    /// `_VCRateControlBandwidthEstimatorMap_SetL4SHighDataRateEnabled` (`0x2063bb984`).
    pub fn set_l4s_high_data_rate_enabled(&mut self, enabled: bool) {
        self.l4s_high_data_rate_enabled = enabled;
        for est in self.bandwidth_estimators.values_mut() {
            est.set_l4s_high_data_rate_enabled(enabled);
        }
    }

    /// `-[VCRateControlBandwidthEstimatorMap setServerBag:]` (`0x2063bba2c`).
    pub fn set_server_bag_params(&mut self, params: ServerBagBwEstimationParams) {
        for est in self.bandwidth_estimators.values_mut() {
            est.set_server_bag_params(&params);
        }
        self.server_bag_params = Some(params);
    }

    /// `_VCRateControlBandwidthEstimatorMap_SetEstimatedBandwidth` (`0x2063bb34c`).
    pub fn set_estimated_bandwidth(&mut self, bandwidth: f64) {
        self.estimated_bandwidth = bandwidth;
        for est in self.bandwidth_estimators.values_mut() {
            est.set_estimated_bandwidth(bandwidth);
        }
    }
}

/// Output of `__AVCStatisticsCollector_ComputeBWEstimation` (`0x205ed4870..0x205ed48ec`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectorBandwidthOutput {
    /// `_sharedEstimatedBandwidth` (`collector + 0x54`, bps, quantized to 8 kbps + state bit).
    pub shared_estimated_bandwidth: u32,
    /// `_sharedEstimatedBandwidthUncapped` (`collector + 0x58`, bps).
    pub shared_estimated_bandwidth_uncapped: u32,
}

/// `__AVCStatisticsCollector_ComputeBWEstimation` (`0x205ed4870..0x205ed48ec`).
///
/// Returns `None` when `collector_mode == 5` (`0x205ed4874: cmp w8, #5; b.eq loc_205ed48ec`).
pub fn compute_bw_estimation_from_map(
    map: &mut VCRateControlBandwidthEstimatorMap,
    collector_mode: u32,
    estimated_bandwidth_cap: u32,
    arrival_time: f64,
) -> Option<CollectorBandwidthOutput> {
    if collector_mode == 5 {
        return None;
    }

    // 0x205ed487c..0x205ed4884: fcvtzu w8, d0
    let raw_f64 = map.estimated_bandwidth_with_arrival_time(arrival_time);
    let raw_u32 = if raw_f64.is_nan() || raw_f64 <= 0.0 {
        0
    } else if raw_f64 >= u32::MAX as f64 {
        u32::MAX
    } else {
        raw_f64 as u32
    };

    // 0x205ed4888..0x205ed4898: apply _estimatedBandwidthCap (`+0x70`) if non-zero
    let capped = if estimated_bandwidth_cap == 0 {
        raw_u32
    } else {
        estimated_bandwidth_cap.min(raw_u32)
    };

    // 0x205ed489c..0x205ed48cc: quantize to 8 kbps and encode state == 1 in LSB of kbps
    let state = map.bandwidth_estimation_state();
    let quantized = if capped == 0 {
        0
    } else {
        let kbps = (((capped as u64) * 0x1062_4dd3u64) >> 38) as u32;
        let mut q = kbps & 0x7f_fff8;
        if state == 1 {
            q += 1;
        }
        q.wrapping_mul(1000)
    };

    // 0x205ed48cc..0x205ed48d4: uncapped
    let uncapped_f64 = map.estimated_bandwidth_uncapped_with_arrival_time(arrival_time);
    let uncapped_u32 = if uncapped_f64.is_nan() || uncapped_f64 <= 0.0 {
        0
    } else if uncapped_f64 >= u32::MAX as f64 {
        u32::MAX
    } else {
        uncapped_f64 as u32
    };

    Some(CollectorBandwidthOutput {
        shared_estimated_bandwidth: quantized,
        shared_estimated_bandwidth_uncapped: uncapped_u32,
    })
}

/// `__AVCStatisticsCollector_GetVCStatisticsWithType` (`0x205ed38fc..0x205ed3934`)
/// for `type == 2` (`mode in 1..=4`).
///
/// Converts `_sharedEstimatedBandwidth` (bps) to kbps (`(val * 0x10624dd3) >> 38`)
/// and zeroes it if `(collector_mode & 0xfffffffd) == 1` (modes 1 or 3) and
/// `current_time - last_bw_estimation_time >= 30.0`.
pub fn bandwidth_estimation_kbps_for_stats(
    shared_estimated_bandwidth: u32,
    collector_mode: u32,
    current_time: f64,
    last_bw_estimation_time: f64,
) -> u32 {
    let mut kbps = (((shared_estimated_bandwidth as u64) * 0x1062_4dd3u64) >> 38) as u32;
    if (collector_mode & 0xffff_fffd) == 1 && (current_time - last_bw_estimation_time) >= 30.0 {
        kbps = 0;
    }
    kbps
}
