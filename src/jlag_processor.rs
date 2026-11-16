//! AVConference JLagProcessor ported line-for-line from iOS 27.0.1
//! (24A446, iPhone17,5 dyld shared cache, AVConference), transliterated
//! directly from `FaceTimeJLagProcessor.kt`.
//!
//! Source functions in AVConference:
//! - 0x2064a5d30 _JLagProcessor_Initialize
//! - 0x2064a5dc0 _JLagProcessor_NewStream
//! - 0x2064a5f48 _JLagProcessor_ResetLagReference
//! - 0x2064a5f60 _JLagProcessor_GetOWRD
//! - 0x2064a5f78 _JLagProcessor_GetStatistics
//! - 0x2064a5f88 _JLagProcessor_Process
//! - 0x2064a6c1c _JLagProcessor_UpdateAverages
//! - 0x2064a6cc4 _JLagProcessor_RecordOWRD
//! - 0x2064a6fa0 _JLagProcessor_CalculatePacketLag
//!
//! Exact constants cited from dyld shared cache:
//! - 0x2066af5c0: 0.0 (initial avgLag)
//! - 0x2066af5c8: 0.01 (initial avgDev)
//! - 0x2066af5d0: 0.0 (initial statsLag)
//! - 0x2066af5d8: 32767.0 (initial statsLastLag)
//! - 0x2066af590: 32767.0 (uninitialized lastLag sentinel)
//! - 0x2066af598: 0.03500000037252903 (35 ms fallback lag on initial out-of-order)
//! - 0x2066af5a0: 0.019999999552965164 (20 ms base spike threshold offset)
//! - 0x2066af5a8: 0.02500000037252903 (25 ms settle state gate)
//! - 0x2066af5b0: 0.003 (3 ms average lag bias threshold)
//! - 0x2066af5b8: -0.003 (-3 ms average lag bias deduction)
//! - 0x2066a3d40: 0.005 (5 ms initial dev bias in state 1)
//! - 0x2066a3d48: 0.0001 (OWRD short filter alpha)
//! - 0x2066a3d50: 0.9999 (OWRD short filter beta)
//! - 0x2066a3d58: 0.1 (OWRD long filter alpha)
//! - 0x2066a3918: 0.9 (OWRD long filter beta)
//! - 0x2066a3938: 0.01 (10 ms peak spike lag tolerance)
//! - 0x2066a3c60: 0.05 (peak spike lag decay alpha)
//! - 0x2066a3c68: 0.95 (peak spike lag decay beta)

use std::any::Any;
use std::sync::Arc;

/// Stack layout of lag packet struct passed as x1 to JLagProcessor_Process / NewStream
/// in _VCJitterBuffer_EnqueuePacket (0x20650e074, sp + 0x160).
#[derive(Debug, Clone, Default)]
pub struct LagPacket {
    pub sequence_number: u16,
    pub timestamp: u32,
    pub remote_time: f64,
    pub arrival_sec: f64,
    pub field18: i32,
    pub field20: i32,
    pub flag24: i8,
    pub has_field0x28: bool,
}

/// Statistics snapshot returned by _JLagProcessor_GetStatistics (0x2064a5f78).
/// Corresponds to 0x18-byte struct at [processor + 0x1b0].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JLagStatistics {
    pub lag: f64,
    pub last_lag: f64,
    pub ref_timestamp: u32,
    pub wrap_count: u32,
}

/// Config structure passed to _JLagProcessor_Initialize (0x2064a5d30).
#[derive(Clone, Default)]
pub struct JLagConfig {
    pub config_flag: i64,
    pub context: Option<Arc<dyn Any + Send + Sync>>,
}

pub struct JLagProcessor {
    // Offset +0x00: lastTimestampDelta (uint32)
    pub last_timestamp_delta: u32,

    // Offset +0x04: wrapCount (uint32)
    pub wrap_count: u32,

    // Offset +0x08: oooCounter (uint32)
    pub ooo_counter: u32,

    // Offset +0x0c: packetCounter (uint32)
    pub packet_counter: u32,

    // Offset +0x10: avgLag (double), initialized to 0.0 (constant 0x2066af5c0)
    pub avg_lag: f64,

    // Offset +0x18: avgDev (double), initialized to 0.01 (constant 0x2066af5c8)
    pub avg_dev: f64,

    // Offset +0x20: owrdShort (double)
    pub owrd_short: f64,

