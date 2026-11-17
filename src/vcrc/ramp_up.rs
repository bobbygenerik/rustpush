//! Ramp-up decisions: `_VCRateControlAlgorithmStabilizedNOWRDPriv_ShouldRampUp` 0x2061ff134,
//! `..._ShouldFastRampUp` 0x2065c19c8, and the helpers they call. `_rampUpStatus` bits are
//! Apple's: 0x2 not stable, 0x4 settling, 0x8 send-limited, 0x10 OWRD, 0x20 NOWRD,
//! 0x40 NOWRD accumulation, 0x80 OWRD constant, 0x100 OWRD list not ready, 0x200 recently
//! above BWE, 0x400 baseband, 0x1000 at max tier, 0x2000 network saturation,
//! 0x4000 loss event / unstable network, 0x8000 ECN CE, 0x10000 bytes in flight.

use super::media::MediaController;
use super::ramp_down::congestion_level_bitrate_range;
use super::state::{HistoryRecord, VcrcState};

impl VcrcState {
    /// `_VCRateControlAlgorithm_IsVideoStopped` 0x20617a9d4: only for config modes 1, 2, 7
    /// (mask 0x86), then `VCRateControlMediaController_IsVideoStopped` 0x20604c8e8.
    pub fn is_video_stopped(&self, mc: Option<&dyn MediaController>) -> bool {
        let mode = self.config.o_0;
        if mode > 7 || (1u32 << mode) & 0x86 == 0 { return false; }
        mc.map_or(false, |m| m.is_video_stopped())
    }

    /// `_VCRateControlAlgorithmBasePriv_RecentlyGoAboveRampUpBandwidth` 0x20617a948.
    pub fn recently_go_above_ramp_up_bandwidth(&self, mc: Option<&dyn MediaController>) -> bool {
        if self.state == 1 && !self.is_first_initial_ramp_up_done { return false; }
        if self.remote_bandwidth_estimation == 0 || self.config.o_182 != 1 { return false; }
        if self.is_video_stopped(mc) { return false; }
        if self.below_no_ramp_up_bandwidth {
            self.rate_control_time - self.first_below_no_ramp_up_bandwidth_time < self.config.o_c0
        } else {
            true
        }
    }

    /// `_VCRateControlAlgorithmBasePriv_ShouldBlockRampUpDueToLossEventThreshold` 0x20617ae18.
    pub fn should_block_ramp_up_due_to_loss_event_threshold(&mut self) -> bool {
        if self.config.o_e8 == 0.0 || !(self.first_loss_event_ramp_down_time > 0.0) { return false; }
        let avg = self.packet_loss_rate_avg_long;
        let block = avg > self.config.o_2f0;
        if self.packet_loss_rate_bad_trend_count < self.config.o_e0 as i32 { return block; }
        if avg > self.config.o_300 {
            // 0x20617ae70..e98: leave loss-based adaptation.
            self.packet_loss_rate_avg_long_at_lba_exit = avg;
            let x = avg * 3.0;
            self.packet_loss_rate_threshold = if x < self.config.o_2f8 { x } else { self.config.o_2f8 };
            self.first_loss_event_ramp_down_time = 0.0;
            self.loss_event_bandwidth_limit = 0;
            self.loss_event_bandwidth_confidence = 0;
            block
        } else if self.packet_loss_rate <= self.packet_loss_rate_threshold {
            block
        } else {
            true
        }
    }

    /// `__VCRateControlAlgorithmStabilizedNOWRD_CheckNetworkSaturation` 0x206200bf4.
    pub fn check_network_saturation(&self, rtt: f64, env: f64, plr: f64, owrd: f64) -> bool {
        rtt > env * self.config.o_1c8 && plr > 0.0 && owrd > env * self.config.o_1d0
    }

    /// `_VCRateControlAlgorithmStabilizedNOWRDPriv_UpdateVCRateControlHistory` 0x2061fe564.
    /// `arg` is the second argument, stored at record +0x24.
    pub fn update_vc_rate_control_history(&mut self, arg: u32) {
        let idx = if self.current_history_size == 0 {
            self.current_history_size = 1;
            0 // 0x2061fe5cc: first record goes to slot 0; the index is not advanced.
        } else {
            if self.current_history_size <= 0x3ff { self.current_history_size += 1; }
            self.current_history_index = (self.current_history_index.wrapping_add(1)) & 0x3ff;
            self.current_history_index as usize
        };
        self.history[idx] = HistoryRecord {
            owrd: self.owrd, round_trip_time: self.round_trip_time,
            round_trip_time_min_envelope: self.round_trip_time_min_envelope,
            packet_loss_rate: self.packet_loss_rate, tier_index: self.current_tier_index, arg,
        };
    }

