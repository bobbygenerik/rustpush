//! Ramp-down decision: `_VCRateControlAlgorithmStabilizedNOWRDPriv_ShouldRampDown` 0x2065c1bb4
//! and the Base helpers it calls. Addresses are instruction addresses (24A446 AVConference).
//! `_rampDownStatus` bits are Apple's: 0x1 frozen OWRD, 0x4 NOWRD, 0x8 NOWRD accumulation,
//! 0x10 BWE overshoot, 0x100 most-burst-loss, 0x200 loss event, 0x400 RTT, 0x800 ECN CE ratio.

use super::state::VcrcState;

/// `_VCRateControlCongestionLevel_BitrateRange` 0x2060de464 (bps → 0..=4).
pub fn congestion_level_bitrate_range(bitrate: u32) -> u32 {
    if bitrate < 100_001 { 0 }
    else if bitrate < 228_001 { 1 }
    else if bitrate < 597_001 { 2 }
    else if bitrate < 1_949_001 { 3 }
    else { 4 }
}

impl VcrcState {
    /// `_VCRateControlAlgorithmBasePriv_KeepOvershootingRampDownBandwidth` 0x20617a910.
    pub fn keep_overshooting_ramp_down_bandwidth(&self) -> bool {
        self.config.o_182 == 1 && self.remote_bandwidth_estimation != 0
            && self.rate_control_time - self.last_no_overshoot_bw_estimation_time > self.config.o_88
    }

    /// `_VCRateControlAlgorithmBasePriv_ResetLossEventBuffer` 0x20617ad2c (memset 0x800, index 0).
    pub fn reset_loss_event_buffer(&mut self) {
        self.loss_event_buffer = [Default::default(); 64];
        self.loss_event_buffer_index = 0;
    }