    // Offset +0x28: owrdLong (double)
    pub owrd_long: f64,

    // Offset +0x30: owrd (float, 32-bit single precision)
    pub owrd: f32,

    // Offset +0x34: inSpike (int32)
    pub in_spike: i32,

    // Offset +0x38: spikeStartArrivalSec (double)
    pub spike_start_arrival_sec: f64,

    // Offset +0x40: spikeStartTimestampTimeSec (double)
    pub spike_start_timestamp_time_sec: f64,

    // Offset +0x48: spikeThreshold (double)
    pub spike_threshold: f64,

    // Offset +0x50: peakSpikeLag (double)
    pub peak_spike_lag: f64,

    // Offset +0x58..+0x147: ringBuffer of 30 doubles (240 bytes)
    pub ring_buffer: [f64; 30],

    // Offset +0x148: ringIndex (int32)
    pub ring_index: i32,

    // Offset +0x14c: spikeCount (int32)
    pub spike_count: i32,

    // Offset +0x150: tsRate (uint32)
    pub ts_rate: u32,

    // Offset +0x154: samplesPerFrame (uint32)
    pub samples_per_frame: u32,

    // Offset +0x158: resetPending (byte / boolean)
    pub reset_pending: bool,

    // Offset +0x159: initializedSecondary (byte / boolean)
    pub initialized_secondary: bool,

    // Offset +0x160: refArrivalSec (double)
    pub ref_arrival_sec: f64,

    // Offset +0x168: refTimestamp (uint32)
    pub ref_timestamp: u32,

    // Offset +0x16c: refSeq (uint16)
    pub ref_seq: u16,

    // Offset +0x170: lastLag (double), initialized to 32767.0 (constant 0x2066af590)
    pub last_lag: f64,

    // Offset +0x180: secondaryRefArrivalSec (double)
    pub secondary_ref_arrival_sec: f64,

    // Offset +0x188: secondaryRefTimestamp (uint32)
    pub secondary_ref_timestamp: u32,

    // Offset +0x18c: secondaryRefSeq (uint16)
    pub secondary_ref_seq: u16,

    // Offset +0x190: lastSecondaryLag (double), initialized to 32767.0
    pub last_secondary_lag: f64,

    // Offset +0x198: counter198 (uint32)
    pub counter198: u32,

    // Offset +0x1a0: configFlag (long)
    pub config_flag: i64,

    // Offset +0x1a8: configContext (Any?)
    pub config_context: Option<Arc<dyn Any + Send + Sync>>,

    // Offset +0x1b0: statsLag (double)
    pub stats_lag: f64,

    // Offset +0x1b8: statsLastLag (double)
    pub stats_last_lag: f64,

    // Offset +0x1c0: statsRefTimestamp (uint32)
    pub stats_ref_timestamp: u32,

    // Offset +0x1c4: statsWrapCount (uint32)
    pub stats_wrap_count: u32,
}

impl Default for JLagProcessor {
    fn default() -> Self {
        Self {
            last_timestamp_delta: 0,
            wrap_count: 0,
            ooo_counter: 0,
            packet_counter: 0,
            avg_lag: 0.0,
            avg_dev: 0.01,
            owrd_short: 0.0,
            owrd_long: 0.0,
            owrd: 0.0,
            in_spike: 0,
            spike_start_arrival_sec: 0.0,
            spike_start_timestamp_time_sec: 0.0,
            spike_threshold: 0.0,
            peak_spike_lag: 0.0,
            ring_buffer: [0.0; 30],
            ring_index: 0,
            spike_count: 0,
            ts_rate: 0,
            samples_per_frame: 0,
            reset_pending: false,
            initialized_secondary: false,
            ref_arrival_sec: 0.0,
            ref_timestamp: 0,
            ref_seq: 0,
            last_lag: 32767.0,
            secondary_ref_arrival_sec: 0.0,
            secondary_ref_timestamp: 0,
            secondary_ref_seq: 0,
            last_secondary_lag: 32767.0,
            counter198: 0,
            config_flag: 0,
            config_context: None,
            stats_lag: 0.0,
            stats_last_lag: 32767.0,
            stats_ref_timestamp: 0,
            stats_wrap_count: 0,
        }
    }
}

impl JLagProcessor {
    pub fn new() -> Self {
        Self::default()
    }

