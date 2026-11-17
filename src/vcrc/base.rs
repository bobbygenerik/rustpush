//! `VCRateControlAlgorithmBasePriv_*`, `VCRateControlUtil_*` and small helpers called by
//! UpdateInternalStatistics. u32 arithmetic is wrapping where Apple's is (unsigned registers).

use super::state::{OwrdList, VcrcState};
use crate::vcrc_config::VcrcConfig;

/// Unsigned remainder as Apple computes it: `a - (b ? a / b : 0) * b`.
fn umod(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { a % b }
}

/// `_VCRateControlAlgorithmBasePriv_GetDoubleTimeFromTimestamp` 0x20617a770:
/// `ts / rate + wraps * 64.0` (ucvtf, fdiv, fmadd with 0x4050000000000000 = 64.0).
pub fn get_double_time_from_timestamp(ts: u32, rate: u32, wraps: u32) -> f64 {
    wraps as f64 * 64.0 + ts as f64 / rate as f64
}

/// `_VCRateControlAlgorithmBasePriv_GetTimestampFromMicroTime` 0x20617a790:
/// `(NTPToMiddle32(MicroToNTP(t)) >> 6) & 0xffff`. Same conversion as our send timestamp.
pub fn get_timestamp_from_micro_time(unix_time_s: f64) -> u16 {
    crate::feedback_delay::ntp_send_timestamp(unix_time_s)
}

/// `_VCRateControlUtil_CalculateNOWRDDetailed` 0x205f03ae0.
pub fn calculate_nowrd_detailed(window: f64, min_span: f64, list: &mut OwrdList, cap: u32,
    min_samples: i32, check_span: bool) -> f64 {
    let mut rear = list.rear;
    let front = list.front;
    // 0x205f03b04..: first candidate start index = rear - minSamples + 1, modulo cap.
    let mut i = umod((rear as u32).wrapping_sub(min_samples as u32).wrapping_add(cap).wrapping_add(1), cap);
    let mut start = front;
    let mut overflow = false;
    if i != front {
        let mut iterations: u32 = 0;
        loop {
            start = i;
            if window < list.times[rear as usize] - list.times[i as usize] { break; }
            iterations = iterations.wrapping_add(1);
            if cap < (iterations & 0xffff) {
                // Logs "calculateNOWRDWithDuration exceeded max possible iterations".
                start = list.front;
                rear = list.rear;
                overflow = true;
                break;
            }
            i = umod(i.wrapping_add(cap.wrapping_sub(1)), cap);
            start = front;
            if i == front { break; }
        }
    }
    let _ = overflow;
    if check_span && list.times[rear as usize] - list.times[start as usize] < min_span {
        // 0x205f03b84..: span shorter than config+0x138 while state == 1.
        list.span_too_short = true;
        return 0.0;
    }
    // 0x205f03c98..: number of samples between start and rear (whole ring when equal).
    let diff = (rear as u32).wrapping_sub(start);
    let add = if diff == 0 || rear < start as i32 { cap } else { 0 };
    let n = diff.wrapping_add(add);
    let slope = if n < 0x7fff_fffe {
        let mid = start.wrapping_add(((n as i32 + 1) / 2) as u32);
        let mid_mod = umod(mid, cap);
        let mut count = (n + 2) >> 1;
        let (mut v_first, mut v_second, mut t_first, mut t_second) = (0.0, 0.0, 0.0, 0.0);
        let mut j: u32 = 0;
        let mut k = start;
        while count != 0 {
            let a = umod(k, cap) as usize;
            let b = umod(j.wrapping_add(mid_mod), cap) as usize;
            v_first += list.values[a];
            v_second += list.values[b];
            t_first += list.times[a];
            t_second += list.times[b];
            j = j.wrapping_add(1);
            k = k.wrapping_add(1);
            count -= 1;
        }
        (v_second - v_first) / (t_second - t_first)
    } else {
        f64::NAN
    };
    list.span_too_short = false;
    window * slope
}