    /// `_VCRateControlAlgorithmBasePriv_ShouldRampDownDueToLossEvent` 0x20617aef0.
    pub fn should_ramp_down_due_to_loss_event(&mut self) -> bool {
        let c = &self.config;
        // 0x20617af0c..f7c: bad-trend count over the last min(o_dc, 64) records.
        let n = c.o_dc as i32;
        let mut by_trend = false;
        if n >= 1 {
            let mut count = 0i32;
            for k in 1..=n.min(64) as u32 {
                let j = (self.loss_event_buffer_index.wrapping_sub(k) & 0x3f) as usize;
                count += self.loss_event_buffer[j].bad_trend as i32;
            }
            if count >= n && self.nowrd > c.o_f8 {
                by_trend = self.current_tier_index >= c.o_50 as i32;
            }
        }
        // 0x20617af84..ff0: gate for the loss-ratio path.
        let ratio_thr = c.o_e8;
        if ratio_thr == 0.0 || self.packet_loss_rate_bad_trend_count >= c.o_e0 as i32 || self.state == 5 {
            return by_trend;
        }
        if self.first_loss_event_ramp_down_time == 0.0 {
            if self.state == 3 { return by_trend; }
            // b.pl: continue when the difference is >= o_1c0 or unordered.
            if self.rate_control_time - self.last_ramp_down_time_due_to_feedback < c.o_1c0 {
                return by_trend;
            }
        }
        // 0x20617aff4..08c: fraction of recent records (within o_f0 s) above the PLR threshold.
        let thr = self.packet_loss_rate_threshold;
        let mut ratio = 0.0;
        let mut i = 0u32;
        let mut hits = 0i32;
        let mut t0 = 0.0;
        let mut raw = self.loss_event_buffer_index.wrapping_sub(1);
        loop {
            let rec = self.loss_event_buffer[(raw & 0x3f) as usize];
            if i == 0 { t0 = rec.time; }
            if rec.time == 0.0 { break; }
            if t0 - rec.time > c.o_f0 {
                if i >= 2 { ratio = hits as f64 / i as f64; }
                break;
            }
            // [0x2066a3c68] = 0.95.
            if (rec.packet_loss_rate < 0.95 && rec.packet_loss_rate > thr) || rec.packet_loss_rate_video > thr {
                hits += 1;
            }
            i += 1;
            raw = raw.wrapping_sub(1);
            if i == 64 { ratio = hits as f64 / i as f64; break; }
        }
        // 0x20617b090..1a8: loss-based adaptation (LBA) trigger.
        let avg = self.packet_loss_rate_avg_long;
        // b.lt is taken on unordered too, so only a real `ratio >= o_e8` continues.
        let mut fired = false;
        if ratio >= ratio_thr && self.nowrd > c.o_f8 {
            if !(avg >= thr) {
                // log "...", RTT
            } else if self.first_loss_event_ramp_down_time != 0.0 {
                let prev = self.packet_loss_rate_avg_long_previous;
                let mut inc = (avg > prev && avg > c.o_300) as i32;
                // csinc ..., pl: 1 when at_start < avg.
                if self.packet_loss_rate_avg_long_at_lba_start < avg { inc = 1; }
                self.packet_loss_rate_bad_trend_count += inc;
            }
            if self.first_loss_event_ramp_down_time == 0.0 {
                self.first_loss_event_ramp_down_time = self.rate_control_time;
                self.packet_loss_rate_avg_long_at_lba_start = avg;
            }
            self.packet_loss_rate_avg_long_previous = avg;
            self.did_lba_ramp_down = true;
            fired = true;
        }
        // 0x20617b1ac..1d0: raise the PLR threshold toward the long average.
        if self.first_loss_event_ramp_down_time > 0.0 {
            let m = if thr < avg { thr } else { avg };
            let o = self.config.o_2f0;
            self.packet_loss_rate_threshold = if o < m { m } else { o };
        }
        if !fired { return by_trend; }
        // 0x20617b1d8..2b4: tier floor and loss-event bandwidth wall.
        let floor = self.config.o_50 as i32;
        if self.current_tier_index < floor || self.actual_bitrate < self.tier(floor) {
            self.reset_loss_event_buffer();
            return by_trend;
        }
        if self.previous_tier_index < self.current_tier_index {
            let tb = self.target_bitrate;
            let lim = self.loss_event_bandwidth_limit;
            if lim == 0 {
                self.loss_event_bandwidth_limit = tb;
                self.loss_event_bandwidth_confidence = self.loss_event_bandwidth_confidence.wrapping_add(1);
            } else {
                let diff = (tb.wrapping_sub(lim) as i32).unsigned_abs();
                // [0x2066a7d40] = 0.35.
                let conf = if lim as f64 * 0.35 > diff as f64 {
                    self.loss_event_bandwidth_confidence.wrapping_add(1)
                } else { 0 };
                self.loss_event_bandwidth_confidence = conf.min(10);
                self.loss_event_bandwidth_limit = (lim as f64 * 0.5 + tb as f64 / 2.0) as u32;
            }
        }
        true
    }

    /// `_VCRateControlAlgorithmStabilizedNOWRDPriv_ShouldRampDown` 0x2065c1bb4.
    pub fn should_ramp_down(&mut self) -> bool {
        self.fast_ramp_down_bitrate_range = self.config.o_148; // 0x2065c1bec..c04
        if !self.is_owrd_list_ready { return false; } // 0x2065c2278
        let mut r = self.keep_overshooting_ramp_down_bandwidth();
        if r {
            self.ramp_down_status |= 0x10;
            self.fast_ramp_down_bitrate_range = 0;
            self.nowrd_acc = 0.0;
        }
        // 0x2065c1c5c..ccc: OWRD frozen for longer than config+0x80.
        if !self.owrd_list.span_too_short && self.first_owrd_frozen_time > 0.0
            && self.rate_control_time - self.first_owrd_frozen_time > self.config.o_80 {
            self.ramp_down_status |= 0x1;
            self.first_owrd_frozen_time = 0.0;
            self.nowrd_acc = 0.0;
            r = true;
        }
        if !self.is_waiting_for_baseband_ramp_down {
            self.should_ramp_down_nowrd(&mut r);
        }
        self.should_ramp_down_tail(r)
    }