    /// _JLagProcessor_Initialize (0x2064a5d30)
    pub fn initialize(&mut self, config: Option<&JLagConfig>) -> bool {
        // 0x2064a5d44: cmp x0, #0; ccmp x1, #0, #4, ne
        let Some(config) = config else {
            return false;
        };
        // 0x2064a5d54: ldr q0, [x1]; str q0, [x0, #0x1a0]
        self.config_flag = config.config_flag;
        self.config_context = config.context.clone();
        true
    }

    /// _JLagProcessor_NewStream (0x2064a5dc0)
    /// Apple writes only: +0x16c, +0x18c, +0x168, +0x188, +0x160, +0x180, +0x170, +0x190,
    /// +0x0, +0x4, +0x10, +0x18, +0x150, +0x154, +0x1b0, +0x1b8, +0x1c4, +0x1c0.
    /// State variables +0x8, +0xc, +0x20..+0x58, +0x148, +0x14c, +0x158, +0x159, +0x198 survive.
    pub fn new_stream(
        &mut self,
        packet: Option<&LagPacket>,
        ts_rate: u32,
        samples_per_frame: u32,
    ) -> bool {
        // 0x2064a5df0..0x2064a5dfc: null/zero checks
        let Some(packet) = packet else {
            return false;
        };
        if ts_rate == 0 || samples_per_frame == 0 {
            return false;
        }

        // 0x2064a5e08..0x2064a5e1c: store initial sequenceNumber, timestamp
        self.secondary_ref_seq = packet.sequence_number;
        self.ref_seq = packet.sequence_number;
        self.secondary_ref_timestamp = packet.timestamp;
        self.ref_timestamp = packet.timestamp;

        // 0x2064a5e20..0x2064a5e28: store initial arrivalSec
        self.secondary_ref_arrival_sec = packet.arrival_sec;
        self.ref_arrival_sec = packet.arrival_sec;

        // 0x2064a5e2c..0x2064a5e38: store 32767.0 (0x40dfffc000000000) to +0x190 and +0x170
        self.last_secondary_lag = 32767.0;
        self.last_lag = 32767.0;

        // 0x2064a5e3c: str xzr, [x22] -> clears lastTimestampDelta and wrapCount (+0x0 and +0x4)
        self.last_timestamp_delta = 0;
        self.wrap_count = 0;

        // 0x2064a5e40..0x2064a5e48: pair at 0x2066af5c0 -> (0.0, 0.01)
        self.avg_lag = 0.0;
        self.avg_dev = 0.01;

        // 0x2064a5e4c..0x2064a5e50: samplesPerFrame and tsRate
        self.samples_per_frame = samples_per_frame;
        self.ts_rate = ts_rate;

        // 0x2064a5e54..0x2064a5e5c: pair at 0x2066af5d0 -> (0.0, 32767.0)
        self.stats_lag = 0.0;
        self.stats_last_lag = 32767.0;

        // 0x2064a5e60: str wzr, [x22, #0x1c4] -> statsWrapCount = 0
        self.stats_wrap_count = 0;
        // 0x2064a5e64: str w8, [x22, #0x1c0] -> statsRefTimestamp = packet.timestamp
        self.stats_ref_timestamp = packet.timestamp;

        true
    }

    /// _JLagProcessor_ResetLagReference (0x2064a5f48)
    pub fn reset_lag_reference(&mut self) {
        // 0x2064a5f4c: ldrb w8, [x0, #0x158]; tbnz w8, #0, 0x2064a5f5c
        if !self.reset_pending {
            self.reset_pending = true;
        }
    }

    /// _JLagProcessor_GetOWRD (0x2064a5f60)
    pub fn get_owrd(&self) -> f32 {
        // 0x2064a5f64: ldr s0, [x0, #0x30]
        self.owrd
    }

    /// _JLagProcessor_GetStatistics (0x2064a5f78)
    pub fn get_statistics(&self) -> JLagStatistics {
        // 0x2064a5f78: add x8, x0, #0x1b0
        JLagStatistics {
            lag: self.stats_lag,
            last_lag: self.stats_last_lag,
            ref_timestamp: self.stats_ref_timestamp,
            wrap_count: self.stats_wrap_count,
        }
    }

