//! `_VCRateControlAlgorithmStabilizedNOWRDPriv_UpdateInternalStatistics` 0x2061fd3b4.
//! Source: sender-rate-20261116/0x2061fd3b4__...UpdateInternalStatistics.asm. Addresses in the
//! comments are instruction addresses in that function.

use super::base::{get_double_time_from_timestamp, get_timestamp_from_micro_time};
use super::media::{MediaController, PacketLossInput};
use super::state::VcrcState;
use super::stats::VcrcStatistics;

impl VcrcState {
    /// Returns Apple's return value: false when the report is the first one or its timestamp
    /// is rejected, true otherwise. `now` stands in for `micro()` inside SetBitrateUnstable.
    pub fn update_internal_statistics(&mut self, s: &VcrcStatistics,
        mut mc: Option<&mut dyn MediaController>, now: f64) -> bool {
        // 0x2061fd400..550: copy feedback fields.
        self.receive_jitter_queue_size = s.receive_jitter_queue_size;
        self.packet_burst_loss = s.burst_loss;
        self.most_burst_loss = s.most_burst_loss;
        self.remote_bandwidth_estimation = s.bandwidth_estimation;
        self.is_remote_bandwidth_estimation_stable = s.is_bwe_stable != 0;
        self.echoed_timestamp = s.echoed_timestamp as u16;
        self.queuing_delay_timestamp = s.queuing_delay as u16;
        self.worst_recent_burst_loss = self.worst_recent_burst_loss.max(self.packet_burst_loss);
        self.actual_bitrate = s.actual_bitrate;
        self.current_ecn_stats = s.ecn_stats;
        self.last_feedback_time = s.time;
        self.last_feedback_target_bitrate = self.target_bitrate;

        // 0x2061fd558 → 0x2061fe3c4: first report.
        if !self.is_first_timestamp_arrived {
            self.is_first_timestamp_arrived = true;
            self.first_feedback_arrival_time = s.time;
            self.previous_timestamp = s.timestamp;
            self.reset_timers();
            return false;
        }
        // 0x2061fd560..5a4: reject repeated or backwards timestamps (cold.1 logs; returns 0).
        let ts = s.timestamp as u32;
        let prev = self.previous_timestamp as u32;
        let delta = ts.wrapping_sub(prev);
        if delta == 0 || (delta & 0xffff) >= 0x7fff { return false; }
        if prev > ts { self.timestamp_wrap_around_counter = self.timestamp_wrap_around_counter.wrapping_add(1); }
        self.previous_timestamp = s.timestamp;
        self.rate_control_time = get_double_time_from_timestamp(ts, 0x400, self.timestamp_wrap_around_counter);
        self.receive_timestamp = get_timestamp_from_micro_time(s.time) as u32;
        self.owrd = s.owrd;

        // 0x2061fd604..644. With no media controller, LastVideoRefreshFrameTime 0x20604e9fc
        // logs and returns NaN (0x20604ea54), so the comparison is false.
        let last_refresh = mc.as_deref().map_or(f64::NAN, |m| m.last_video_refresh_frame_time());
        self.should_consider_video_in_loss_event = s.time - last_refresh < self.config.o_1c0;
        if self.last_allow_ramp_up_time == 0.0 { self.last_allow_ramp_up_time = self.rate_control_time; }

        // 0x2061fd670..738: packet loss.
        if let Some(m) = mc.as_deref_mut() {
            let input = PacketLossInput {
                echoed_timestamp: self.echoed_timestamp,
                audio_received_packets: s.audio_received_packets,
                video_received_packets: s.video_received_packets,
                total_received_kbytes: s.total_received_kbytes,
                packet_burst_loss: self.packet_burst_loss as u16,
                receive_jitter_queue_size: self.receive_jitter_queue_size,
                traffic_type: m.traffic_type(),
            };
            let out = m.compute_packet_loss(&input);
            if let Some(v) = out.most_burst_loss { self.most_burst_loss = v; }
            if let Some(v) = out.packet_loss_rate_audio { self.packet_loss_rate_audio = v; }
            if let Some(v) = out.packet_loss_rate { self.packet_loss_rate = v; }
            if let Some(v) = out.packet_loss_rate_video { self.packet_loss_rate_video = v; }
        } else {
            self.packet_loss_rate = s.packet_loss_rate;
        }
        if self.packet_loss_rate > self.config.o_2e0 { self.last_network_unstable_time = self.rate_control_time; }

        // 0x2061fd780..80c: RTT sample.
        let rtt_sample = if s.round_trip_time != 0.0 {
            Some(s.round_trip_time)
        } else {
            if self.receive_timestamp < self.echoed_timestamp as u32 {
                self.receive_timestamp = self.receive_timestamp.wrapping_add(0x10000);
            }
            let qd = self.queuing_delay_timestamp as u32;
            if qd == 0 {
                self.echoed_timestamp = 0xffff; // 0x2061fd970..980
                None
            } else {
                let rx = self.receive_timestamp.wrapping_sub(self.echoed_timestamp as u32);
                let i = if rx > qd { rx - qd } else { 1 }; // csinc at 0x2061fd800
                Some(get_double_time_from_timestamp(i, 0x400, 0))
            }
        };
        // 0x2061fd810..96c: RTT smoothing and minimum envelope.
        if let Some(rtt) = rtt_sample {
            self.most_recent_round_trip_time = rtt;
            let old = self.round_trip_time;
            // [0x2066a3d58] = 0.1, [0x2066a3918] = 0.9.
            self.round_trip_time = if old == 0.0 { rtt } else { old * 0.9 + rtt * 0.1 };
            let env = self.round_trip_time_min_envelope;
            if env == 0.0 {
                self.round_trip_time_min_envelope = self.round_trip_time;
            } else {
                let prev_rtt = self.previous_round_trip_time;
                let cur = self.round_trip_time;
                self.previous_round_trip_time = cur;
                if cur < env || prev_rtt - cur > self.config.o_1e0 {
                    // [0x2066a7f30] = 0.0002, [0x2066a7f38] = 0.2 (when env > sample).
                    let w = if env > rtt { 0.2 } else { 0.0002 };
                    self.round_trip_time_min_envelope = rtt * w + env * (1.0 - w);
                }
            }
            if !(self.worst_recent_round_trip_time > self.round_trip_time) {
                self.worst_recent_round_trip_time = self.round_trip_time;
            }
            if self.round_trip_time > self.config.o_2d8 {
                self.last_network_unstable_time = self.rate_control_time;
            }
        }

        // 0x2061fd98c..9ec.
        self.calculate_congestion_metrics(self.owrd, self.rate_control_time);
        if self.is_owrd_constant {
            if self.first_owrd_frozen_time == 0.0 { self.first_owrd_frozen_time = self.rate_control_time; }
        } else {
            self.first_owrd_frozen_time = 0.0;
        }

        // 0x2061fd9f0..bd4: send-rate-limited detection (skipped with no media controller).
        if let Some(m) = mc.as_deref() {
            let (iv, ov, ia, oa) = m.media_queue_settings();
            self.in_video_bitrate = iv;
            self.out_video_bitrate = ov;
            self.in_audio_bitrate = ia;
            self.out_audio_bitrate = oa;
            self.actual_bitrate = (ov + oa) as u32; // fcvtzu
            let tier = self.current_tier_index;
            let c = &self.config;
            let below = tier > c.o_18 && tier >= c.o_54 as i32
                && (self.actual_bitrate as f64) < c.o_108 * self.target_bitrate as f64;
            if below {
                if !(m.is_audio_only() && tier >= c.o_58 as i32) && !self.is_send_bitrate_limited {
                    self.send_bitrate_limited_start_time = self.rate_control_time;
                    self.is_send_bitrate_limited = true;
                }
            } else if self.is_send_bitrate_limited {
                let start = self.send_bitrate_limited_start_time;
                if start > 0.0 && self.rate_control_time - start > c.o_110 {
                    self.set_bitrate_unstable(now);
                }
                self.is_send_bitrate_limited = false;
            }
        }

        // 0x2061fdbd8..c60: ramp-up settle duration.
        let mut settle = self.config.o_a0;
        if self.config.o_187 == 1 && self.first_loss_event_ramp_down_time == 0.0 {
            let d1 = self.config.o_1b0 * self.round_trip_time;
            if d1 < settle { settle = d1; }
            if self.config.o_120 > settle { settle = self.config.o_120; }
        }
        self.ramp_up_settle_duration = settle;
        let echoed = self.echoed_timestamp;
        let qd_ts = self.queuing_delay_timestamp;

        // 0x2061fdc68..fc8: stabilization.
        if self.is_stable {
            if self.state != 1 && self.is_ramp_up_settling
                && self.rate_control_time - self.stabilization_time > settle {
                self.is_ramp_up_settling = false;
            }
        } else {
            match mc.as_deref() {
                None => {
                    if self.new_owrd_sample_collected == 0 {
                        self.stabilization_time = self.rate_control_time;
                        self.new_owrd_sample_collected = 1;
                        self.is_waiting_for_baseband_ramp_down = false;
                    }
                }
                Some(m) => {
                    let (mut counter, tx_time) = m.media_queue_rate_change_counter();
                    let mut tx_ts = get_timestamp_from_micro_time(tx_time);
                    if self.config.o_208 == 2 {
                        counter = self.rate_change_counter;
                        tx_ts = echoed;
                    } else if self.config.o_208 == 1 {
                        counter = self.rate_change_counter;
                        tx_ts = get_timestamp_from_micro_time(self.rate_change_system_time);
                    }
                    let rcc = self.rate_change_counter;
                    if self.last_rate_change_counter != rcc {
                        if counter < rcc {
                            self.new_owrd_sample_collected = 0;
                        } else {
                            self.is_new_rate_sent_out = true;
                            if qd_ts == 0 || (echoed.wrapping_sub(tx_ts)) > 0x7ffe {
                                self.new_owrd_sample_collected = 0;
                            } else {
                                self.stabilization_time = self.rate_control_time;
                                self.new_owrd_sample_collected = 1;
                                self.is_waiting_for_baseband_ramp_down = false;
                                self.last_rate_change_counter = rcc;
                            }
                        }
                    }
                }
            }
            // 0x2061fdeec..f1c.
            if self.should_ramp_up_skip_nowrd_stabilization {
                self.is_stable = true;
                self.new_owrd_sample_collected = 0;
            }
            // 0x2061fdf20..fc8.
            let n = self.new_owrd_sample_collected;
            if n != 0 {
                if self.rate_control_time - self.stabilization_time < self.config.o_120 || n < self.config.o_144 {
                    self.new_owrd_sample_collected = n.wrapping_add(1);
                } else {
                    self.is_stable = true;
                    self.new_owrd_sample_collected = 0;
                }
            }
        }

        // 0x2061fdfcc..028: congestion expiry.
        if self.is_congested && self.rate_control_time - self.last_congestion_time > self.config.o_118 {
            self.is_congested = false;
        }
        self.check_paused(); // 0x2061fe030

        // 0x2061fe034..160: abnormal bad network; [0x2066a7f18] = 90.0.
        if self.current_tier_index <= self.config.o_30 && self.owrd >= self.config.o_a8 {
            self.abnormal_network_detected = true;
        }
        if self.rate_control_time - self.last_network_unstable_time > 90.0 {
            self.abnormal_network_detected = false;
        }

        // 0x2061fe164..194: remote bandwidth estimate (kbps → bps).
        if self.config.o_182 == 1 {
            self.check_bandwidth_overshoot(self.remote_bandwidth_estimation.wrapping_mul(1000));
        } else {
            self.remote_bandwidth_estimation = 0;
        }
        self.update_loss_event(); // 0x2061fe1a4

        // 0x2061fe1a8..268: ramp-down suppression threshold.
        self.should_suppress_ramp_down = if self.config.o_194 == 1 {
            // ucvtf d0, w8, #1 = value / 2.0; fcvtzu back to u32.
            let mut thr = (self.tier(self.config.o_14) as f64 / 2.0) as u32;
            if self.round_trip_time > self.config.o_198 {
                let mut e = self.round_trip_time_min_envelope;
                loop {
                    e = e + e;
                    if !(e > 0.0) || !(self.round_trip_time > e) { break; }
                    thr = (thr as f64 / 2.0) as u32;
                }
            }
            self.ramp_down_suppression_bitrate_threshold = thr;
            self.target_bitrate <= thr
        } else {
            false
        };

        // 0x2061fe26c..354: ECN CE ratio over the loss-event ring.
        let window = {
            let x = self.config.o_358 * self.round_trip_time;
            let m = if self.config.o_368 < x { self.config.o_368 } else { x };
            if self.config.o_360 > m { self.config.o_360 } else { m }
        };
        self.ecn_ce_ratio = self.ecn_ce_ratio_over(window);
        true
    }