    /// `_VCRateControlAlgorithmStabilizedNOWRDPriv_ShouldRampUp` 0x2061ff134.
    pub fn should_ramp_up(&mut self, mc: Option<&dyn MediaController>) -> bool {
        let mut up = self.is_stable;
        if !self.is_stable { self.ramp_up_status |= 0x2; }
        if self.is_ramp_up_settling { up = false; self.ramp_up_status |= 0x4; }
        if self.is_send_bitrate_limited { up = false; self.ramp_up_status |= 0x8; }
        if self.recently_go_above_ramp_up_bandwidth(mc) { up = false; self.ramp_up_status |= 0x200; }
        if !self.is_owrd_list_ready { up = false; self.ramp_up_status |= 0x100; }
        if self.is_owrd_constant { up = false; self.ramp_up_status |= 0x80; }
        if self.owrd > self.config.o_a8 { up = false; self.ramp_up_status |= 0x10; }
        if self.nowrd > self.config.o_b0 { up = false; self.ramp_up_status |= 0x20; }
        // 0x2061ff318..3a8: accumulation threshold (table half at config+0x3a0 for ramp-up).
        let range = congestion_level_bitrate_range(self.target_bitrate) as usize;
        let thr = if self.should_consider_congestion_level_for_nowrd_acc {
            self.config.o_378[5 + range] * self.round_trip_time_min_envelope
        } else {
            self.config.o_68
        };
        if self.nowrd_acc > thr { up = false; self.ramp_up_status |= 0x40; }
        if self.baseband_adaptation_enabled && self.state != 1 && self.baseband_additional_tiers_for_ramp_up == 0 {
            up = false; self.ramp_up_status |= 0x400;
        }
        let tier = self.current_tier_index;
        if tier >= self.config.o_14 { up = false; self.ramp_up_status |= 0x1000; }
        // 0x2061ff45c..514: above tier config+0x1a8.
        if tier >= self.config.o_1a8 as i32 {
            let bt = self.config.o_189 == 1 && self.with_bt_coex;
            let block = if self.state == 1 {
                self.remote_bandwidth_estimation == 0
            } else {
                bt || self.rate_control_time - self.last_network_unstable_time < self.config.o_298
            };
            if block { up = false; self.ramp_up_status |= 0x4000; }
        }
        // 0x2061ff518..608: network saturation now and over the last config+0x1d8 history records.
        if self.config.o_188 == 1 {
            let mut sat = self.check_network_saturation(self.round_trip_time,
                self.round_trip_time_min_envelope, self.packet_loss_rate, self.owrd);
            let n = self.config.o_1d8 as i32;
            let mut k = 0i32;
            while k < n {
                let j = (self.current_history_index.wrapping_sub(k as u32) & 0x3ff) as usize;
                let h = self.history[j];
                sat &= self.check_network_saturation(h.round_trip_time,
                    h.round_trip_time_min_envelope, h.packet_loss_rate, h.owrd);
                k += 1;
            }
            if sat { up = false; self.ramp_up_status |= 0x2000; }
        }
        if self.should_block_ramp_up_due_to_loss_event_threshold() { up = false; self.ramp_up_status |= 0x4000; }
        if self.ecn_ce_ratio > 0.0 { up = false; self.ramp_up_status |= 0x8000; }
        // 0x2061ff664..6e4: bytes-in-flight gate.
        let bif_block = self.bytes_in_flight_adaptation_enabled && self.bytes_in_flight != 0
            && !self.allow_ramp_up_based_on_bytes_in_flight;
        if bif_block {
            self.ramp_up_status |= 0x10000;
        } else if up {
            self.last_allow_ramp_up_time = self.rate_control_time;
            return true;
        }
        // 0x2061ff6e8..79c: allow anyway after config+0xc8 seconds without a ramp-up when the
        // only blocker is send-limited (0x8), baseband (0x400), or 0x800.
        let wait = self.config.o_c8;
        let last = self.last_allow_ramp_up_time;
        if wait != 0.0 && last != 0.0 && !(self.rate_control_time - last < wait) {
            let s = self.ramp_up_status;
            if s == 0x8 || s == 0x800 || s == 0x400 {
                self.ramp_up_status = 0;
                self.last_allow_ramp_up_time = self.rate_control_time;
                return true;
            }
        }
        false
    }

    /// `__VCRateControlAlgorithmStabilizedNOWRD_IsBitrateOscillating` 0x2062009b0. Walks the
    /// history from the newest record; true once more than config+0x1e8 tier changes over fewer
    /// than config+0x1ec distinct tiers have been seen and `tier` is the lowest tier visited.
    /// Gated by config+0x186, whose value is still UNKNOWN (vcrc_config EFFECTIVE_VALUE_UNKNOWNS).
    pub fn is_bitrate_oscillating(&self, tier: i32) -> bool {
        if self.config.o_186 != 1 { return false; }
        let mut counts = [0u32; 66];
        counts[tier as usize] += 1;
        let size = self.current_history_size as i32;
        if size < 2 { return false; }
        let mut idx = self.current_history_index;
        let (mut distinct, mut changes) = (1i32, 1i32);
        let (mut prev, mut min) = (tier, tier);
        for _ in 0..size - 1 {
            let t = self.history[(idx & 0x3ff) as usize].tier_index;
            if prev != t {
                if counts[t as usize] == 0 { distinct += 1; }
                counts[t as usize] += 1;
                changes += 1;
            }
            prev = t;
            if t < min { min = t; }
            if changes > self.config.o_1e8 as i32 && distinct < self.config.o_1ec as i32 && min == tier {
                return true;
            }
            idx = idx.wrapping_sub(1);
        }
        false
    }

    /// `_VCRateControlAlgorithmStabilizedNOWRDPriv_ShouldFastRampUp` 0x2065c19c8.
    pub fn should_fast_ramp_up(&self) -> bool {
        let c = &self.config;
        let tier = self.current_tier_index;
        if self.abnormal_network_detected && tier >= c.o_20 { return false; }
        if c.o_187 != 1 || self.first_loss_event_ramp_down_time != 0.0 { return false; }
        if !self.is_remote_bandwidth_estimation_stable { return false; }
        let gap = c.o_1b8 as i32 + tier;
        let idx = if c.o_14 < gap { c.o_14 } else { gap };
        if self.tier(idx) >= self.remote_bandwidth_estimation.wrapping_mul(1000) { return false; }
        let t = self.rate_control_time;
        t - self.last_network_unstable_time > c.o_1c0
            && tier <= c.o_1a8 as i32
            && t - self.last_ramp_down_time_due_to_feedback > c.o_118
            && t - self.last_ramp_down_time_due_to_baseband > c.o_118
            && t - self.extended_ramp_up_frozen_exit_time > c.o_1c0
    }
}