    /// _JLagProcessor_CalculatePacketLag (0x2064a6fa0)
    pub fn calculate_packet_lag(&mut self, packet: &mut LagPacket) -> (f64, f64) {
        // 0x2064a6fd0: str wzr, [x0, #0x8] -> oooCounter = 0
        self.ooo_counter = 0;

        // 0x2064a6fd4..0x2064a6fdc: tsDelta = packet.timestamp - refTimestamp
        let ts_delta = packet.timestamp.wrapping_sub(self.ref_timestamp);

        // 0x2064a6fe0..0x2064a6fe4: diff = tsDelta - lastTimestampDelta
        let diff = ts_delta.wrapping_sub(self.last_timestamp_delta);

        // 0x2064a6fe8..0x2064a6ff0: cmp diff, #0x7ffffffe; b.hi 0x2064a70a4
        if diff <= 0x7ffffffe {
            // 0x2064a6ff4..0x2064a6ff8: cmp tsDelta, lastTimestampDelta; b.hs 0x2064a70a0
            if ts_delta < self.last_timestamp_delta {
                // 0x2064a7090..0x2064a7098: wrapCount++
                self.wrap_count += 1;
            }
            // 0x2064a70a0: str w25, [x20] -> lastTimestampDelta = tsDelta
            self.last_timestamp_delta = ts_delta;
        }

        // 0x2064a70a4..0x2064a70ac: elapsedArrival = packet.arrivalSec - refArrivalSec
        let elapsed_arrival = packet.arrival_sec - self.ref_arrival_sec;

        // 0x2064a70b0..0x2064a70c4: fmadd d1, d1, d3, d2 -> totalTsDelta = wrapCount * 2^32 + tsDelta
        let total_ts_delta = (self.wrap_count as f64 * 4294967296.0) + ts_delta as f64;

        // 0x2064a70c8..0x2064a70d4: transitSec = totalTsDelta / tsRate; primaryLag = elapsedArrival - transitSec
        let cr = self.ts_rate as f64;
        let transit_sec = total_ts_delta / cr;
        let primary_lag = elapsed_arrival - transit_sec;

        // 0x2064a70dc: str d0, [x21, #0x8] -> store elapsedArrival into packet.remoteTime
        packet.remote_time = elapsed_arrival;

        // 0x2064a70e0..0x2064a7100: secondary lag calculation
        let sec_ts_delta = packet.timestamp.wrapping_sub(self.secondary_ref_timestamp);
        let sec_transit_sec = sec_ts_delta as f64 / cr;
        let sec_elapsed = packet.arrival_sec - self.secondary_ref_arrival_sec;
        let secondary_lag = sec_elapsed - sec_transit_sec;

        (primary_lag, secondary_lag)
    }

    /// _JLagProcessor_RecordOWRD (0x2064a6cc4)
    pub fn record_owrd(&mut self, lag: f64) {
        // 0x2064a6ce4: ldr w8, [x0, #0x34]; cbz w8, 0x2064a6d90
        if self.in_spike != 0 {
            return;
        }

        // 0x2064a6d94..0x2064a6da4: if ((lag - owrdLong) > 5.0) return
        if (lag - self.owrd_long) > 5.0 {
            return;
        }

        // 0x2064a6da8..0x2064a6db4: if ((owrdShort - lag) > 5.0) return
        if (self.owrd_short - lag) > 5.0 {
            return;
        }

        // 0x2064a6e7c..0x2064a6e80: if (packetCounter == 0) return
        if self.packet_counter == 0 {
            return;
        }

        // Short filter: 0.0001 * lag + 0.9999 * owrdShort (constants 0x2066a3d48, 0x2066a3d50)
        self.owrd_short = self.owrd_short * 0.9999 + lag * 0.0001;

        // Long filter: 0.1 * lag + 0.9 * owrdLong (constants 0x2066a3d58, 0x2066a3918)
        self.owrd_long = self.owrd_long * 0.9 + lag * 0.1;

        // 0x2064a6eb8: diff = owrdLong - owrdShort
        let mut diff = self.owrd_long - self.owrd_short;
        if diff < 0.0 {
            // 0x2064a6ec4: owrdShort = owrdLong; diff = 0.0
            self.owrd_short = self.owrd_long;
            diff = 0.0;
        }

        // 0x2064a6ed0: str s0, [x19, #0x30]
        self.owrd = diff as f32;
    }