    /// 0x2065c1ce4..fdc: NOWRD and accumulated-NOWRD checks.
    fn should_ramp_down_nowrd(&mut self, r: &mut bool) {
        let limited = self.is_send_bitrate_limited;
        let settling_or_init = self.is_ramp_up_settling || self.state == 1;
        if settling_or_init {
            let thr = if limited { self.config.o_60 } else { self.config.o_70 };
            let nowrd = self.nowrd;
            let fire = nowrd > thr && self.previous_nowrd > thr && self.nowrd_short > 0.0;
            self.previous_nowrd = nowrd;
            if fire && !limited {
                self.ramp_down_status |= 0x4;
                self.nowrd_acc = 0.0;
                *r = true;
            }
        } else {
            self.previous_nowrd = 0.0;
        }
        // 0x2065c1dd8..e2c: accumulation threshold.
        let range = congestion_level_bitrate_range(self.target_bitrate) as usize;
        let consider = self.should_consider_congestion_level_for_nowrd_acc;
        let mut acc_thr = if consider {
            self.config.o_378[range] * self.round_trip_time_min_envelope
        } else {
            self.config.o_68
        };
        let mut above = false;
        // b.ls: continue when previous <= nowrd (unordered skips to the OWRD check).
        if self.previous_nowrd <= self.nowrd {
            if settling_or_init && !limited { acc_thr = self.config.o_78; }
            if !consider && self.should_suppress_ramp_down {
                acc_thr *= self.config.o_1a0; // 0x2065c1f20..f28 (log only before it)
            }
            if self.nowrd_acc > acc_thr {
                above = self.current_tier_index > self.config.o_18;
                self.nowrd_acc -= acc_thr;
                if self.nowrd_acc < 0.0 { self.nowrd_acc = 0.0; }
            }
        }
        // 0x2065c1f7c..fc8 / 0x2065c222c: positive OWRD seen within the last second.
        let last_pos = if self.owrd > 0.0 {
            self.last_positive_owrd_time = self.rate_control_time;
            self.rate_control_time
        } else {
            self.last_positive_owrd_time
        };
        if last_pos > 0.0 && self.rate_control_time - last_pos > 1.0 {
            self.nowrd_acc = 0.0;
            self.last_positive_owrd_time = self.rate_control_time;
        }
        if above {
            self.ramp_down_status |= 0x8;
            *r = true;
        }
    }

    /// 0x2065c1fe0..2204: burst loss, loss events, ECN, RTT.
    fn should_ramp_down_tail(&mut self, mut r: bool) -> bool {
        // Most burst loss (config+0x4c is 0 on the static profile, so this is off).
        let mbl = self.config.o_4c as i32;
        if mbl >= 1 && (self.most_burst_loss as i32) > mbl && self.packet_burst_loss != 0
            && self.current_tier_index >= self.config.o_50 as i32 {
            self.ramp_down_status |= 0x100;
            self.nowrd_acc = 0.0;
            self.did_mbl_ramp_down = true;
            r = true;
        }
        if self.should_ramp_down_due_to_loss_event() {
            self.ramp_down_status |= 0x200;
            self.last_loss_event_ramp_down_time = self.rate_control_time;
            self.reset_loss_event_buffer();
            self.nowrd_acc = 0.0;
            r = true;
        }
        // 0x2065c20fc..1ac: ECN CE ratio.
        let c = &self.config;
        let ratio = self.ecn_ce_ratio;
        let tb = self.target_bitrate;
        let ecn;
        if ratio > c.o_340 && tb >= c.o_350 {
            if self.tier(self.config.o_14) == self.config.o_350 {
                self.additional_tier_for_ramp_down = 1;
            }
            ecn = true;
        } else {
            let c = &self.config;
            let a = ratio > c.o_330 && tb > c.o_348;
            let b = tb > c.o_34c && ratio > c.o_338;
            ecn = a || b;
        }
        if ecn {
            self.ramp_down_status |= 0x800;
            self.reset_loss_event_buffer();
            self.nowrd_acc = 0.0;
            r = true;
        }
        // 0x2065c21b4..200: RTT above config+0x80 while above tier config+0x28.
        if self.round_trip_time > self.config.o_80 && self.current_tier_index > self.config.o_28 {
            self.ramp_down_status |= 0x400;
            self.nowrd_acc = 0.0;
            r = true;
        }
        r
    }
}
