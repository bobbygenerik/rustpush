use rtc_rtcp::{header::{Header, PacketType, HEADER_LENGTH}, packet::Packet, raw_packet::RawPacket};
use rtc_shared::{error::{Error, Result}, marshal::Unmarshal};

/// Receive-side diagnostics only. The timestamp is an RTP reference, not a
/// decoder receipt or permission to acknowledge an encoder reference slot.
/// iOS27 __VideoReceiver_ProcessVideoRTCPInternal / ProcessRtcpLTRAck.
/// Type6 frameSize is a packet count (fecFeedback.sizeInPackets), not bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ApplicationFeedback {
    LtrAck { timestamp: u32 },
    Loss { timestamp: u32, received_packets: u16, frame_packet_count: u8, lost_packets: u8 },
    Unknown { identifier: Option<u32> },
}

pub(super) fn application_feedback(packet: &[u8]) -> Result<ApplicationFeedback> {
    let header = Header::unmarshal(&mut &packet[..])?;
    let length = (usize::from(header.length) + 1) * HEADER_LENGTH;
    if header.packet_type != PacketType::PayloadSpecificFeedback || header.count != 15 {
        return Err(Error::InvalidHeader);
    }
    let packet = packet.get(..length).ok_or(Error::PacketTooShort)?;
    // RTCP padding is not application data. It cannot satisfy the known FCI
    // layouts or borrow bytes from the next packet in a compound datagram.
    let padding = if header.padding {
        let count = usize::from(*packet.last().ok_or(Error::PacketTooShort)?);
        if count == 0 || count > packet.len().saturating_sub(12) {
            return Err(Error::PacketTooShort);
        }
        count
    } else { 0 };
    let body = packet.get(..packet.len() - padding).filter(|body| body.len() >= 12)
        .ok_or(Error::PacketTooShort)?;
    let identifier = body.get(12..16).map(|bytes| u32::from_be_bytes(bytes.try_into().unwrap()));
    match identifier {
        Some(kind @ (5 | 6)) => {
            let payload = body.get(16..if kind == 5 { 20 } else { 24 }).ok_or(Error::PacketTooShort)?;
            let timestamp = u32::from_be_bytes(payload[..4].try_into().unwrap());
            Ok(if kind == 5 { ApplicationFeedback::LtrAck { timestamp } } else {
                ApplicationFeedback::Loss { timestamp,
                    received_packets: u16::from_be_bytes(payload[4..6].try_into().unwrap()),
                    frame_packet_count: payload[6], lost_packets: payload[7] }
            })
        }
        identifier => Ok(ApplicationFeedback::Unknown { identifier }),
    }
}

/// Wire serializer for iOS 27 PSFB AFB (PT=206, FMT=15) Type-5 LTR ACK and
/// Type-6 FEC loss feedback (`__RTCPAddPSFBAlfbPacket` 0x205f6a9ec /
/// `__VideoReceiver_SendRTCPResiliencyInfo` 0x205ffe5cc).
#[allow(dead_code)]
pub(super) fn encode_application_feedback(
    sender_ssrc: u32,
    media_ssrc: u32,
    feedback: ApplicationFeedback,
) -> Result<Vec<u8>> {
    match feedback {
        ApplicationFeedback::LtrAck { timestamp } => {
            let mut buf = Vec::with_capacity(20);
            buf.extend_from_slice(&[0x8f, 206, 0x00, 0x04]);
            buf.extend_from_slice(&sender_ssrc.to_be_bytes());
            buf.extend_from_slice(&media_ssrc.to_be_bytes());
            buf.extend_from_slice(&5u32.to_be_bytes());
            buf.extend_from_slice(&timestamp.to_be_bytes());
            Ok(buf)
        }
        ApplicationFeedback::Loss {
            timestamp,
            received_packets,
            frame_packet_count,
            lost_packets,
        } => {
            let mut buf = Vec::with_capacity(24);
            buf.extend_from_slice(&[0x8f, 206, 0x00, 0x05]);
            buf.extend_from_slice(&sender_ssrc.to_be_bytes());
            buf.extend_from_slice(&media_ssrc.to_be_bytes());
            buf.extend_from_slice(&6u32.to_be_bytes());
            buf.extend_from_slice(&timestamp.to_be_bytes());
            buf.extend_from_slice(&received_packets.to_be_bytes());
            buf.push(frame_packet_count);
            buf.push(lost_packets);
            Ok(buf)
        }
        ApplicationFeedback::Unknown { .. } => Err(Error::InvalidHeader),
    }
}