    /// _JLagProcessor_UpdateAverages (0x2064a6c1c)
    pub fn update_averages(
        &mut self,
        lag: f64,
        packet: &LagPacket,
        samples_per_frame: i32,
        is_state2: bool,
    ) {
        // 0x2064a6c1c..0x2064a6c20: spf = if (samplesPerFrame != 0) samplesPerFrame else packet.field18
        let spf = if samples_per_frame != 0 {
            samples_per_frame
        } else {
            packet.field18
        };

        // 0x2064a6c24..0x2064a6c5c: alpha computation
        let alpha = if is_state2 {
            0.5
        } else {
            let rate = self.ts_rate as f64;
            let f20 = packet.field20 as f64;
            let spf_d = spf as f64;
            let denom = ((rate * 20.0) / spf_d) / f20 + 1.0;
            2.0 / denom
        };

        // 0x2064a6c64..0x2064a6c70: newAvgLag = lag * alpha + avgLag * (1.0 - alpha)
        let new_avg_lag = lag * alpha + self.avg_lag * (1.0 - alpha);
        self.avg_lag = new_avg_lag;

        // 0x2064a6c74..0x2064a6c7c: newAvgDev = Math.abs(lag - newAvgLag) * alpha + avgDev * (1.0 - alpha)
        let new_avg_dev = (lag - new_avg_lag).abs() * alpha + self.avg_dev * (1.0 - alpha);
        self.avg_dev = new_avg_dev;

        // 0x2064a6c88: cmp newAvgLag, 0.003 (constant 0x2066af5b0)
        if new_avg_lag > 0.003 {
            // 0x2064a6c98: avgLag = newAvgLag + (-0.003) (constant 0x2066af5b8)
            self.avg_lag = new_avg_lag - 0.003;
            // 0x2064a6ca4..0x2064a6cbc: refTimestamp -= (uint)(tsRate * 0.003)
            let shift = (self.ts_rate as f64 * 0.003) as u32;
            self.ref_timestamp = self.ref_timestamp.wrapping_sub(shift);
        }
    }

