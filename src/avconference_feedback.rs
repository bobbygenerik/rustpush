// Native evidence: docs/apple-parity/ios27/evidence/facetime-20261010/README.md
// in the parent app. AVConference 24A446: SerializeWithData 0x2064c1f48,
// UnserializeWithData 0x2064c2268, FECUtil_PackFrameLoss 0x205fc5af8.
// Packet-format version and FEC feedback subversion are independent. AVConference
// 24A446 negotiates the lower subversion, defaults to zero, and enables packed
// loss statistics only for subversions 1/2. See the parent app negotiation record.

use std::sync::{Mutex, atomic::{AtomicU64, Ordering}};
use std::time::Instant;
#[path = "avconference_history.rs"]
mod send_history;
pub(super) use send_history::LossSample;
use send_history::SendHistory;

struct HistoryClock { epoch: Instant, history: SendHistory }
impl Default for HistoryClock {
    fn default() -> Self { Self { epoch: Instant::now(), history: SendHistory::default() } }
}

// Keep advertisement and negotiation tied to the implemented capability. Mode 0
// is cumulative feedback and must be correlated with local send history.
pub(super) const LOCAL_FEC_FEEDBACK_VERSION: u8 = 0;

pub(super) fn negotiate_feedback_version(local: u8, remote: Option<u32>) -> u8 {
    local.min(remote.unwrap_or_default() as u8)
}

pub(super) fn loss_stats_enabled(version: u8) -> bool {
    matches!(version, 1 | 2)
}

#[derive(Default)]
pub(super) struct ParticipantFeedbackState {
    // Low byte is the mode, high bits distinguish changes even on 1 -> 0 -> 1.
    configuration: AtomicU64,
    video_received: AtomicU64,
    video_sent: AtomicU64,
    send_history: Mutex<HistoryClock>,
}

impl ParticipantFeedbackState {
    pub(super) fn configure(&self, local: u8, remote: Option<u32>) {
        let version = negotiate_feedback_version(local, remote);
        let changed = self.configuration.fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
            (old as u8 != version).then(|| (old.wrapping_add(256) & !255) | u64::from(version))
        });
        if changed.is_ok() { self.send_history.lock().unwrap().history.reset(); }
    }

    pub(super) fn configuration(&self) -> u64 {
        self.configuration.load(Ordering::Acquire)
    }

    pub(super) fn accepts_packed_report(&self, configuration: u64) -> bool {
        self.configuration() == configuration && loss_stats_enabled(configuration as u8)
    }

    pub(super) fn accepts_report(&self, configuration: u64) -> bool {
        self.configuration() == configuration
    }

    pub(super) fn record_video_sent(&self) { self.video_sent.fetch_add(1, Ordering::Relaxed); }

    pub(super) fn record_audio_sent(&self, sequence: u16) {
        let mut clock = self.send_history.lock().unwrap();
        let now = clock.epoch.elapsed();
        clock.history.record_audio(sequence, self.video_sent.load(Ordering::Relaxed), now);
    }

    pub(super) fn observe_cumulative(&self, sequence: u16, received: u16) -> Option<LossSample> {
        let mut clock = self.send_history.lock().unwrap();
        let now = clock.epoch.elapsed();
        clock.history.observe(sequence, received, now)
    }

    pub(super) fn record_video_packet(&self) {
        self.video_received.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn video_received(&self) -> u16 {
        (self.video_received.load(Ordering::Relaxed) & 0x0fff) as u16
    }
}

#[derive(Default)]
pub(super) struct CumulativeCounter12 {
    last: Option<u64>,
}

impl CumulativeCounter12 {
    pub(super) fn observe(&mut self, value: u16) -> Option<u64> {
        let value = u64::from(value & 0x0fff);
        let next = if let Some(previous) = self.last {
            let delta = value.wrapping_sub(previous & 0x0fff) & 0x0fff;
            // Native RTPProcessMediaControlInfo rejects the older half-cycle.
            if delta & 0x0800 != 0 { return None }
            previous + delta
        } else { value };
        self.last = Some(next);
        Some(next)
    }
}

