use keystore::{KeystoreAccessRules, KeystoreDigest, KeystorePadding, KeystorePublicKey, KeystoreSignKey, RsaKey};
use openssl::{hash::MessageDigest, nid::Nid, pkey::{PKey, Params}, rsa::{Padding, Rsa}, sign::Signer, x509::{X509, X509Name, X509Req}};
use plist::{Data, Value};
use rand::{seq::SliceRandom, thread_rng};
use rasn::types::Oid;
use regex::Regex;
use reqwest::Version;
use serde::Serialize;
use x509_cert::{attr::AttributeTypeAndValue, der::{Decode, Encode, EncodePem, asn1::{BitString, Null, SetOfVec, Utf8StringRef}, pem::LineEnding}, name::{Name, RdnSequence, RelativeDistinguishedName}, request::{CertReq, CertReqInfo}, spki::{AlgorithmIdentifier, ObjectIdentifier, SubjectPublicKeyInfo}};

use crate::{OSConfig, PushError, util::{KeyPair, KeyPairNew, REQWEST, get_nested_value, plist_to_buf, plist_to_string}};

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ActivationInfo {
    pub activation_randomness: String,
    pub activation_state: String,
    pub build_version: String,
    pub device_cert_request: Data,
    pub device_class: String,
    pub product_type: String,
    pub product_version: String,
    pub serial_number: String,
    #[serde(rename = "UniqueDeviceID")]
    pub unique_device_id: String
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct ActivationRequest {
    activation_info_complete: bool,
    #[serde(rename = "ActivationInfoXML")]
    activation_info_xml: Data,
    fair_play_cert_chain: Data,
    fair_play_signature: Data
}

macro_rules! include_cert {
    ($name:literal) => {
        (include_bytes!(concat!("../certs/fairplay/", $name, ".crt")), include_bytes!(concat!("../certs/fairplay/", $name, ".pem")))
    };
}

const FAIRPLAY_KEYS: &[(&'static [u8], &'static [u8])] = &[
    include_cert!("4056631661436364584235346952193"),
    include_cert!("4056631661436364584235346952194"),
    include_cert!("4056631661436364584235346952195"),
    include_cert!("4056631661436364584235346952196"),
    include_cert!("4056631661436364584235346952197"),
    include_cert!("4056631661436364584235346952198"),
    include_cert!("4056631661436364584235346952199"),
    include_cert!("4056631661436364584235346952200"),
    include_cert!("4056631661436364584235346952201"),
    include_cert!("4056631661436364584235346952208"),
];

fn fairplay_sign(data: &[u8]) -> Result<(&'static [u8], Vec<u8>), PushError> {
    let (cert, key) = FAIRPLAY_KEYS.choose(&mut thread_rng()).expect("no keys!");

    let key = PKey::private_key_from_der(&key)?;

    let mut signer = Signer::new(MessageDigest::sha1(), &key)?;
    signer.set_rsa_padding(Padding::PKCS1)?;
    Ok((*cert, signer.sign_oneshot_to_vec(data)?))
}

fn extract_activation_certificate(body: &str) -> Result<Vec<u8>, PushError> {
    // Albert can return HTTP 200 with an empty plist and an error in the
    // surrounding activation document. Keep the actual rejection diagnostic.
    let rejection = Regex::new(r"ErrorKey:\s*<SetFontStyle\b[^>]*>\s*([A-Z][A-Z0-9_]{0,127})\s*</SetFontStyle>").unwrap();
    if let Some(captures) = rejection.captures(body) {
        return Err(PushError::ActivationRejected(captures[1].to_owned()));
    }
    let protocol = Regex::new(r"(?s)<Protocol>(.*)</Protocol>").unwrap();
    let captures = protocol.captures(body).ok_or(PushError::AlbertCertParseError)?;
    let parsed: Value = plist::from_bytes(captures[1].as_bytes())
        .map_err(|_| PushError::AlbertCertParseError)?;
    get_nested_value(&parsed, &["device-activation", "activation-record", "DeviceCertificate"])
        .and_then(|value| value.as_data()).map(|data| data.to_vec())
        .ok_or(PushError::AlbertCertParseError)
}

#[cfg(test)]
mod response_tests {
    use super::*;

    #[test]
    fn apple_integrity_rejection_is_not_reported_as_a_parse_failure() {
        let body = r#"<Document><Protocol><plist version="1.0"><dict/></plist></Protocol>
            <TextView>ErrorKey: <SetFontStyle normalStyle="gray">DEVICE_INFO_INTEGRITY_FAILED</SetFontStyle></TextView>
            </Document>"#;
        let error = extract_activation_certificate(body).unwrap_err();
        assert!(error.to_string().contains("DEVICE_INFO_INTEGRITY_FAILED"));
        assert!(!matches!(error, PushError::AlbertCertParseError));
    }

    #[test]
    fn multiline_success_keeps_certificate_bytes() {
        let body = "<Protocol>\n<plist version=\"1.0\"><dict><key>device-activation</key><dict><key>activation-record</key><dict><key>DeviceCertificate</key><data>AQID</data></dict></dict></dict></plist>\n</Protocol>";
        assert_eq!(extract_activation_certificate(body).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn malformed_or_empty_protocol_is_still_a_parse_failure() {
        for body in ["not an activation response", "<Protocol><plist><dict/></plist></Protocol>"] {
            assert!(matches!(extract_activation_certificate(body), Err(PushError::AlbertCertParseError)));
        }
    }
}

pub async fn activate(os_config: &dyn OSConfig) -> Result<KeyPairNew<RsaKey>, PushError> {
    // Hosted activation can supply an identity distinct from the relay's login
    // identity. Resolve it before creating a key or constructing the signed CSR.
    let mut context = os_config.prepare_activation().await?;
    let key = RsaKey::overwrite(&format!("activation:{}", context.info.serial_number), 1024, KeystoreAccessRules {
        signature_padding: vec![KeystorePadding::PKCS1],
        digests: vec![KeystoreDigest::Sha1],
        can_sign: true,
        ..Default::default()
    })?;

    let public_key = SubjectPublicKeyInfo::from_der(&key.get_public_key()?).unwrap();
    let request = CertReqInfo {
        version: x509_cert::request::Version::V1,
        subject: RdnSequence(vec![
            RelativeDistinguishedName(SetOfVec::from_iter([
                AttributeTypeAndValue {
                    oid: ObjectIdentifier::new("2.5.4.3").unwrap(),
                    value: Utf8StringRef::new("Client Push Certificate").unwrap().into(),
                }
            ]).unwrap())
        ]),
        public_key,
        attributes: SetOfVec::new(),
    };

    let req_info = request.to_der().unwrap();

    let sign = key.sign(KeystoreDigest::Sha1, KeystorePadding::PKCS1, &req_info)?;

    let result = CertReq {
        info: request,
        algorithm: AlgorithmIdentifier {
            oid: ObjectIdentifier::new("1.2.840.113549.1.1.5").unwrap(),
            parameters: Some(Null.into()),
        },
        signature: BitString::from_bytes(&sign).unwrap(),
    };
    
    let csr = result.to_pem(LineEnding::LF).unwrap();

    context.info.device_cert_request = csr.into_bytes().into();
    let activation_bytes = plist_to_buf(&context.info)?;

    let (fair_play_cert_chain, fair_play_signature) = fairplay_sign(&activation_bytes)?;
    
    let request = ActivationRequest {
        activation_info_complete: true,
        activation_info_xml: activation_bytes.into(),
        fair_play_cert_chain: fair_play_cert_chain.to_vec().into(),
        fair_play_signature: fair_play_signature.into(),
    };

    #[derive(Serialize)]
    #[serde(rename_all = "kebab-case")]
    struct FormBody {
        activation_info: String,
    }

    let mut url = reqwest::Url::parse("https://albert.apple.com/deviceservices/deviceActivation").unwrap();
    url.query_pairs_mut().append_pair("device", &context.device);
    let resp = REQWEST
        .post(url)
        .header("User-Agent", &context.user_agent)
        .form(&FormBody { activation_info: plist_to_string(&request)? })
        .send().await?;

    let status = resp.status();
    let body = resp.text().await?;

    let certificate = extract_activation_certificate(&body).map_err(|error| {
        log::error!("Albert activation failed. HTTP status: {}, {}", status, error);
        error
    })?;

    Ok(KeyPairNew {
        cert: X509::from_pem(&certificate)?.to_der()?,
        private: key
    })
}

