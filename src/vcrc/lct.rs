//! `VCRateControlAlgorithmLayeredContinuousTier`: object creation (init chain + Configure),
//! the StabilizedNOWRD StateEnter/StateExit it inherits, RampUp/RampDown with their speed
//! factors, the state machine (UpdateTargetBitrate) and DoRateControl for VCRC statistics
//! (type 2). Addresses are 24A446 AVConference.

use super::media::MediaController;
use super::ramp_down::congestion_level_bitrate_range;
use super::state::VcrcState;
use super::stats::VcrcStatistics;
use crate::vcrc_config::{VcrcConfig, TIER_BITRATES};

impl VcrcState {
    /// What AVCRateController_ConfigureInternal does for mode 1 with the default selection:
    /// alloc/init the class (0x205ead53c..548) and call VCRateControlAlgorithm_Configure with
    /// `initial = 1` (0x205ead9f0..9f8).
    pub fn new_layered_continuous_tier(config: VcrcConfig, mc: Option<&mut (dyn MediaController + '_)>,
        now: f64) -> Self {
        let mut s = VcrcState::new(config.clone());
        // -[VCRateControlAlgorithmBase init] 0x206179f98.
        s.paused = false;
        s.packet_loss_rate_threshold = 0.15; // q0 from 0x2066a7d30 = (0.15, -1.0) → +0x1410
        s.packet_loss_rate_avg_long = -1.0; //                                       → +0x1418
        s.rate_sharing_count = 1;
        s.rate_sharing_factor = 1.0;
        s.consecutive_zero_plr_count = 0;
        // -[VCRateControlAlgorithmStabilizedNOWRD init] 0x2061fb2a0.
        s.state = 0; s.ramp_up_status = 0; s.ramp_down_status = 0;
        s.should_consider_congestion_level_for_bif = true;
        // -[VCRateControlAlgorithmLayeredContinuousTier init] 0x205f95a30.
        s.state = 0; s.ramp_up_status = 0; s.ramp_down_status = 0;
        s.should_ramp_up_skip_nowrd_stabilization = true;
        s.congestion_severity = 1.0;
        s.configure(config, true, mc, now);
        s
    }

    /// `VCRateControlAlgorithmLayeredContinuousTierPriv_Configure` 0x205f95c34, which first runs
    /// `VCRateControlAlgorithmStabilizedNOWRDPriv_Configure` 0x2061fb4b4. SmartBrake is not set
    /// up because config+0x370 is 0 on this profile.
    pub fn configure(&mut self, config: VcrcConfig, initial: bool,
        mc: Option<&mut (dyn MediaController + '_)>, now: f64) {
        self.config = config; // memmove 0x430
        self.set_bitrate_unstable(now);
        self.is_send_bitrate_limited = false;
        self.with_bt_coex = false;
        self.bytes_in_flight_adaptation_enabled = self.config.o_180 != 0;
        self.first_loss_event_ramp_down_time = 0.0;
        self.packet_loss_rate_threshold = self.config.o_2e8;
        self.reset_loss_event_buffer();
        self.bytes_in_flight_state = -1;
        if initial {
            self.state = 1;
            self.current_tier_index = self.config.o_10;
        }
        self.baseband_adaptation_enabled = self.config.o_183 & 1 != 0;
        let mut t = self.current_tier_index;
        if self.config.o_14 <= t { t = self.config.o_14; }
        if t <= self.config.o_18 { t = self.config.o_18; }
        self.current_tier_index = t;
        self.target_bitrate = self.tier(t);
        if let Some(m) = mc { m.set_min_target_bitrate(self.tier(self.config.o_18)); }
        self.owrd_list = Default::default(); // ResetOWRDList 0x20617ad5c (memset 0x670)
        self.bytes_in_flight = 0;
        self.bytes_in_flight_threshold = 0;
        self.owrd_list.span_too_short = true; // +0x668 = 1
        // LayeredContinuousTier part.
        self.bandwidth_wall = 0.0;
        self.bandwidth_wall_confidence = 0;
        self.target_bitrate_continuous = self.target_bitrate as f64;
        self.target_bitrate_tier = TIER_BITRATES[self.current_tier_index as usize];
    }

    /// `_VCRateControlAlgorithmBasePriv_StateChange` 0x20617a470.
    pub fn state_change(&mut self, new_state: i32) {
        if new_state == self.state { return; }
        self.vtable_state_exit();
        self.state = new_state;
        self.vtable_state_enter();
    }

    /// vtable StateExit = `__VCRateControlAlgorithmStabilizedNOWRD_StateExit` 0x2061fc094
    /// (LayeredContinuousTier's setUpVTable 0x205f95b90 leaves slots 2/3 to StabilizedNOWRD).
    pub fn vtable_state_exit(&mut self) {
        if self.state == 4 {
            self.extended_ramp_up_frozen_exit_time = self.rate_control_time;
        } else if self.state == 1 {
            self.is_first_initial_ramp_up_done = true; // "Initial Rampup done!"
        }
    }

    /// vtable StateEnter = `__VCRateControlAlgorithmStabilizedNOWRD_StateEnter` 0x2061fbf6c.
    pub fn vtable_state_enter(&mut self) {
        match self.state {
            4 => {
                self.ramp_up_frozen_time = self.rate_control_time;
                let k = if self.rate_control_time - self.extended_ramp_up_frozen_exit_time
                    <= self.config.o_1c0 { self.config.o_1f8 } else { self.config.o_1f0 };
                self.ramp_up_frozen_duration = k * self.get_ramp_up_frozen_duration();
            }
            3 => {
                self.ramp_up_frozen_time = self.rate_control_time;
                self.ramp_up_frozen_duration = self.get_ramp_up_frozen_duration();
            }
            1 => self.is_ramp_up_settling = false,
            _ => {}
        }
    }

    fn arc4random_uniform(&mut self, n: u32) -> u32 {
        let mut x = self.rng;
        x ^= x << 13; x ^= x >> 7; x ^= x << 17;
        self.rng = x;
        (x % n as u64) as u32
    }

    /// `__VCRateControlAlgorithmStabilizedNOWRD_GetRampUpFrozenDuration` 0x206200b18.
    pub fn get_ramp_up_frozen_duration(&mut self) -> f64 {
        let c = &self.config;
        let base = if c.o_187 == 1 { self.ramp_up_settle_duration } else { c.o_98 };
        let d = if self.current_tier_index >= c.o_28 {
            c.o_1c0
        } else {
            if self.current_tier_index >= c.o_20 && self.abnormal_network_detected { return 30.0; }
            base
        };
        if self.config.o_185 == 1 {
            // arc4random_uniform(0xffff) at 0x206200bc0; [0x2066a7f20] = 65535.0.
            let r = self.arc4random_uniform(0xffff) as f64;
            d + (d * r / 65535.0 + d * -0.5)
        } else {
            d
        }
    }

    /// `_VCRateControlAlgorithmBasePriv_UpdateCongestionStatusWhenRampUp` 0x20617b2b8.
    pub fn update_congestion_status_when_ramp_up(&mut self) {
        self.consecutive_ramp_down = 0;
        self.last_time_start_ramping_down = 0.0;
        let tb = self.target_bitrate;
        let lim = self.loss_event_bandwidth_limit;
        let diff = (tb.wrapping_sub(lim) as i32).unsigned_abs() as f64;
        let conf = self.loss_event_bandwidth_confidence;
        // [0x2066a7d40] = 0.35, [0x2066a7d48] = 0.16, [0x2066a3938] = 0.01.
        if lim as f64 * 0.35 > diff && conf != 0 {
            let mut x = diff * 0.16 / self.target_bitrate_continuous / conf as f64;
            if x < 0.01 { x = 0.01; }
            self.target_bitrate_continuous *= 1.0 + x;
        } else {
            self.target_bitrate_continuous = tb as f64;
        }
    }

    /// `_VCRateControlAlgorithmBasePriv_UpdateCongestionStatusWhenRampDown` 0x20617b33c.
    pub fn update_congestion_status_when_ramp_down(&mut self, t: f64) {
        self.consecutive_ramp_down = self.consecutive_ramp_down.wrapping_add(1);
        if self.last_time_start_ramping_down == 0.0 {
            self.last_time_start_ramping_down = t;
        } else if !(t - self.last_time_start_ramping_down < self.config.o_98) {
            self.consecutive_ramp_down = 0;
            self.last_time_start_ramping_down = 0.0;
        } else if self.consecutive_ramp_down >= self.config.o_150 {
            self.is_congested = true;
            self.last_congestion_time = t;
        }
        self.target_bitrate_continuous = self.target_bitrate as f64;
    }

    /// `_VCRateControlAlgorithmStabilizedNOWRDPriv_ShouldDoAdditionalRampDownDueToLossEvent`
    /// 0x2061fed98.
    pub fn should_do_additional_ramp_down_due_to_loss_event(&self) -> bool {
        if self.ramp_down_status & 0x200 == 0 { return false; }
        if self.first_loss_event_ramp_down_time > 0.0 { return true; }
        self.target_bitrate > self.ramp_down_suppression_bitrate_threshold
    }

    /// `_VCRateControlAlgorithmStablizedNORDPriv_GetBitrateOscillationCount` 0x2065c186c.
    /// Counts high→low→high swings of the history's target-bitrate field (+0x24) around
    /// config+0x200 × tier[o_14] and 36000 bps. Gated by config+0x186 (UNKNOWN; 0 → off).
    pub fn get_bitrate_oscillation_count(&self) -> i32 {
        if self.config.o_186 != 1 { return 0; }
        let thr = (self.config.o_200 * self.tier(self.config.o_14) as f64) as u32;
        let n = -(self.current_history_size as i32).max(0);
        let (mut i, mut count, mut st) = (0i32, 0i32, 0u32);
        while i != n {
            let v = self.history[((self.current_history_index as i32 + i) & 0x3ff) as usize].arg;
            let a = if v > thr { 2 } else { 1 };
            let low = (v >> 5) < 0x465;
            let b = if st != 0 { if low { 3 } else { 2 } } else { low as u32 };
            st = if st == 1 { a } else { b };
            if st == 3 { st = 0; count += 1; }
            i -= 1;
            if i == -0x400 { break; }
        }
        count
    }

    /// `__VCRateControlAlgorithmLayeredContinuousTier_RampUpSpeedFactor` 0x205f975a4.
    pub fn ramp_up_speed_factor(&mut self) -> f64 {
        let c = &self.config;
        let bwe = self.remote_bandwidth_estimation;
        let floor_kbps = self.tier(c.o_1c) / 1000;
        // 0x205f975c4..6bc: time constant.
        let tc = if self.abnormal_network_detected && self.current_tier_index > c.o_20 {
            30.0
        } else if c.o_18e == 1 && bwe != 0 && bwe <= floor_kbps {
            3.0
        } else if self.state == 1 {
            self.round_trip_time_min_envelope
        } else {
            let r = congestion_level_bitrate_range(self.target_bitrate_continuous as u32) as usize;
            [c.o_158, c.o_160, c.o_168, c.o_170, c.o_178][r]
        };
        // 0x205f976c0..744: oscillation brake, checked except on every 32nd call.
        let cnt = self.ramp_up_counter;
        self.ramp_up_counter = cnt.wrapping_add(1);
        let mut adj = 1.0;
        if cnt & 0x1f != 0 && self.get_bitrate_oscillation_count() >= 1 && self.speed_factor > 0.5
            && self.target_bitrate_continuous >= 100000.0 { // [0x2066a3f88]; b.lt keeps 1.0
            adj = 0.0;
        }
        self.ramp_up_speed_adjustment_factor = adj;
        let base = (1.0 / tc) * adj;
        // 0x205f97764..7dc: reference = bandwidth wall when trusted, else remote BWE.
        let tcont = self.target_bitrate_continuous;
        let bwe_bps = bwe.wrapping_mul(1000);
        let wall = self.bandwidth_wall;
        // [0x2066a5be0] = 76000.
        let (reference, conf) = if wall == 0.0 || tcont < 76000.0 {
            (bwe_bps as f64, 3)
        } else if bwe_bps == 0 || wall <= bwe_bps as f64 {
            (wall, self.bandwidth_wall_confidence)
        } else {
            (bwe_bps as f64, 3)
        };
        let dist = (reference - tcont).abs();
        let band = reference * 0.15; // [0x2066a3d38]
        let near = dist < band && conf > 2;
        let c = &self.config;
        let speed = if c.o_18e == 1 && bwe != 0 {
            if !near { base } else {
                let r = dist / band / (conf - 2) as f64;
                if bwe > floor_kbps {
                    if self.abnormal_network_detected { if base < r { base } else { r } } else { r }
                } else if base < r { base } else { r }
            }
        } else if !near {
            base
        } else {
            let r = dist / band / (conf - 2) as f64;
            if self.abnormal_network_detected { if base < r { base } else { r } } else { r }
        };
        // fmaxnm with [0x2066a5be8] = 1/60, then fminnm with 2.0.
        speed.max(1.0 / 60.0).min(2.0)
    }

    /// `__VCRateControlAlgorithmLayeredContinuousTier_RampUp` 0x205f974a4. Returns bps.
    pub fn lct_ramp_up(&mut self, now: f64) -> u32 {
        self.set_bitrate_unstable(now);
        self.update_congestion_status_when_ramp_up();
        self.packet_loss_rate_bad_trend_count = 0;
        let t = self.target_bitrate_continuous;
        let dt = self.rate_control_time - self.previous_rate_control_time;
        let sf = self.ramp_up_speed_factor();
        self.speed_factor = sf;
        // [0x2066a3f58] = 228000, [0x2066a5bd8] = 1.159, [0x2066a5920] = 32000.
        let x = if t > 228000.0 { t * 1.159f64.powf(dt * sf) } else { (dt * sf) * 32000.0 + t };
        let cap = TIER_BITRATES[self.config.o_14 as usize] as f64;
        (if x < cap { x } else { cap }) as u32
    }

    /// `__VCRateControlAlgorithmLayeredContinuousTier_RampDownSpeedFactor` 0x205f973a8.
    pub fn ramp_down_speed_factor(&self, low_range: bool) -> f64 {
        let tc = self.target_bitrate_continuous;
        let f = if low_range && tc < TIER_BITRATES[self.config.o_30 as usize] as f64 {
            1.0
        } else if self.should_do_additional_ramp_down_due_to_loss_event() {
            2.0
        } else if self.ramp_down_status & 0x200 != 0 || self.config.o_18b & 1 != 0 {
            1.0
        } else if self.ramp_down_status & 0x800 != 0
            && tc < TIER_BITRATES[self.config.o_20 as usize] as f64 {
            1.0
        } else {
            3.0
        };
        f * self.congestion_severity
    }

    /// `__VCRateControlAlgorithmLayeredContinuousTier_RampDown` 0x205f970fc. Returns bps.
    pub fn lct_ramp_down(&mut self, now: f64) -> u32 {
        self.set_bitrate_unstable(now);
        self.update_congestion_status_when_ramp_down(self.rate_control_time);
        let t = self.target_bitrate_continuous;
        let mut x = if t > 228000.0 {
            let sf = self.ramp_down_speed_factor(false);
            self.speed_factor = sf;
            t / 1.159f64.powf(sf)
        } else {
            let sf = self.ramp_down_speed_factor(true);
            self.speed_factor = sf;
            // [0x2066a5bc8] 50000, [0x2066a5bd0] 132000; steps 8000 / 16000 / 32000.
            let step = if t < 50000.0 { 8000.0 } else if t < 132000.0 { 16000.0 } else { 32000.0 };
            t - step * sf
        };
        let c = &self.config;
        let floor = TIER_BITRATES[c.o_18 as usize] as f64;
        if !(x > floor) { x = floor; }
        self.congestion_severity = 1.0;
        let mut d = (x as u32) as f64;
        // 0x205f97290..384: optional tier steps (config+0x18e is UNKNOWN; 0 → skipped).
        if c.o_18e == 1 {
            let one_tier_down = |s: &VcrcState| {
                let mut i = s.current_tier_index - 1;
                if !(i > s.config.o_18) { i = s.config.o_18; }
                s.tier(i) as f64
            };
            if self.ramp_down_status & 0x10 != 0 && self.current_tier_index < c.o_20 {
                d = one_tier_down(self);
            } else if c.o_18a == 1 {
                let floor2 = self.tier(c.o_30) as f64;
                if d < floor2 {
                    if self.target_bitrate_continuous > floor2 {
                        d = floor2;
                    } else if self.rate_control_time - self.last_ramp_down_time_due_to_feedback < c.o_240 {
                        d = self.target_bitrate_continuous;
                    } else {
                        d = one_tier_down(self);
                    }
                }
            }
        }
        d as u32
    }

    /// `_VCRateControlAlgorithmLayeredContinuousTierPriv_SetTargetBitrate` 0x205f97024.
    pub fn set_target_bitrate(&mut self, bps: u32) {
        let mut i = self.config.o_14;
        let tier = loop {
            if i < 0 { break i; } // tbnz w10, #31 keeps the negative max tier
            if TIER_BITRATES[i as usize] <= bps { break i; }
            if i <= 0 { break -1; }
            i -= 1;
        };
        let old = self.current_tier_index;
        self.current_tier_index = if tier > self.config.o_18 { tier } else { self.config.o_18 };
        if self.current_tier_index < old {
            self.last_ramp_down_time_due_to_feedback = self.rate_control_time;
        }
        self.target_bitrate = bps;
        self.target_bitrate_tier = TIER_BITRATES[self.current_tier_index as usize];
    }

    /// State-2 bandwidth-wall update before a ramp-down
    /// (`..._UpdateTargetBitrate` 0x205f969dc..96b60).
    fn update_bandwidth_wall(&mut self) {
        if self.ramp_down_status == 0x800 { return; } // "CERatio ramp down. Ignore updating..."
        let t = self.target_bitrate_continuous as u32; // fcvtzu 0x205f96a00
        let wall = self.bandwidth_wall;
        if wall == 0.0 {
            self.bandwidth_wall = t as f64;
            self.bandwidth_wall_confidence += 1;
            return;
        }
        let diff = ((t as i32) as f64 - wall) as i32; // scvtf / fsub / fcvtzs 0x205f96a30..a38
        let abs = diff.unsigned_abs();
        if wall * 0.15 <= abs as f64 {
            let c = &self.config;
            if c.o_18e == 1 && diff < 0
                && self.remote_bandwidth_estimation.wrapping_sub(1) < self.tier(c.o_1c) / 1000 {
                self.bandwidth_wall_confidence = (self.bandwidth_wall_confidence - 1).max(0);
            } else {
                self.bandwidth_wall_confidence = 0;
            }
        } else {
            self.bandwidth_wall_confidence = (self.bandwidth_wall_confidence + 1).min(10);
        }
        self.bandwidth_wall = t as f64 / 2.0 + wall * 0.5;
    }

    /// `_VCRateControlAlgorithmLayeredContinuousTierPriv_UpdateTargetBitrate` 0x205f96554,
    /// statistics type 2 (VCRC feedback). States: 1 initial ramp, 2 steady, 6 after ramp-down,
    /// 0 paused; any other state (3 after a pause, for example) only logs.
    pub fn update_target_bitrate_vcrc(&mut self, now: f64) {
        let up = self.should_ramp_up_flag;
        let down = self.should_ramp_down_flag;
        let c = &self.config;
        let cap_tier = if c.o_28 <= c.o_14 { c.o_28 } else { c.o_14 };
        let cap = self.tier(cap_tier) as f64;
        match self.state {
            1 => {
                if down {
                    self.target_bitrate_continuous = self.lct_ramp_down(now) as f64;
                    self.state_change(6);
                } else if up {
                    self.target_bitrate_continuous = self.lct_ramp_up(now) as f64;
                    if cap <= self.target_bitrate_continuous { self.state_change(2); }
                }
            }
            2 => {
                if down {
                    self.update_bandwidth_wall();
                    self.target_bitrate_continuous = self.lct_ramp_down(now) as f64;
                    self.state_change(6);
                } else if up {
                    self.target_bitrate_continuous = self.lct_ramp_up(now) as f64;
                    if self.should_fast_ramp_up() && self.target_bitrate_continuous < cap {
                        self.state_change(1);
                    }
                }
            }
            6 => {
                if down {
                    self.target_bitrate_continuous = self.lct_ramp_down(now) as f64;
                } else if up {
                    self.target_bitrate_continuous = self.lct_ramp_up(now) as f64;
                    self.state_change(2);
                }
            }
            _ => {}
        }
        self.set_target_bitrate(self.target_bitrate_continuous as u32); // fcvtzu 0x205f96b94
    }

    /// `_VCRateControlAlgorithmLayeredContinuousTierPriv_VCRCStatisticsPostStateTransitionUpdates`
    /// 0x205f96ec0.
    pub fn post_state_transition_updates(&mut self, mc: Option<&mut (dyn MediaController + '_)>) {
        if !(self.ramp_up_status == 2 && self.ramp_down_status == 0) {
            self.previous_rate_control_time = self.rate_control_time;
        }
        self.update_vc_rate_control_history(self.target_bitrate_continuous as u32);
        if let Some(m) = mc {
            let limited_long = self.is_send_bitrate_limited
                && self.rate_control_time - self.send_bitrate_limited_start_time > 0.5;
            m.set_should_disable_large_frame_requests_when_initial_ramp_up(self.state == 1 && !limited_long);
            m.set_rate_limited_max_time_exceeded(limited_long);
        }
    }

    /// `__VCRateControlAlgorithmLayeredContinuousTier_DoRateControl` 0x205f95cd8 for VCRC
    /// statistics (type 2) on a call that is not audio-only. Returns Apple's return value.
    /// Audio-only calls take StabilizedNOWRD's own state machine (0x2061fc230), which is
    /// not ported: this returns false for them without changing anything.
    pub fn do_rate_control(&mut self, stats: &VcrcStatistics,
        mut mc: Option<&mut (dyn MediaController + '_)>, now: f64) -> bool {
        if mc.as_deref().map_or(false, |m| m.is_audio_only()) { return false; }
        // VCRCStatisticsPreStateTransitionUpdates 0x205f96ddc.
        if !self.update_internal_statistics(stats, mc.as_deref_mut(), now) { return false; }
        // UpdateSmartBrakeOutputWithVCRCStatistics 0x2061fe408 only fills its input record
        // when config+0x370 == 0 (no query), so it has no effect here.
        self.should_ramp_up_flag = false;
        self.should_ramp_down_flag = self.should_ramp_down();
        if !self.should_ramp_down_flag {
            self.should_ramp_up_flag = self.should_ramp_up(mc.as_deref());
        }
        self.update_target_bitrate_vcrc(now);
        self.post_state_transition_updates(mc.as_deref_mut());
        if let Some(m) = mc { m.observe_decision(self); }
        // 0x205f964f0..504: statuses cleared after every successful round.
        self.ramp_up_status = 0;
        self.ramp_down_status = 0;
        true
    }
}
