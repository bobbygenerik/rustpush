use rustpush::avconference::{is_remote_audio_expected, RemoteMediaState};

#[test]
fn audio_expected_armed_from_call_start_and_disarmed_on_pause() {
    assert!(is_remote_audio_expected(None));
    assert!(!is_remote_audio_expected(Some(RemoteMediaState::Paused)));
    assert!(is_remote_audio_expected(Some(RemoteMediaState::Enabled)));
    assert!(!is_remote_audio_expected(Some(RemoteMediaState::Disabled)));
    assert!(!is_remote_audio_expected(Some(RemoteMediaState::Unresolved)));
}