pub(super) fn burst_loss_for_jump(jump: u16) -> u8 {
    jump.saturating_sub(1).min(15) as u8
}

pub(super) fn normalize_frame_loss(size: u16, loss: u16) -> (u8, u8) {
    let loss = loss.min(size);
    if size > 63 {
        (63, (63 * u32::from(loss) / u32::from(size)) as u8)
    } else {
        (size as u8, loss as u8)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct FTAudioControlData {
    pub(super) version: u8,
    // Native receiveQueueTarget / 20, not a bitrate estimate (v2 only).
    pub(super) receive_queue_target_bucket: u8,
    pub(super) feedback_sequence: u16,
    // Native totalReceivedKBytes (v2 only).
    pub(super) total_kbytes_recv: u16,
    pub(super) audio_burst_loss: u8,
    pub(super) audio_received_packets: u16,
    pub(super) queuing_delay: Option<u16>,
    // 1024/s clock.
    pub(super) current_send_timestamp: Option<u16>,
    pub(super) q13_one_way_delay: Option<u16>,
    pub(super) video_burst_loss: Option<u8>,
    pub(super) video_frame_size: Option<u8>,
    pub(super) video_packet_loss: Option<u8>,
    pub(super) video_received_packets: Option<u16>,
    pub(super) bandwidth_estimate: Option<u16>,
    pub(super) ect: Option<u16>,
    pub(super) ce: Option<u16>,
    // feedback/history index, secondary-only count, primary count, total count.
    pub(super) connection_stats_blob: Option<[u8; 4]>,
}

impl FTAudioControlData {
    pub(super) fn delay_ms(&self) -> Option<u32> {
        // RTPUnpackDouble 0x205fa9d4c neutralizes 0xffff. Keep unknown separate
        // from a measured zero so it cannot imply a healthy link.
        self.q13_one_way_delay.filter(|value| *value != u16::MAX)
            .map(|value| value as u32 * 125 / 1024)
    }

    // Explicit packed-mode wrapper retained for historical codec fixtures.
    #[cfg(test)]
    pub(super) fn from_ext(profile: u16, payload: &[u8]) -> Option<Self> {
        Self::from_ext_with_feedback_version(profile, payload, 1)
    }

    pub(super) fn from_ext_with_feedback_version(profile: u16, payload: &[u8], feedback_version: u8) -> Option<Self> {
        let version = (profile >> 14) as u8;
        let flags = match version {
            1 => profile as u8 & 0x0f,
            2 => (profile >> 8) as u8 & 0x0f,
            _ => return None,
        };
        // Apple's v2 decoder rejects a zero flag nibble and always reads the
        // six timing bytes. V1 also always has timing, regardless of bit zero.
        if version == 2 && flags == 0 { return None }
        let has_video = version == 2 && flags & 4 != 0;
        let has_ecn = version == 2 && flags & 8 != 0;
        let has_connection_stats = flags & 2 != 0;
        let expected = 12 + usize::from(has_video) * 4
            + usize::from(has_ecn) * 4 + usize::from(has_connection_stats) * 4;
        if payload.len() != expected { return None }
        // Length is checked before indexing. No allocation or untrusted drain.
        let word = |offset| u16::from_be_bytes([payload[offset], payload[offset + 1]]);
        let packed = word(4);
        let mut data = Self {
            version,
            feedback_sequence: word(0),
            audio_received_packets: packed & 0x0fff,
            queuing_delay: Some(word(6)),
            current_send_timestamp: Some(word(8)),
            q13_one_way_delay: Some(word(10)),
            ..Default::default()
        };
        if version == 2 {
            data.receive_queue_target_bucket = profile as u8;
            data.total_kbytes_recv = word(2);
            data.audio_burst_loss = (packed >> 12) as u8;
        } else {
            // V1's second word is bandwidth, and the packed high nibble is
            // video burst loss. It does not supply a six-bit size/loss pair.
            data.bandwidth_estimate = Some(word(2));
            data.video_burst_loss = Some((packed >> 12) as u8);
        }
        let mut offset = 12;
        if has_video {
            let packed = word(offset);
            data.video_burst_loss = Some((packed >> 12) as u8);
            if loss_stats_enabled(feedback_version) {
                data.video_frame_size = Some(((packed >> 6) & 0x3f) as u8);
                data.video_packet_loss = Some((packed & 0x3f) as u8);
            } else {
                data.video_received_packets = Some(packed & 0x0fff);
            }
            data.bandwidth_estimate = Some(word(offset + 2));
            offset += 4;
        }
        if has_ecn {
            data.ect = Some(word(offset));
            data.ce = Some(word(offset + 2));
            offset += 4;
        }
        if has_connection_stats {
            data.connection_stats_blob = Some(payload[offset..offset + 4].try_into().ok()?);
        }
        Some(data)
    }

    pub(super) fn packed_video_loss(&self, feedback_version: u8) -> Option<(u8, u8)> {
        if !loss_stats_enabled(feedback_version) || self.version != 2
            || self.video_received_packets.is_some() { return None }
        Some((self.video_frame_size?, self.video_packet_loss?))
    }

    #[cfg(test)]
    pub(super) fn to_ext(&self) -> Option<(u16, Vec<u8>)> {
        self.to_ext_with_feedback_version(1)
    }

    pub(super) fn to_ext_with_feedback_version(&self, feedback_version: u8) -> Option<(u16, Vec<u8>)> {
        if !matches!(self.version, 1 | 2) { return None }
        let packed_loss = loss_stats_enabled(feedback_version);
        if (packed_loss && self.video_received_packets.is_some())
            || (!packed_loss && (self.video_frame_size.is_some() || self.video_packet_loss.is_some())) {
            return None
        }
        let timing = [self.queuing_delay?, self.current_send_timestamp?, self.q13_one_way_delay?];
        let mut flags = 1u16;
        let mut body = Vec::with_capacity(24);
        body.extend_from_slice(&self.feedback_sequence.to_be_bytes());
        let (second, burst) = if self.version == 2 {
            (self.total_kbytes_recv, self.audio_burst_loss)
        } else {
            if self.ect.is_some() || self.ce.is_some() || self.video_frame_size.is_some()
                || self.video_packet_loss.is_some() || self.video_received_packets.is_some() { return None }
            if self.video_burst_loss.is_some() { flags |= 4; }
            (self.bandwidth_estimate?, self.video_burst_loss.unwrap_or_default())
        };
        body.extend_from_slice(&second.to_be_bytes());
        let packed = ((burst.min(15) as u16) << 12) | (self.audio_received_packets & 0x0fff);
        body.extend_from_slice(&packed.to_be_bytes());
        for value in timing { body.extend_from_slice(&value.to_be_bytes()); }
        if self.version == 2 {
            if self.video_burst_loss.is_some() || self.video_frame_size.is_some()
                || self.video_packet_loss.is_some() || self.video_received_packets.is_some()
                || self.bandwidth_estimate.is_some() {
                let burst = self.video_burst_loss?.min(15) as u16;
                let low = if packed_loss {
                    let size = self.video_frame_size? as u16;
                    let loss = self.video_packet_loss? as u16;
                    let (size, loss) = if size > 63 { (63, 63 * loss / size) } else { (size, loss) };
                    (size << 6) | (loss & 63)
                } else { self.video_received_packets? & 0x0fff };
                let packed = (burst << 12) | low;
                flags |= 4;
                body.extend_from_slice(&packed.to_be_bytes());
                body.extend_from_slice(&self.bandwidth_estimate?.to_be_bytes());
            }
            if self.ect.is_some() || self.ce.is_some() {
                flags |= 8;
                body.extend_from_slice(&self.ect?.to_be_bytes());
                body.extend_from_slice(&self.ce?.to_be_bytes());
            }
        }
        if let Some(stats) = self.connection_stats_blob {
            flags |= 2;
            body.extend_from_slice(&stats);
        }
        let profile = (self.version as u16) << 14 | if self.version == 2 {
            (flags << 8) | self.receive_queue_target_bucket as u16
        } else { flags };
        Some((profile, body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> FTAudioControlData {
        FTAudioControlData {
            version: 2, feedback_sequence: 1, total_kbytes_recv: 2,
            audio_burst_loss: 3, audio_received_packets: 4,
            queuing_delay: Some(5), current_send_timestamp: Some(6),
            q13_one_way_delay: Some(7), ..Default::default()
        }
    }

    #[test]
    fn native_v2_base_vector() {
        let body = [0, 1, 0, 2, 0x30, 4, 0, 5, 0, 6, 0, 7];
        assert_eq!(data().to_ext(), Some((0x8100, body.to_vec())));
        assert_eq!(FTAudioControlData::from_ext(0x8100, &body), Some(data()));
    }

    #[test]
    fn existing_v2_fixture_remains_byte_identical() {
        let mut body = [0;20];
        body[8] = 0xf4;
        body[9] = 0x40;
        let decoded = FTAudioControlData::from_ext(0x8d00, &body).unwrap();
        assert_eq!(decoded.to_ext(), Some((0x8d00, body.to_vec())));
    }

    #[test]
    fn native_v1_has_bandwidth_and_burst_without_frame_loss() {
        let body = [0, 1, 0, 2, 0x30, 4, 0, 5, 0, 6, 0, 7];
        let decoded = FTAudioControlData::from_ext(0x4001, &body).unwrap();
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.bandwidth_estimate, Some(2));
        assert_eq!(decoded.total_kbytes_recv, 0);
        assert_eq!(decoded.video_burst_loss, Some(3));
        assert_eq!(decoded.audio_burst_loss, 0);
        assert_eq!(decoded.audio_received_packets, 4);
        assert_eq!(decoded.queuing_delay, Some(5));
        assert_eq!(decoded.current_send_timestamp, Some(6));
        assert_eq!(decoded.q13_one_way_delay, Some(7));
        assert_eq!(decoded.video_frame_size, None);
        assert_eq!(decoded.video_packet_loss, None);
        // Native v1 video-presence flag does not add four body bytes.
        assert_eq!(decoded.to_ext(), Some((0x4005, body.to_vec())));
        assert_eq!(FTAudioControlData::from_ext(0x4005, &body), Some(decoded));
    }

    #[test]
    fn native_optional_field_order_and_connection_bytes() {
        let body = [0,1, 0,2, 0x30,4, 0,5, 0,6, 0,7,
                    0x82,0x89, 0,10, 0,11, 0,12, 13,14,15,16];
        let decoded = FTAudioControlData::from_ext(0x8f23, &body).unwrap();
        assert_eq!(decoded.receive_queue_target_bucket, 0x23);
        assert_eq!(decoded.video_burst_loss, Some(8));
        assert_eq!(decoded.video_frame_size, Some(10));
        assert_eq!(decoded.video_packet_loss, Some(9));
        assert_eq!(decoded.bandwidth_estimate, Some(10));
        assert_eq!(decoded.ect, Some(11));
        assert_eq!(decoded.ce, Some(12));
        assert_eq!(decoded.connection_stats_blob, Some([13,14,15,16]));
        assert_eq!(decoded.to_ext(), Some((0x8f23, body.to_vec())));
    }

    #[test]
    fn v1_connection_stats_follow_timing() {
        let body = [0,1, 0,2, 0x30,4, 0,5, 0,6, 0,7, 8,9,10,11];
        let decoded = FTAudioControlData::from_ext(0x4003, &body).unwrap();
        assert_eq!(decoded.connection_stats_blob, Some([8,9,10,11]));
        assert_eq!(decoded.to_ext(), Some((0x4007, body.to_vec())));
    }

    #[test]
    fn native_large_frame_vectors_preserve_loss_fraction() {
        for (size, loss, expected) in [(63,10,0x0fca), (64,1,0x0fc0),
            (100,20,0x0fcc), (128,32,0x0fcf), (200,42,0x0fcd), (255,255,0x0fff)] {
            let sample = FTAudioControlData { video_burst_loss: Some(0),
                video_frame_size: Some(size), video_packet_loss: Some(loss),
                bandwidth_estimate: Some(0), ..data() };
            let (profile, body) = sample.to_ext().unwrap();
            assert_eq!(&body[12..14], &u16::to_be_bytes(expected));
            let decoded = FTAudioControlData::from_ext(profile, &body).unwrap();
            assert_eq!(decoded.video_frame_size, Some(size.min(63)));
        }
    }

    #[test]
    fn burst_counts_saturate_instead_of_wrapping() {
        let sample = FTAudioControlData { audio_burst_loss: 16,
            video_burst_loss: Some(255), video_frame_size: Some(1),
            video_packet_loss: Some(0), bandwidth_estimate: Some(0), ..data() };
        let (_, body) = sample.to_ext().unwrap();
        assert_eq!(&body[4..6], &[0xf0,4]);
        assert_eq!(&body[12..14], &[0xf0,0x40]);
    }

    #[test]
    fn unknown_delay_is_preserved_on_wire_but_is_not_a_measurement() {
        let sample = FTAudioControlData { q13_one_way_delay: Some(u16::MAX), ..data() };
        assert_eq!(sample.delay_ms(), None);
        let (profile, body) = sample.to_ext().unwrap();
        assert_eq!(&body[10..12], &[0xff,0xff]);
        assert_eq!(FTAudioControlData::from_ext(profile, &body).unwrap().delay_ms(), None);
        assert_eq!(FTAudioControlData { q13_one_way_delay: Some(8192), ..data() }.delay_ms(), Some(1000));
        assert_eq!(FTAudioControlData { q13_one_way_delay: Some(0), ..data() }.delay_ms(), Some(0));
    }

    #[test]
    fn every_v2_layout_rejects_truncation_and_extra_bytes() {
        for flags in 1u16..16 {
            let expected = 12 + if flags & 4 != 0 {4} else {0}
                + if flags & 8 != 0 {4} else {0} + if flags & 2 != 0 {4} else {0};
            for length in 0..=28 {
                assert_eq!(FTAudioControlData::from_ext(0x8000 | flags << 8, &vec![0;length]).is_some(),
                    length == expected, "flags={flags} length={length}");
            }
        }
    }

    #[test]
    fn every_v1_layout_requires_exact_timing_and_connection_length() {
        for flags in 0u16..16 {
            let expected = if flags & 2 != 0 {16} else {12};
            for length in 0..=24 {
                assert_eq!(FTAudioControlData::from_ext(0x4000 | flags, &vec![0;length]).is_some(),
                    length == expected, "flags={flags} length={length}");
            }
        }
    }

    #[test]
    fn unsupported_versions_and_empty_v2_flags_are_rejected() {
        for profile in [0, 0x0100, 0xc100, 0x8000, 0x80ff] {
            assert_eq!(FTAudioControlData::from_ext(profile, &[0;12]), None);
        }
    }

    #[test]
    fn partial_outgoing_groups_are_rejected() {
        for sample in [FTAudioControlData { current_send_timestamp: None, ..data() },
            FTAudioControlData { video_burst_loss: Some(1), ..data() },
            FTAudioControlData { ect: Some(1), ..data() },
            FTAudioControlData { version: 0, ..data() }] {
            assert_eq!(sample.to_ext(), None);
        }
    }
}
