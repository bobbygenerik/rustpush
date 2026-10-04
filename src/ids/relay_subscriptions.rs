use std::collections::HashMap;
use super::qrp::IdsqrProtoSubscribedStream;

/// One-to-one media uses the wildcard established by AllocBind. An empty
/// SessionInfo subscription would replace that wildcard and stop relay media.
pub(super) fn media_subscriptions(
    relay_mode: bool,
    streams: &HashMap<u64, Vec<u32>>,
) -> (u32, Vec<IdsqrProtoSubscribedStream>) {
    if !relay_mode {
        return (6, vec![IdsqrProtoSubscribedStream {
            wildcard_subscription: Some(true),
            ..Default::default()
        }]);
    }
    (0, streams.iter().map(|(id, streams)| IdsqrProtoSubscribedStream {
        wildcard_subscription: None,
        peer_participant_id: Some(*id),
        peer_stream_ids: streams.clone(),
        is_seamless_transition: None,
    }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;
    use super::super::qrp::IdsqrProtoSessionInfoRequest;

    fn request(relay_mode: bool, streams: &HashMap<u64, Vec<u32>>) -> IdsqrProtoSessionInfoRequest {
        let (limit, subscriptions) = media_subscriptions(relay_mode, streams);
        let wire = IdsqrProtoSessionInfoRequest {
            max_concurrent_streams: Some(limit),
            subscribed_streams: subscriptions,
            ..Default::default()
        }.encode_to_vec();
        IdsqrProtoSessionInfoRequest::decode(&wire[..]).unwrap()
    }

    #[test]
    fn one_to_one_updates_keep_allocbind_wildcard() {
        let decoded = request(false, &HashMap::new());
        assert_eq!(decoded.max_concurrent_streams, Some(6));
        assert_eq!(decoded.subscribed_streams.len(), 1);
        assert_eq!(decoded.subscribed_streams[0].wildcard_subscription, Some(true));
        assert_eq!(decoded.subscribed_streams[0].peer_participant_id, None);
    }

    #[test]
    fn switching_to_one_to_one_replaces_group_streams_with_wildcard() {
        let decoded = request(false, &HashMap::from([(42, vec![10, 11])]));
        assert_eq!(decoded.subscribed_streams.len(), 1);
        assert_eq!(decoded.subscribed_streams[0].wildcard_subscription, Some(true));
        assert!(decoded.subscribed_streams[0].peer_stream_ids.is_empty());
    }

    #[test]
    fn group_calls_keep_explicit_peer_streams() {
        let decoded = request(true, &HashMap::from([(42, vec![10, 11])]));
        assert_eq!(decoded.max_concurrent_streams, Some(0));
        assert_eq!(decoded.subscribed_streams[0].wildcard_subscription, None);
        assert_eq!(decoded.subscribed_streams[0].peer_participant_id, Some(42));
        assert_eq!(decoded.subscribed_streams[0].peer_stream_ids, vec![10, 11]);
    }
}