/// Wire serializer for iOS 27 legacy RTCP LTR ACK (PT=192, FMT=0)
/// (`_RTPSendRTCP` 0x205f650a4). Serializes `sender_ssrc` only
/// (no remote SSRC field) followed by 1..=10 big-endian u16 tokens (native
/// writer buffer bounds check at 0x205f653e0) with `0xffff` odd-count tail
/// padding.
#[allow(dead_code)]
pub(super) fn encode_legacy_ltr_ack(sender_ssrc: u32, tokens: &[u16]) -> Result<Vec<u8>> {
    if tokens.is_empty() || tokens.len() > 10 || tokens.contains(&0xffff) {
        return Err(Error::InvalidHeader);
    }
    let dwords = 1 + tokens.len().div_ceil(2);
    let mut buf = Vec::with_capacity((dwords + 1) * HEADER_LENGTH);
    buf.push(0x80);
    buf.push(192);
    buf.extend_from_slice(&(dwords as u16).to_be_bytes());
    buf.extend_from_slice(&sender_ssrc.to_be_bytes());
    for &token in tokens {
        buf.extend_from_slice(&token.to_be_bytes());
    }
    if tokens.len() % 2 == 1 {
        buf.extend_from_slice(&0xffffu16.to_be_bytes());
    }
    Ok(buf)
}

/// Wire parser for iOS 27 legacy RTCP LTR ACK (PT=192, FMT=0).
///
/// Not matched to a native function: 0x205ff1854 is `__VideoReceiver_SendRTCP`,
/// and the real `__VideoReceiver_ProcessRtcpLTRAck` at 0x206008aa0 receives one
/// already-selected 32-bit value rather than parsing PT=192 u16 tokens.
/// Returns `(sender_ssrc, tokens)`.
#[allow(dead_code)]
pub(super) fn parse_legacy_ltr_ack(packet: &[u8]) -> Result<(u32, Vec<u16>)> {
    if packet.len() < 12 || packet[0] != 0x80 || packet[1] != 192 {
        return Err(Error::InvalidHeader);
    }
    let length_words = usize::from(u16::from_be_bytes([packet[2], packet[3]]));
    if length_words < 2 || length_words > 9 {
        return Err(Error::InvalidHeader);
    }
    let total_bytes = (length_words + 1) * HEADER_LENGTH;
    let body = packet.get(..total_bytes).ok_or(Error::PacketTooShort)?;
    let sender_ssrc = u32::from_be_bytes(body[4..8].try_into().unwrap());
    let mut tokens = Vec::with_capacity((length_words - 1) * 2);
    for chunk in body[8..].chunks_exact(2) {
        let token = u16::from_be_bytes([chunk[0], chunk[1]]);
        if token == 0xffff {
            break;
        }
        tokens.push(token);
    }
    if tokens.is_empty() {
        return Err(Error::InvalidHeader);
    }
    Ok((sender_ssrc, tokens))
}

/// FaceTime accepts application feedback other than REMB. iOS27's
/// __RTCPAddPSFBAlfbPacket writes numeric FCI identifiers5/6 with PT206/FMT15;
/// that shared format alone does not identify a REMB payload.
/// Keep the vendored WebRTC parser and recognized-packet validation unchanged.
pub(super) fn parse(mut data: &[u8]) -> Result<Vec<Box<dyn Packet>>> {
    if data.is_empty() { return Err(Error::InvalidHeader); }
    let mut packets: Vec<Box<dyn Packet>> = Vec::new();
    while !data.is_empty() {
        let header = Header::unmarshal(&mut &data[..])?;
        let length = (usize::from(header.length) + 1) * HEADER_LENGTH;
        let packet = data.get(..length).ok_or(Error::PacketTooShort)?;
        let application_feedback = header.packet_type == PacketType::PayloadSpecificFeedback
            && header.count == 15;
        if application_feedback && packet.len() < 12 { return Err(Error::PacketTooShort); }
        if application_feedback && packet.get(12..16) != Some(b"REMB".as_slice()) {
            // Opaque means unsupported, not malformed. Do not zero its media
            // SSRC, interpret it as bitrate feedback, or discard adjacent NACKs.
            self::application_feedback(packet)?;
            packets.push(Box::new(RawPacket::unmarshal(&mut &packet[..])?));
        } else {
            packets.extend(rtc_rtcp::packet::unmarshal(&mut &packet[..])?);
        }
        data = &data[length..];
    }
    Ok(packets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_legacy_ltr_ack_enforces_ten_token_native_limit() {
        let max = encode_legacy_ltr_ack(0x1122_3344, &[0x1234; 10]).unwrap();
        assert_eq!(parse_legacy_ltr_ack(&max).unwrap(), (0x1122_3344, vec![0x1234; 10]));

        let odd = encode_legacy_ltr_ack(0x1122_3344, &[0x0042, 0x0043, 0x0044]).unwrap();
        assert_eq!(parse_legacy_ltr_ack(&odd).unwrap(), (0x1122_3344, vec![0x0042, 0x0043, 0x0044]));

        assert!(encode_legacy_ltr_ack(0x1122_3344, &[]).is_err());
        assert!(encode_legacy_ltr_ack(0x1122_3344, &[0x1234; 11]).is_err());
        assert!(encode_legacy_ltr_ack(0x1122_3344, &[0xffff]).is_err());
    }
}