/// `_VCRateControlUtil_CalculateCongestionMetrics` 0x205f038d8. Returns true when NOWRD was
/// recomputed (enough samples).
pub fn util_calculate_congestion_metrics(owrd: f64, time: f64, list: &mut OwrdList,
    config: &VcrcConfig, check_span: bool) -> bool {
    // 0x205f03938..: accept owrd < 8.0, and either owrd <= 0 or spacing >= config+0x130.
    if !(owrd < 8.0 && (owrd <= 0.0 || config.o_130 <= time - list.times[list.rear as usize])) {
        return false;
    }
    let cap = config.o_140;
    let mut count = list.count;
    let mut rear = list.rear as u32;
    if count != 0 {
        rear = umod(rear.wrapping_add(1), cap);
        list.rear = rear as i32;
    }
    list.values[rear as usize] = owrd;
    list.times[rear as usize] = time;
    if count < cap { count += 1; }
    list.count = count;
    if rear == list.front && count != 1 {
        list.front = umod(rear.wrapping_add(1), cap);
    }
    if config.o_144 <= count {
        list.nowrd = calculate_nowrd_detailed(config.o_120, config.o_138, list, cap,
            config.o_144 as i32, check_span);
        list.nowrd_short = calculate_nowrd_detailed(config.o_128, config.o_138, list,
            config.o_140, config.o_144 as i32, check_span);
        let cap = config.o_140;
        let prev = umod((list.rear as u32).wrapping_add(cap).wrapping_sub(1), cap);
        let acc = list.nowrd_acc + (list.nowrd
            * (list.times[list.rear as usize] - list.times[prev as usize])) / config.o_120;
        list.nowrd_acc = if 0.0 <= acc { acc } else { 0.0 };
        return true;
    }
    false
}

impl VcrcState {
    /// `_VCRateControlAlgorithmBasePriv_CalculateCongestionMetrics` 0x20617a7b0, called with
    /// (owrd, rateControlTime) from UpdateInternalStatistics.
    pub fn calculate_congestion_metrics(&mut self, owrd: f64, time: f64) {
        self.owrd_list.nowrd_acc = self.nowrd_acc;
        let check_span = self.state == 1;
        let ready = util_calculate_congestion_metrics(owrd, time, &mut self.owrd_list,
            &self.config, check_span);
        self.is_owrd_list_ready = ready;
        if ready {
            self.nowrd = self.owrd_list.nowrd;
            self.nowrd_short = self.owrd_list.nowrd_short;
            self.nowrd_acc = self.owrd_list.nowrd_acc;
            if self.owrd <= 0.0 {
                self.is_owrd_constant = false;
            } else {
                self.is_owrd_constant = self.nowrd_short == 0.0;
                if self.nowrd_short == 0.0 { return; }
            }
            self.last_owrd_change_time = self.rate_control_time;
        }
    }

    /// `_VCRateControlAlgorithmBasePriv_ResetTimers` 0x20617ad68.
    pub fn reset_timers(&mut self) {
        if self.enable_ramp_up_stuck_fix {
            self.last_no_overshoot_bw_estimation_time = 0.0;
            self.first_below_no_ramp_up_bandwidth_time = 0.0;
        }
    }

    /// `_VCRateControlAlgorithmBasePriv_CheckBandwidthOvershoot` 0x20617a834; `bwe` in bps.
    pub fn check_bandwidth_overshoot(&mut self, bwe: u32) {
        if bwe == 0 {
            self.last_no_overshoot_bw_estimation_time = self.rate_control_time;
            self.below_no_ramp_up_bandwidth = false;
            return;
        }
        let top: i32 = if self.max_bandwidth_estimation_tier == 0 { 0x42 }
            else { self.max_bandwidth_estimation_tier as i32 };
        let cur = self.current_tier_index;
        let idx = if cur + 1 >= top - 1 { top - 1 } else { cur + 1 };
        let next = self.tier(idx);
        let t = self.target_bitrate;
        let limit = (t as f64 + next.wrapping_sub(t) as f64 * self.config.o_90) as u32;
        self.is_overshoot = bwe < limit;
        if !self.is_overshoot {
            self.last_no_overshoot_bw_estimation_time = self.rate_control_time;
        }
        let mut j = self.config.o_d8 + cur;
        if top - 2 <= j { j = top - 2; }
        let lo = self.tier(j);
        let hi = self.tier(j + 1);
        let no_ramp_up = (lo as f64 + hi.wrapping_sub(lo) as f64 * self.config.o_90) as u32;
        self.below_no_ramp_up_bandwidth = bwe > no_ramp_up;
        if bwe <= no_ramp_up {
            self.first_below_no_ramp_up_bandwidth_time = 0.0;
            return;
        }
        if self.first_below_no_ramp_up_bandwidth_time == 0.0 {
            self.first_below_no_ramp_up_bandwidth_time = self.rate_control_time;
        }
    }

