use std::collections::HashMap;

/// Parse the existing stream-state dictionary without panicking on an unknown
/// or malformed peer key. Preserve valid entries and their original state;
/// parsing is not a claim that every advertised state/type is implemented.
pub(super) fn parse_stream_group_states(state: HashMap<String, u8>) -> HashMap<u32, u8> {
    state.into_iter().filter_map(|(key, value)| key.parse().ok().map(|key| (key, value))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_key_does_not_discard_valid_camera_and_audio_states() {
        let parsed = parse_stream_group_states(HashMap::from([
            ("1".into(), 1), ("2".into(), 2), ("camera".into(), 2),
            ("4294967296".into(), 1), ("-1".into(), 1), ("".into(), 1),
        ]));
        assert_eq!(parsed, HashMap::from([(1, 1), (2, 2)]));
    }

    #[test]
    fn preserves_native_aliases_and_unknown_values_without_inventing_semantics() {
        let parsed = parse_stream_group_states(HashMap::from([
            ("1".into(), 0), ("5".into(), 2), ("128".into(), 1),
            ("4294967295".into(), 255), ("0".into(), 255),
        ]));
        assert_eq!(parsed, HashMap::from([(1, 0), (5, 2), (128, 1), (u32::MAX, 255), (0, 255)]));
    }

    #[test]
    fn empty_and_entirely_unknown_dictionaries_are_safe() {
        assert!(parse_stream_group_states(HashMap::new()).is_empty());
        assert!(parse_stream_group_states(HashMap::from([("new-state".into(), 1)])).is_empty());
    }
}
