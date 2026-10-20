use serde::Deserialize;

use crate::{activation::ActivationInfo, PushError};

/// Per-attempt activation identity, separate from the relay's login identity.
pub struct ActivationContext {
    pub info: ActivationInfo,
    pub device: String,
    pub user_agent: String,
}

#[derive(Deserialize)]
enum RelayActivation {
    Serial(SerialActivation),
}

#[derive(Deserialize)]
struct SerialActivation {
    activation_randomness: String,
    activation_state: String,
    build_version: String,
    device_class: String,
    product_type: String,
    product_version: String,
    serial_number: String,
    unique_device_id: Option<String>,
    activation_device: String,
    ua: Option<String>,
}

pub(crate) async fn prepare_relay_activation(
    client: &reqwest::Client,
    host: &str,
    code: &str,
    token: &Option<String>,
    fallback: ActivationContext,
) -> Result<ActivationContext, PushError> {
    // Verified in the signed OpenBubbles 20002286 client: an empty POST with
    // bearer authorization, separate from subscription/ticket activation.
    let mut request = client.post(format!("{}/api/v1/bridge/activate", host.trim_end_matches('/')))
        .bearer_auth(code)
        .header("Content-Length", "0");
    if let Some(token) = token {
        request = request.header("X-Beeper-Access-Token", token);
    }
    let response = request.send().await?;
    if response.status() != reqwest::StatusCode::OK {
        return Err(PushError::RelayError(response.status().as_u16(),
            "Hosted activation metadata unavailable".to_string()));
    }
    // Unknown variants and malformed responses must not silently reuse the
    // incompatible login identity. Only the verified Serial contract is ported.
    let RelayActivation::Serial(remote) = response.json().await?;
    Ok(ActivationContext {
        info: ActivationInfo {
            activation_randomness: remote.activation_randomness,
            activation_state: remote.activation_state,
            build_version: remote.build_version,
            device_cert_request: Vec::new().into(),
            device_class: remote.device_class,
            product_type: remote.product_type,
            product_version: remote.product_version,
            serial_number: remote.serial_number,
            // Native 0x152c888..0x152c8c0 uses the relay UUID when absent.
            unique_device_id: remote.unique_device_id
                .unwrap_or(fallback.info.unique_device_id).to_uppercase(),
        },
        device: remote.activation_device,
        // Native 0x152caa4..0x152cb08 defaults to get_normal_ua here.
        user_agent: remote.ua.unwrap_or(fallback.user_agent),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::{Read, Write}, net::TcpListener, thread};

    fn fallback() -> ActivationContext {
        ActivationContext {
            info: ActivationInfo {
                activation_randomness: "old-randomness".into(),
                activation_state: "Unactivated".into(),
                build_version: "19H386".into(),
                device_cert_request: Vec::new().into(),
                device_class: "MacOS".into(),
                product_type: "iMac13,1".into(),
                product_version: "15.8.3".into(),
                serial_number: "LOGIN_SERIAL".into(),
                unique_device_id: "relay-uuid".into(),
            },
            device: "MacOS".into(),
            user_agent: "ApplePushService/4.0 existing-agent".into(),
        }
    }

    fn serial_reply() -> serde_json::Value {
        serde_json::json!({"Serial": {
            "activation_randomness": "server-randomness",
            "activation_state": "Unactivated",
            "build_version": "22G513",
            "device_class": "MacOS",
            "product_type": "iMac13,1",
            "product_version": "13.6.4",
            "serial_number": "ACTIVATION_SERIAL",
            "unique_device_id": null,
            "activation_device": "MacOS",
            "ua": null
        }})
    }

    fn server(status: u16, body: String) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let worker = thread::spawn(move || {
            let start = std::time::Instant::now();
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if start.elapsed().as_secs() >= 3 { return String::new(); }
                        thread::sleep(std::time::Duration::from_millis(5));
                    },
                    Err(e) => panic!("{e}"),
                }
            };
            stream.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            write!(stream, "HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            String::from_utf8(request).unwrap()
        });
        (format!("http://{addr}"), worker)
    }

    #[tokio::test]
    async fn hosted_activation_uses_server_identity_instead_of_login_metadata() {
        let (host, worker) = server(200, serial_reply().to_string());
        let result = prepare_relay_activation(&reqwest::Client::new(), &host,
            "test-ticket", &None, fallback()).await.unwrap();
        assert_eq!(result.info.serial_number, "ACTIVATION_SERIAL");
        assert_eq!(result.info.build_version, "22G513");
        assert_eq!(result.info.product_version, "13.6.4");
        assert_eq!(result.info.activation_randomness, "server-randomness");
        assert_eq!(result.info.unique_device_id, "RELAY-UUID");
        assert_eq!(result.user_agent, "ApplePushService/4.0 existing-agent");
        let request = worker.join().unwrap().to_lowercase();
        assert!(request.starts_with("post /api/v1/bridge/activate http/1.1\r\n"));
        assert!(request.contains("authorization: bearer test-ticket\r\n"));
        assert!(request.contains("content-length: 0\r\n"));
    }

    #[tokio::test]
    async fn supplied_uid_and_agent_override_relay_defaults() {
        let mut body = serial_reply();
        body["Serial"]["unique_device_id"] = "server-uuid".into();
        body["Serial"]["ua"] = "server-agent".into();
        body["Serial"]["activation_device"] = "server-device".into();
        let (host, worker) = server(200, body.to_string());
        let result = prepare_relay_activation(&reqwest::Client::new(), &host,
            "test-ticket", &Some("test-relay-token".into()), fallback()).await.unwrap();
        assert_eq!(result.info.unique_device_id, "SERVER-UUID");
        assert_eq!(result.user_agent, "server-agent");
        assert_eq!(result.device, "server-device");
        assert!(worker.join().unwrap().to_lowercase()
            .contains("x-beeper-access-token: test-relay-token\r\n"));
    }

    #[tokio::test]
    async fn invalid_response_is_an_error_instead_of_legacy_activation() {
        for body in ["{}", "{\"Unknown\":{}}", "{\"Serial\":{}}"] {
            let (host, worker) = server(200, body.into());
            let result = prepare_relay_activation(&reqwest::Client::new(), &host,
                "test-ticket", &None, fallback()).await;
            assert!(result.is_err(), "invalid hosted response must not fall back");
            worker.join().unwrap();
        }
    }

    #[tokio::test]
    async fn http_failure_preserves_status_without_falling_back() {
        for status in [401, 404, 503] {
            let (host, worker) = server(status, "device unavailable".into());
            let result = prepare_relay_activation(&reqwest::Client::new(), &host,
                "test-ticket", &None, fallback()).await;
            assert!(matches!(result, Err(PushError::RelayError(code, _)) if code == status));
            worker.join().unwrap();
        }
    }
}