    /// _JLagProcessor_Process (0x2064a5f88)
    pub fn process(
        &mut self,
        packet: Option<&mut LagPacket>,
        samples_per_frame: i32,
        queue_samples: i32,
        state: i32,
    ) -> f64 {
        // 0x2064a5fd0..0x2064a5fe4: argument validations
        let Some(packet) = packet else {
            return f64::NAN;
        };
        if state > 5 || (samples_per_frame | queue_samples) < 0 {
            return f64::NAN;
        }

        // 0x2064a5fe8..0x2064a5ff0: packet flag24 == 1 skipped
        if packet.flag24 == 1 {
            return f64::NAN;
        }

        // 0x2064a6164..0x2064a6178: sequence diff check against 0x7fff
        let seq_diff = ((packet.sequence_number as i32).wrapping_sub(self.ref_seq as i32)) & 0xffff;
        if seq_diff >= 0x7fff {
            // Out-of-order packet:
            // 0x2064a617c..0x2064a6190: oooCounter <= 3 && packetCounter <= 2999 (0xbb7)
            if self.ooo_counter <= 3 && self.packet_counter <= 2999 {
                self.ooo_counter += 1;
                // 0x2064a619c..0x2064a61b4: fallback to 0.035 if lastLag == 32767.0
                let lag_result = if self.last_lag == 32767.0 {
                    0.03500000037252903
                } else {
                    self.last_lag
                };
                self.stats_lag = lag_result;
                self.stats_last_lag = self.last_lag;
                self.stats_wrap_count = self.wrap_count;
                self.stats_ref_timestamp = self.ref_timestamp;
                return self.finish_process(
                    lag_result,
                    packet,
                    samples_per_frame,
                    queue_samples,
                    state,
                );
            }
        }

        // 0x2064a61cc: calculate primary and secondary lag
        let (mut primary_lag, mut secondary_lag) = self.calculate_packet_lag(packet);
        let mut lag_result: f64;

        // 0x2064a61d0: check state <= 4 vs state == 5
        if state <= 4 {
            // 0x2064a61d8..0x2064a61ec: update reference to current packet
            self.ref_seq = packet.sequence_number;
            self.ref_arrival_sec = packet.arrival_sec;
            self.ref_timestamp = packet.timestamp;

            // 0x2064a61f0..0x2064a6218: check initializedSecondary
            if !self.initialized_secondary && !packet.has_field0x28 {
                let recalced = self.calculate_packet_lag(packet);
                primary_lag = recalced.0;
                secondary_lag = recalced.1;
                self.initialized_secondary = true;
            }

            // 0x2064a621c..0x2064a6224: reset counters
            self.wrap_count = 0;
            self.packet_counter = 0;
            lag_result = primary_lag;
        } else {
            // state == 5:
            // 0x2064a622c..0x2064a6234: packetCounter++
            self.packet_counter += 1;
            lag_result = primary_lag;

            let should_switch_to_secondary: bool;
            // 0x2064a623c..0x2064a6248: fabs primaryLag > 5.0
            if primary_lag.abs() > 5.0 {
                // Spurious primary lag (0x2064a6310): lagResult = lastLag
                lag_result = self.last_lag;

                // 0x2064a6318..0x2064a6344: secondary reference validation
                let abs_sec = secondary_lag.abs();
                let diff_sec = (secondary_lag - self.last_secondary_lag).abs();
                if abs_sec < 2.5 && diff_sec < 2.5 {
                    let pre_good = self.counter198;
                    self.counter198 += 1;
                    // 0x2064a6348: cmp w8, #9 where w8 is pre-increment value; b.le 0x2064a6358
                    if pre_good > 9 {
                        should_switch_to_secondary = true;
                    } else {
                        // 0x2064a634c b.le 0x2064a6358: check resetPending
                        should_switch_to_secondary = self.reset_pending;
                    }
                } else {
                    // 0x2064a6328/0x2064a6338 b.pl 0x2064a6354: counter198 = 0, check resetPending
                    self.counter198 = 0;
                    should_switch_to_secondary = self.reset_pending;
                }
            } else {
                // primaryLag <= 5.0: 0x2064a6248 b.le 0x2064a6354: counter198 = 0, check resetPending
                self.counter198 = 0;
                should_switch_to_secondary = self.reset_pending;
            }

            if should_switch_to_secondary {
                // Switch to secondary reference (0x2064a6434..0x2064a64f4):
                self.ref_seq = self.secondary_ref_seq;
                self.ref_arrival_sec = self.secondary_ref_arrival_sec;
                self.ref_timestamp = self.secondary_ref_timestamp;
                self.last_timestamp_delta = 0;
                self.wrap_count = 0;
                self.counter198 = 0;
                self.packet_counter = 0;
                self.reset_pending = false;
                lag_result = secondary_lag;
                self.owrd_short = 0.0;
                self.owrd_long = 0.0;
                self.owrd = 0.0;
            }
        }

        // 0x2064a6508..0x2064a6524: update secondary reference
        self.secondary_ref_timestamp = packet.timestamp;
        self.secondary_ref_arrival_sec = packet.arrival_sec;
        self.secondary_ref_seq = packet.sequence_number;
        self.last_secondary_lag = secondary_lag;

        // 0x2064a6528..0x2064a653c: update stats
        self.stats_lag = lag_result;
        self.stats_last_lag = self.last_lag;
        self.stats_wrap_count = self.wrap_count;
        self.stats_ref_timestamp = self.ref_timestamp;

        self.finish_process(lag_result, packet, samples_per_frame, queue_samples, state)
    }

