use plist::{Dictionary, Value};
use serde::Serialize;

pub(crate) struct DelegateRequest<'a> {
    pub url: &'a str,
    pub body: Value,
    pub legacy: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct AuthRequest<'a> {
    apple_id: &'a str,
    client_id: &'a str,
    delegates: Dictionary,
    password: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V2AuthRequest<'a> {
    delegates: Dictionary,
    protocol_version: &'a str,
    user_info: V2AuthUserInfo<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct V2AuthUserInfo<'a> {
    client_id: &'a str,
    language: &'a str,
    timezone: &'a str,
}

pub(crate) fn prepare_delegate_request<'a>(
    compatibility: bool, default_url: &'a str, username: &str, pet: &str,
    client_id: &str, delegates: Dictionary,
) -> Result<DelegateRequest<'a>, plist::Error> {
    // Existing pre-v2 contract, also documented for remote-v3 Windows PETs in
    // anisette-v3-server issue #61. Do not mix its payload with the v2 endpoint.
    if compatibility && default_url == "https://setup.icloud.com/setup/signin/v2/login" {
        return Ok(DelegateRequest {
            url: "https://setup.icloud.com/setup/iosbuddy/loginDelegates",
            body: plist::to_value(&AuthRequest { apple_id: username, client_id, delegates, password: pet })?,
            legacy: true,
        });
    }
    Ok(DelegateRequest {
        url: default_url,
        body: plist::to_value(&V2AuthRequest {
            delegates, protocol_version: "1.0",
            user_info: V2AuthUserInfo { client_id, language: "en-US", timezone: "America/New_York" },
        })?,
        legacy: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const V2: &str = "https://setup.icloud.com/setup/signin/v2/login";

    fn delegates() -> Dictionary {
        [("com.apple.private.ids".to_string(), Value::Dictionary([
            ("protocol-version".to_string(), Value::String("4".to_string()))
        ].into_iter().collect())), ("com.apple.mobileme".to_string(), Value::Dictionary(Dictionary::new()))]
            .into_iter().collect()
    }

    #[test]
    fn compatibility_pairs_legacy_endpoint_with_legacy_credential_body() {
        let r = prepare_delegate_request(true, V2, "fake@example.com", "fake-pet", "CLIENT-ID", delegates()).unwrap();
        assert_eq!(r.url, "https://setup.icloud.com/setup/iosbuddy/loginDelegates");
        assert!(r.legacy);
        let body = r.body.as_dictionary().unwrap();
        assert_eq!(body["apple-id"].as_string(), Some("fake@example.com"));
        assert_eq!(body["password"].as_string(), Some("fake-pet"));
        assert_eq!(body["client-id"].as_string(), Some("CLIENT-ID"));
        assert_eq!(body["delegates"].as_dictionary().unwrap().len(), 2);
        assert!(!body.contains_key("protocolVersion"));
        assert!(!body.contains_key("userInfo"));
    }

    #[test]
    fn native_sessions_retain_v2_contract_without_pet_in_body() {
        let r = prepare_delegate_request(false, V2, "fake@example.com", "fake-pet", "CLIENT-ID", delegates()).unwrap();
        assert_eq!(r.url, V2);
        assert!(!r.legacy);
        let body = r.body.as_dictionary().unwrap();
        assert_eq!(body["protocolVersion"].as_string(), Some("1.0"));
        assert_eq!(body["userInfo"].as_dictionary().unwrap()["client-id"].as_string(), Some("CLIENT-ID"));
        assert!(!body.contains_key("password"));
        assert!(!body.contains_key("apple-id"));
    }

    #[test]
    fn unrecognized_endpoint_is_not_redirected() {
        let r = prepare_delegate_request(true, "https://example.invalid/custom", "u", "p", "id", delegates()).unwrap();
        assert_eq!(r.url, "https://example.invalid/custom");
        assert!(!r.legacy);
    }
}
