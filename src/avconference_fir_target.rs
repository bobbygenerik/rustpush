/// Retains explicit negotiated logical IDs without promoting SSRC fallbacks.
pub(super) fn negotiated_stream_ids(
    streams: impl IntoIterator<Item = (u32, Option<u32>)>,
) -> std::collections::HashMap<u32, u32> {
    // Absence must reach received_target as None. An SSRC-derived fallback
    // is not an explicitly negotiated IDS stream ID; zero can be explicit.
    streams.into_iter().filter_map(|(ssrc, id)| id.map(|id| (ssrc, id))).collect()
}

/// Receiver-owned FIR target. Native group IDs are not RTP SSRCs.
/// iOS27 AVConference streamGroupIDFromStreamGroup: maps camera1 to 'came'
/// and screen3 to 'scre'; ordinary stream callbacks send group0.
pub(super) fn received_target(
    grouped: bool,
    group_kind: u32,
    received_stream_id: u32,
    negotiated_stream_id: Option<u32>,
) -> Option<(u32, u32)> {
    if !grouped { return Some((received_stream_id & 0xffff, 0)); }
    let group_id = match group_kind {
        1 => 0x6361_6d65,
        3 => 0x7363_7265,
        _ => return None,
    };
    // Native serializes stream ID as unsigned short, group ID as unsigned int.
    Some((negotiated_stream_id? & 0xffff, group_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_preserves_explicit_zero_and_distinct_logical_ids() {
        let ids = negotiated_stream_ids([
            (0x1234_1111, Some(0)),
            (0x1234_2222, Some(0x4567)),
        ]);
        assert_eq!(ids.get(&0x1234_1111), Some(&0));
        assert_eq!(ids.get(&0x1234_2222), Some(&0x4567));
    }

    #[test]
    fn absent_negotiated_id_reaches_unknown_target_fallback() {
        let ssrc = 0x1234_1111;
        let ids = negotiated_stream_ids([(ssrc, None)]);
        assert!(ids.is_empty());
        assert_eq!(received_target(true, 1, ssrc & 0xffff, ids.get(&ssrc).copied()), None);
    }

    #[test]
    fn ordinary_callback_forwards_received_id_with_zero_group() {
        assert_eq!(received_target(false, 1, 0x1234, None), Some((0x1234, 0)));
    }

    #[test]
    fn camera_uses_negotiated_stream_id_and_native_group_namespace() {
        assert_eq!(received_target(true, 1, 0x1111, Some(0x4567)), Some((0x4567, 0x6361_6d65)));
    }

    #[test]
    fn valid_zero_stream_is_not_treated_as_missing() {
        assert_eq!(received_target(true, 1, 0x1111, Some(0)), Some((0, 0x6361_6d65)));
    }

    #[test]
    fn screen_uses_its_distinct_native_group() {
        assert_eq!(received_target(true, 3, 0x1111, Some(0x1234)), Some((0x1234, 0x7363_7265)));
    }

    #[test]
    fn native_unsigned_short_encoding_is_preserved() {
        assert_eq!(received_target(true, 1, 0x1111, Some(0x1234_abcd)), Some((0xabcd, 0x6361_6d65)));
    }

    #[test]
    fn unknown_metadata_keeps_legacy_fallback_available() {
        assert_eq!(received_target(true, 1, 0x1111, None), None);
        assert_eq!(received_target(true, 99, 0x1111, Some(1)), None);
    }
}
