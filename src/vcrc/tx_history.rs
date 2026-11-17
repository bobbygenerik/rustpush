//! The sender-side packet-loss calculation behind
//! `VCRateControlMediaController_ComputePacketLoss` 0x20657519c on the VCMediaQueue back end
//! that VCSession uses for 1:1 calls (`VCMediaQueue_ComputeOneToOneStatsWithPacketInfo`
//! 0x206372560 → `VCMediaQueueUtil_FindPacketInTxHistory` 0x205f5ab54), and the send-side
//! bookkeeping that feeds it (`__VCMediaQueue_SendAndFreePackets` 0x20636d870..8e4 and
//! `__VCMediaQueue_AddInfoToOneToOneTxHistory` 0x206372c30). See port-notes.md.

use super::media::{PacketLossInput, PacketLossOutput};

/// One 48-byte tx-history record (queue+0x3d0 + i*0x30, 512 records).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TxRecord {
    /// +0x00 u16: receiver's burst loss for this record, capped at 15 (set when acknowledged).
    pub burst_bucket: u16,
    /// +0x02 u16: tx timestamp (1/1024 s, the value carried in the packet's media control info).
    pub tx_timestamp: u16,
    /// +0x04 u32: receiver's jitter-queue size (set when acknowledged; not cleared on reuse).
    pub jitter: u32,
    /// +0x08 u32: cumulative sent audio packets when this record was added.
    pub sent_audio: u32,
    /// +0x0c u32: cumulative sent video packets when this record was added.
    pub sent_video: u32,
    /// +0x10 u32: receiver's audio received-packet count (0 until acknowledged).
    pub recv_audio: u32,
    /// +0x14 u32: receiver's video word (packed frame loss on the loss-stats layout).
    pub recv_video: u32,
    /// +0x18 u64: cumulative sent bytes when added.
    pub sent_bytes: u64,
    /// +0x20 u64: receiver's total received bytes (KB × 1000), unwrapped.
    pub recv_bytes: u64,
    /// +0x28 f64: send time (`micro()`, seconds).
    pub time: f64,
}

/// The VCMediaQueue state these functions use.
#[derive(Clone, Debug)]
pub struct TxHistory {
    /// queue+0x3d0: ring of 512 records.
    pub records: Box<[TxRecord; 512]>,
    /// queue+0x3c8: next write index.
    pub write_index: u32,
    /// queue+0x3cc: records in use (max 512).
    pub count: u32,
    /// queue+0x6410: cumulative sent audio packets.
    pub sent_audio_packets: u32,
    /// queue+0x6414: cumulative sent video packets.
    pub sent_video_packets: u32,
    /// queue+0x6418: cumulative sent bytes (u32, wraps).
    pub sent_bytes: u32,
}

impl Default for TxHistory {
    fn default() -> Self {
        Self { records: Box::new([TxRecord::default(); 512]), write_index: 0, count: 0,
            sent_audio_packets: 0, sent_video_packets: 0, sent_bytes: 0 }
    }
}

/// `_FECUtil_UnpackFrameLoss` 0x205fc5d8c: returns (high field, low field) for a 6- or 8-bit
/// layout; any other layout only logs and leaves the outputs unset (None).
pub fn fec_unpack_frame_loss(word: u32, bits: u32) -> Option<(u32, u32)> {
    let mask = match bits { 6 => 0x3f, 8 => 0xff, _ => return None };
    Some(((word >> bits) & mask, word & mask))
}

impl TxHistory {
    /// Counters updated per media packet in `__VCMediaQueue_SendAndFreePackets`
    /// 0x20636d870..8e4. `media_type` is packet+0xb0 (1 video, 0 audio); `flag_e2`/`flag_e3`
    /// are packet bytes +0xe2/+0xe3 (meaning not yet traced); `packet_count` packet+0x10c;
    /// `payload_len` packet+0x4; `ipv6` packet+0x125 (header 0x30 instead of 0x1c bytes).
    pub fn track_sent_packet(&mut self, media_type: u32, flag_e2: bool, flag_e3: bool,
        packet_count: u32, payload_len: u16, ipv6: bool) {
        if media_type == 1 {
            if !flag_e2 { self.sent_video_packets = self.sent_video_packets.wrapping_add(packet_count); }
        } else if media_type == 0 && flag_e3 {
            self.sent_audio_packets = self.sent_audio_packets.wrapping_add(packet_count);
        }
        let hdr = if ipv6 { 0x30 } else { 0x1c };
        self.sent_bytes = self.sent_bytes
            .wrapping_add((hdr + payload_len as u32).wrapping_mul(packet_count));
    }

    /// `__VCMediaQueue_AddInfoToOneToOneTxHistory` 0x206372c30. On a 1:1 queue this runs for
    /// each sent packet that carries media control info, with `ts` =
    /// `(NTPToMiddle32(MicroToNTP(now)) >> 6) & 0xffff` and `time` = now (0x20636d9b4..9ec).
    pub fn add_info(&mut self, ts: u16, time: f64) {
        let r = &mut self.records[self.write_index as usize];
        r.tx_timestamp = ts;
        r.time = time;
        r.sent_audio = self.sent_audio_packets;
        r.sent_video = self.sent_video_packets;
        r.burst_bucket = 0;
        r.recv_audio = 0;
        r.recv_video = 0;
        r.sent_bytes = self.sent_bytes as u64;
        r.recv_bytes = 0;
        self.write_index = (self.write_index + 1) & 0x1ff;
        if self.count <= 0x1ff { self.count += 1; }
    }