    /// Loop at 0x2061fe2d4..344. Walks back from the newest record (index − 1) while the record
    /// is within `window` seconds of the newest and fewer than 64 have been visited; the last
    /// visited record is the reference. Ratio = Δ(+0x1c) / Δ(+0x1a), both as u16 differences.
    fn ecn_ce_ratio_over(&self, window: f64) -> f64 {
        let mut j_raw = self.loss_event_buffer_index.wrapping_sub(1);
        let mut w12: i32 = -63;
        let (mut t0, mut hi0, mut lo0) = (0.0f64, 0u16, 0u16);
        let mut j;
        loop {
            j = (j_raw & 0x3f) as usize;
            let rec = &self.loss_event_buffer[j];
            if w12 == -63 { hi0 = rec.ecn_high; lo0 = rec.ecn_low; t0 = rec.time; }
            if rec.time == 0.0 { return 0.0; }
            // ccmp: continue only when (t0 - time <= window) and the pre-increment counter != 0.
            let cont = t0 - rec.time <= window && w12 != 0;
            w12 += 1;
            j_raw = j_raw.wrapping_sub(1);
            if !cont { break; }
        }
        let rec = &self.loss_event_buffer[j];
        if rec.ecn_low == lo0 { return 0.0; }
        let d_lo = lo0.wrapping_sub(rec.ecn_low);
        let d_hi = hi0.wrapping_sub(rec.ecn_high);
        d_hi as f64 / d_lo as f64
    }
}
