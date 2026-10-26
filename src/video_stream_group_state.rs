use std::collections::HashMap;

/// Decode individual integer entries after validating the plist envelope. A
/// future/invalid value must not discard valid camera/microphone entries in
/// the same message. Preserve representable unknown values for diagnosis.
pub(super) fn parse_stream_group_state_payload(payload: &[u8]) -> Option<HashMap<u32, u8>> {
    let plist::Value::Dictionary(entries) = plist::from_bytes(payload).ok()? else { return None; };
    Some(parse_stream_group_states(entries.into_iter().filter_map(|(key, value)| {
        let value = u8::try_from(value.as_unsigned_integer()?).ok()?;
        Some((key, value))
    }).collect()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RemoteMediaState {
    Disabled,
    Enabled,
    Paused,
    // Neither unordered alias iteration nor a new peer value establishes a
    // known state. Do not turn this into a pause or an enable policy.
    Unresolved,
}

/// Bounded observations for the three currently supported media types. These
/// belong to a participant incarnation, not a call-wide camera boolean.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct RemoteMediaStates {
    pub(super) camera: Option<RemoteMediaState>,
    pub(super) microphone: Option<RemoteMediaState>,
    pub(super) screen: Option<RemoteMediaState>,
}

impl RemoteMediaStates {
    pub(super) fn update(&mut self, message: &HashMap<u32, u8>) {
        for (aliases, current) in [
            (&[1, 5, 128][..], &mut self.camera),
            (&[2, 6, 129][..], &mut self.microphone),
            (&[3, 130][..], &mut self.screen),
        ] {
            let mut values = aliases.iter().filter_map(|key| message.get(key).copied());
            let Some(first) = values.next() else { continue; };
            let mut enabled = first == 1;
            let mut conflicting = first > 2;
            for value in values {
                enabled |= value == 1;
                conflicting |= value != first || value > 2;
            }
            *current = Some(if enabled { RemoteMediaState::Enabled }
                else if conflicting { RemoteMediaState::Unresolved }
                else if first == 0 { RemoteMediaState::Disabled }
                else { RemoteMediaState::Paused });
        }
    }
}

/// Existing FaceTime camera state encoding: enabled1, paused2. These are
/// media states, not logical stream IDs or bitrate ladder entries.
pub(super) fn advertised_camera_state(video_enabled: bool) -> u8 {
    if video_enabled { 1 } else { 2 }
}

/// Preserve the existing FIR response keys while reporting the real camera
/// state; a request for repair must not announce a paused camera as enabled.
pub(super) fn fir_response_stream_states(video_enabled: bool) -> HashMap<u32, u8> {
    HashMap::from([(1, advertised_camera_state(video_enabled)), (2, 1)])
}

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
    fn remote_states_are_unknown_until_observed_and_media_types_stay_separate() {
        let mut states = RemoteMediaStates::default();
        assert_eq!(states.camera, None);
        states.update(&HashMap::from([(128, 2), (6, 1), (130, 0)]));
        assert_eq!(states.camera, Some(RemoteMediaState::Paused));
        assert_eq!(states.microphone, Some(RemoteMediaState::Enabled));
        assert_eq!(states.screen, Some(RemoteMediaState::Disabled));
        // A partial microphone message does not implicitly enable the camera.
        states.update(&HashMap::from([(2, 2)]));
        assert_eq!(states.camera, Some(RemoteMediaState::Paused));
        assert_eq!(states.microphone, Some(RemoteMediaState::Paused));
    }

    #[test]
    fn enabled_alias_wins_regardless_of_hashmap_iteration_order() {
        for enabled_alias in [1, 5, 128] {
            for _ in 0..32 {
                let mut message = HashMap::from([(1, 0), (5, 2), (128, 255)]);
                message.insert(enabled_alias, 1);
                let mut states = RemoteMediaStates::default();
                states.update(&message);
                assert_eq!(states.camera, Some(RemoteMediaState::Enabled));
            }
        }
    }

    #[test]
    fn conflicting_or_invalid_non_enabled_aliases_are_not_a_guessed_pause() {
        let mut states = RemoteMediaStates::default();
        for message in [HashMap::from([(1, 0), (5, 2)]),
            HashMap::from([(1, 2), (128, 255)]), HashMap::from([(1, 255)])] {
            states.update(&message);
            assert_eq!(states.camera, Some(RemoteMediaState::Unresolved));
        }
        states.update(&HashMap::from([(1, 2), (5, 2), (128, 2)]));
        assert_eq!(states.camera, Some(RemoteMediaState::Paused));
        states.update(&HashMap::from([(7, 1), (u32::MAX, 1)]));
        assert_eq!(states.camera, Some(RemoteMediaState::Paused));
        states = RemoteMediaStates::default();
        assert_eq!(states, RemoteMediaStates { camera: None, microphone: None, screen: None });
    }

    #[test]
    fn fir_response_preserves_actual_camera_pause_without_alias_changes() {
        assert_eq!(fir_response_stream_states(true), HashMap::from([(1, 1), (2, 1)]));
        assert_eq!(fir_response_stream_states(false), HashMap::from([(1, 2), (2, 1)]));
        assert_eq!(advertised_camera_state(true), 1);
        assert_eq!(advertised_camera_state(false), 2);
    }

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