    fn rec(&self, i: i32) -> &TxRecord { &self.records[(i & 0x1ff) as usize] }

    /// `VCRateControlMediaController_ComputePacketLoss` 0x20657519c (VCMediaQueue path): builds
    /// the packet-info struct (0x206575224..2a4) and runs
    /// `VCMediaQueue_ComputeOneToOneStatsWithPacketInfo` 0x206372560, which calls
    /// FindPacketInTxHistory with windows 1.0 s and 5.0 s and frame-loss mode on.
    /// `frame_based_redundancy` is `VCMediaQueue_GetIsFrameBasedRedundancyEnabled` 0x20637281c.
    pub fn compute_packet_loss(&mut self, input: &PacketLossInput, frame_based_redundancy: bool)
        -> PacketLossOutput {
        let bits = if frame_based_redundancy && input.traffic_type & 3 == 3 { 8 } else { 6 };
        let info = PacketInfo {
            echoed: input.echoed_timestamp,
            recv_audio: input.audio_received_packets,
            recv_video: input.video_received_packets,
            burst: input.packet_burst_loss as u32,
            recv_bytes: (input.total_received_kbytes as u64).wrapping_mul(1000),
            jitter: input.receive_jitter_queue_size,
            frame_bits: bits,
        };
        self.find_packet_in_tx_history(&info, 1.0, 5.0, true)
    }