    fn finish_process(
        &mut self,
        lag_result: f64,
        packet: &LagPacket,
        samples_per_frame: i32,
        queue_samples: i32,
        state: i32,
    ) -> f64 {
        // 0x2064a6540..0x2064a6550: state branching
        match state {
            1 => {
                // 0x2064a655c: constant 0x2066af5a8 = 0.02500000037252903
                if lag_result <= 0.02500000037252903 {
                    self.avg_lag = lag_result;
                    self.avg_dev = lag_result.abs() + 0.005; // constant 0x2066a3d40
                    self.owrd_short = lag_result;
                    self.owrd_long = lag_result;
                }
            }
            2 => {
                // 0x2064a6598: _JLagProcessor_UpdateAverages
                self.update_averages(lag_result, packet, samples_per_frame, true);
                // 0x2064a65a4: _JLagProcessor_RecordOWRD
                self.record_owrd(lag_result);
            }
            5 => {
                // 0x2064a65c4..0x2064a6614: spf resolution (w25)
                let w25 = if samples_per_frame > 0 {
                    samples_per_frame
                } else if packet.field18 > 0 {
                    packet.field18
                } else {
                    self.samples_per_frame as i32
                };
                let cr = self.ts_rate as f64;
                // 0x2064a6624..0x2064a6630: d3 = queueSamples / tsRate
                let d3 = (queue_samples as u32 as f64) / cr;

                if self.in_spike != 0 {
                    // 4a. 0x2064a663c..0x2064a664c
                    let d4 = lag_result - self.avg_lag;
                    if d4 <= self.spike_threshold {
                        // 0x2064a664c: LEAVE spike
                        self.in_spike = 0;
                    } else {
                        // 4b. 0x2064a6650..0x2064a6658: n = ++spikeCount
                        self.spike_count += 1;
                        let n = self.spike_count;
                        let mut check_timeout_and_peak = false;

                        if n < 5 {
                            // 4c. 0x2064a665c..0x2064a6660 -> STORE
                            self.ring_buffer[n as usize] = lag_result;
                            check_timeout_and_peak = true;
                        } else if lag_result > self.peak_spike_lag + 0.01 {
                            // 4d. 0x2064a6664..0x2064a6678 -> LEAVE (constant 0x2066a3938 = 0.01)
                            self.in_spike = 0;
                        } else if n <= 29 {
                            // 4e. 0x2064a6868..0x2064a686c -> STORE
                            self.ring_buffer[n as usize] = lag_result;
                            check_timeout_and_peak = true;
                        } else {
                            // 4f. Flatness block 0x2064a6a84..0x2064a6b60
                            self.ring_buffer[self.ring_index as usize] = lag_result;
                            self.ring_index = (self.ring_index + 1) % 30;
                            let mut sum1 = 0.0;
                            let mut sum2 = 0.0;
                            for i in 0..15 {
                                sum1 += self.ring_buffer[((self.ring_index + i) % 30) as usize];
                                sum2 +=
                                    self.ring_buffer[((self.ring_index + 15 + i) % 30) as usize];
                            }
                            let flatness_threshold =
                                (1.25 * self.avg_dev + 0.019999999552965164 + d3) * 15.0;
                            if (sum1 - sum2).abs() >= flatness_threshold {
                                // Not flat -> TIMEOUT_CHECK (no ring[n] store)
                                check_timeout_and_peak = true;
                            } else {
                                // Flat -> LEAVE
                                self.in_spike = 0;
                            }
                        }

                        if check_timeout_and_peak {
                            // TIMEOUT_CHECK (0x2064a6878..)
                            let mut timed_out = false;
                            if self.config_flag == 1 {
                                let cur_time = ((self.wrap_count as f64 * 4294967296.0)
                                    + packet.timestamp as f64)
                                    / cr;
                                if (cur_time - self.spike_start_timestamp_time_sec) > 8.0 {
                                    timed_out = true;
                                }
                            } else {
                                let f20_clamped = if packet.field20 > 0 {
                                    packet.field20
                                } else {
                                    1
                                };
                                let denom = (f20_clamped * w25) as f64;
                                let limit = ((cr * 8.0) / denom) as i64 as u32;
                                if (self.spike_count as u32) > limit {
                                    timed_out = true;
                                }
                            }

                            if timed_out {
                                self.in_spike = 0; // LEAVE
                            } else {
                                // PEAK (0x2064a68b8..0x2064a6a14)
                                if lag_result > self.peak_spike_lag {
                                    self.peak_spike_lag = lag_result;
                                } else {
                                    self.peak_spike_lag =
                                        lag_result * 0.05 + self.peak_spike_lag * 0.95;
                                }
                            }
                        }
                    }
                } else {
                    // 0x2064a671c..0x2064a6730: threshold = 4.0 * avgDev + 0.019999999552965164 + d3
                    let threshold = 4.0 * self.avg_dev + 0.019999999552965164 + d3;
                    let lag_jump = lag_result - self.last_lag;
                    if lag_jump > threshold {
                        // Spike entry (0x2064a67f4..0x2064a6864)
                        self.in_spike = 1;
                        self.spike_threshold = threshold;
                        self.peak_spike_lag = lag_result;
                        self.ring_buffer.fill(0.0);
                        self.ring_index = 0;
                        self.spike_count = 0;
                        self.ring_buffer[0] = lag_result;
                        self.spike_start_arrival_sec = packet.arrival_sec;
                        self.spike_start_timestamp_time_sec = ((self.wrap_count as f64
                            * 4294967296.0)
                            + packet.timestamp as f64)
                            / cr;
                    }
                }

                if self.in_spike == 0 {
                    self.record_owrd(lag_result);
                    self.update_averages(lag_result, packet, samples_per_frame, false);
                }
            }
            _ => {}
        }

        // 0x2064a6a44: str d8, [x19, #0x170]
        self.last_lag = lag_result;
        lag_result
    }
}