    /// `_VCRateControlAlgorithmBasePriv_UpdateLossEvent` 0x20617aa94.
    pub fn update_loss_event(&mut self) {
        if !(0.0 < self.round_trip_time) { return; }
        let idx = self.loss_event_buffer_index;
        let plr = self.packet_loss_rate;
        let consider_video = self.should_consider_video_in_loss_event;
        let rec = &mut self.loss_event_buffer[idx as usize];
        rec.time = self.rate_control_time;
        rec.packet_loss_rate = plr;
        rec.packet_loss_rate_video = if consider_video { self.packet_loss_rate_video } else { 0.0 };
        // [0x2066a3958] = 0.001.
        rec.bad_trend = 0.001 < plr - self.previous_packet_loss_rate
            && 0.0 < self.previous_packet_loss_rate;
        rec.ecn_low = self.current_ecn_stats as u16;
        rec.ecn_high = (self.current_ecn_stats >> 16) as u16;
        let mut d = plr;
        if consider_video && !(self.packet_loss_rate_video <= plr) {
            d = self.packet_loss_rate_video;
        }
        // config+0x428 / +0x308: optional cap on the PLR fed to the long average.
        if self.config.o_428 == 1 && self.config.o_308 != 0.0 && self.config.o_308 <= d {
            d = self.config.o_308;
        }
        let repeated_zero = if d == 0.0 {
            self.consecutive_zero_plr_count = self.consecutive_zero_plr_count.wrapping_add(1);
            1 < self.consecutive_zero_plr_count
        } else {
            self.consecutive_zero_plr_count = 0;
            false
        };
        if self.packet_loss_rate_avg_long == -1.0 {
            self.packet_loss_rate_avg_long = d;
        } else {
            // [0x2066a6218] = 0.03, or config+0x310 after repeated zero PLR when config+0x428 != 0.
            let w = if self.config.o_428 != 0 && d == 0.0 && repeated_zero { self.config.o_310 } else { 0.03 };
            self.packet_loss_rate_avg_long = d * w + (1.0 - w) * self.packet_loss_rate_avg_long;
            self.previous_packet_loss_rate_avg_factor = w;
        }
        self.loss_event_buffer_index = (idx + 1) & 0x3f;
        self.previous_packet_loss_rate = plr;
    }

    /// `_VCRateControlAlgorithmStabilizedNOWRDPriv_SetBitrateUnstable` 0x2061fc130.
    /// `now` stands in for Apple's `micro()` call.
    pub fn set_bitrate_unstable(&mut self, now: f64) {
        self.is_stable = false;
        self.rate_change_counter = self.rate_change_counter.wrapping_add(1);
        self.is_new_rate_sent_out = false;
        if self.state != 1 {
            self.ramp_up_frozen_time = self.rate_control_time;
            self.last_no_overshoot_bw_estimation_time = self.rate_control_time;
        }
        if self.config.o_208 == 1 {
            self.rate_change_system_time = now;
        }
    }

    /// `_VCRateControlAlgorithmBasePriv_CheckPaused` 0x20617a1e8. Pause/resume go through the
    /// class vtable's StateExit/StateEnter (object +0x20 / +0x18); for LayeredContinuousTier
    /// these are StabilizedNOWRD's (see lct.rs `vtable_state_exit` / `vtable_state_enter`).
    pub fn check_paused(&mut self) {
        if self.state == 1 { return; }
        let paused = self.paused; // VCRateControlAlgorithm_IsPaused 0x20617a3f8
        if self.state != 0 && paused {
            self.vtable_state_exit();
            self.state = 0;
            self.vtable_state_enter();
            self.pause_start_time = self.rate_control_time;
        } else if self.state == 0
            && (!paused || self.config.o_2a0 < self.rate_control_time - self.pause_start_time) {
            self.vtable_state_exit();
            self.state = 3;
            self.vtable_state_enter();
            self.paused = false; // VCRateControlAlgorithm_SetPaused(self, 0) 0x20617a580
        }
    }
}