    /// `_VCMediaQueueUtil_FindPacketInTxHistory` 0x205f5ab54.
    pub fn find_packet_in_tx_history(&mut self, p: &PacketInfo, short_window: f64,
        long_window: f64, frame_loss: bool) -> PacketLossOutput {
        let mut out = PacketLossOutput::default();
        let mut buckets = [0u32; 16];
        let count = self.count as i32;
        let start = self.write_index as i32 - 1; // w2
        // 0x205f5abe8..c58: newest-first search for the echoed timestamp.
        let mut found: Option<(i32, i32)> = None; // (position i, index)
        let mut idx = start;
        for i in 0..count.max(0) {
            let r = self.rec(idx);
            let d = (r.tx_timestamp as u32).wrapping_sub(p.echoed as u32);
            let st = if r.tx_timestamp == p.echoed { 1 } else if 0x7fff > (d & 0xffff) { 4 } else { 2 };
            if st == 2 { break; }
            if st == 1 { found = Some((i, idx & 0x1ff)); break; }
            idx -= 1;
        }
        let (mut w14, mut w15) = (0u32, 0u32); // frame-loss totals (high, low fields)
        let (mut ref_short, mut ref_long, mut _ref_1s, mut a) = (-1i32, -1i32, -1i32, -1i32);
        if let Some((pos, fidx)) = found {
            a = fidx;
            let t_now;
            {
                let r = &mut self.records[fidx as usize];
                t_now = r.time;
                r.burst_bucket = p.burst.min(15) as u16;
                r.recv_audio = p.recv_audio;
                r.recv_video = p.recv_video;
                r.recv_bytes = p.recv_bytes;
                r.jitter = p.jitter;
            }
            let w7 = start - 1; // write_index - 2
            let (mut rate_done, mut s_done, mut l_done, mut fl_done, mut five_done) =
                (false, false, false, false, false);
            let mut w8 = pos;
            let mut last_pos: i32 = -1; // w9
            let mut x27;
            'outer: loop {
                x27 = w8 + 1;
                if x27 >= count { break 'outer; }
                let mut w11 = w7 - w8;
                // skip records the receiver has not acknowledged (+0x10 == 0)
                loop {
                    if self.rec(w11).recv_audio != 0 { break; }
                    x27 += 1;
                    w11 -= 1;
                    if x27 >= count { break 'outer; }
                }
                let j = w11 & 0x1ff;
                let r = *self.rec(j);
                if frame_loss && !fl_done && r.recv_video != 0 {
                    if let Some((hi, lo)) = fec_unpack_frame_loss(r.recv_video & 0xffff, p.frame_bits) {
                        w14 = w14.wrapping_add(hi);
                        w15 = w15.wrapping_add(lo);
                    } else {
                        // outputs stay at the 0xaaaaaaaa fill (0x205f5b09c)
                        w14 = w14.wrapping_add(0xaaaaaaaa);
                        w15 = w15.wrapping_add(0xaaaaaaaa);
                    }
                }
                buckets[r.burst_bucket as usize & 0xf] += 1;
                let dt = t_now - r.time;
                if !(rate_done || dt <= 1.0) {
                    let bits_per_s = ((p.recv_bytes.wrapping_sub(r.recv_bytes)) as f64 / dt * 8.0) as u32;
                    out.received_bitrate = Some(bits_per_s);
                    rate_done = true;
                }
                if !s_done && !(dt <= short_window) { ref_short = j; s_done = true; }
                if !l_done && !(dt <= long_window) { ref_long = j; l_done = true; }
                if !(fl_done || dt < 1.0) { _ref_1s = j; fl_done = true; }
                if !(five_done || dt <= 5.0) { five_done = true; }
                last_pos = x27;
                w8 = x27;
                if s_done && l_done && fl_done && five_done && rate_done {
                    break 'outer;
                }
            }
            let all = s_done && l_done && fl_done && five_done && rate_done;
            // 0x205f5b1ec..1f0: position reached when the walk stopped.
            let w27 = if all { x27 } else if count > w8 + 1 { count } else { w8 + 1 };
            if last_pos != -1 {
                let k = (start - last_pos) & 0x1ff;
                if !all && count >= 0x80 && w27 == count {
                    if !rate_done {
                        let r = *self.rec(k);
                        let dt = t_now - r.time;
                        if dt > 0.0 {
                            out.received_bitrate =
                                Some(((p.recv_bytes.wrapping_sub(r.recv_bytes)) as f64 / dt * 8.0) as u32);
                        }
                    }
                    if !s_done { ref_short = k; }
                    if !l_done { ref_long = k; }
                    if !fl_done { _ref_1s = k; }
                }
                // 0x205f5b280..2f4: unwrap the receiver's 16-bit KB counter against record k.
                let r_kb = self.rec(k).recv_bytes / 1000;
                let p_kb = p.recv_bytes / 1000;
                if ((p_kb & 0xffff) as u16) < (r_kb & 0xffff) as u16 {
                    let diff = (p_kb as u32).wrapping_sub(r_kb as u32);
                    if (diff & 0xffff) <= 0x7ffe {
                        let v = ((r_kb & 0x7f_ffff_ffff_0000) | (p_kb & 0xffff))
                            .wrapping_mul(1000).wrapping_add(0x3e80000);
                        self.records[a as usize].recv_bytes = v;
                    }
                }
            }
        }
        self.tail(&mut out, &buckets, a, ref_short, ref_long, frame_loss, w14, w15);
        out
    }

    /// 0x205f5ac74..afc8: burst-loss percentile, the two PLR windows and video frame loss.
    #[allow(clippy::too_many_arguments)]
    fn tail(&self, out: &mut PacketLossOutput, buckets: &[u32; 16], a: i32, ref_short: i32,
        ref_long: i32, frame_loss: bool, frames: u32, lost: u32) {
        // Burst length below which more than 90% of acknowledged records fall
        // ([0x2066a3918] = 0.9); 16 if never reached, 0 with no records.
        let sum: u32 = buckets[1..16].iter().fold(0u32, |s, v| s.wrapping_add(*v));
        out.most_burst_loss = Some(if sum == 0 {
            0
        } else {
            let mut acc = 0u32;
            let mut k = 16;
            for i in 1..16 {
                acc = acc.wrapping_add(buckets[i]);
                if acc as f64 / sum as f64 > 0.9 { k = i as u32; break; }
            }
            k
        });
        let plr = |b: i32| -> Option<f64> {
            if a < 0 || b < 0 { return None; }
            let (ra, rb) = (self.rec(a), self.rec(b));
            if ra.recv_audio == 0 || rb.recv_audio == 0 || (a & 0x1ff) == (b & 0x1ff) { return None; }
            let sa = ra.sent_audio.wrapping_sub(ra.jitter) as f64;
            let sb = rb.sent_audio.wrapping_sub(rb.jitter) as f64;
            let sent = sa - sb;
            if !(sent > 0.0) { return None; }
            let recv = ra.recv_audio as f64 - rb.recv_audio as f64;
            if recv < 0.0 { return None; }
            let v = 1.0 - recv / sent;
            Some(if v < 0.0 { 0.0 } else { v })
        };
        out.packet_loss_rate_audio = plr(ref_short); // info+0x30 ← 1 s reference
        out.packet_loss_rate = plr(ref_long); //        info+0x38 ← 5 s reference
        if frame_loss {
            out.packet_loss_rate_video = Some(if frames != 0 { lost as f64 / frames as f64 } else { 0.0 });
        }
        // The 3-candidate video path (0x205f5aed8..afc4) only runs with frame_loss == false,
        // which ComputeOneToOneStatsWithPacketInfo never passes (w3 = 1 at 0x2063725bc).
    }
}

/// The packet-info struct ComputePacketLoss builds for FindPacketInTxHistory.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PacketInfo {
    /// +0x00 echoed tx timestamp.
    pub echoed: u16,
    /// +0x04 receiver's audio received-packet count.
    pub recv_audio: u32,
    /// +0x08 receiver's video word.
    pub recv_video: u32,
    /// +0x0c burst loss.
    pub burst: u32,
    /// +0x10 total received bytes (KB × 1000).
    pub recv_bytes: u64,
    /// +0x18 jitter-queue size.
    pub jitter: u32,
    /// +0x48 frame-loss layout: 6, or 8 with frame-based redundancy and trafficType&3 == 3.
    pub frame_bits: u32,
}
