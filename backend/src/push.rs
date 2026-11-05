//! Web Push: messages to a player's devices while the site is closed.
//!
//! Payloads are encrypted for the browser (RFC 8291, `aes128gcm`) and the
//! server identifies itself to push services with VAPID (RFC 8292). Only
//! endpoints of known push services are accepted, so the server never posts
//! to an address a player made up.

use std::sync::Mutex;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes128Gcm, Nonce};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use hkdf::Hkdf;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use p256::elliptic_curve::sec1::ToEncodedPoint;
use p256::{PublicKey, SecretKey};
use sha2::Sha256;

/// Record size announced in the header; one record carries the whole payload
const RECORD_SIZE: u32 = 4096;
/// Push services keep an undelivered message this long
pub const TTL_SECONDS: u32 = 24 * 3600;

/// Hosts of the push services browsers use. Anything else is refused.
const PUSH_HOSTS: [&str; 5] = [
    "fcm.googleapis.com",
    "updates.push.services.mozilla.com",
    ".notify.windows.com",
    ".push.apple.com",
    ".push.services.mozilla.com",
];

/// Whether an endpoint is an https URL of a known push service
pub fn allowed_endpoint(endpoint: &str) -> bool {
    let Some(rest) = endpoint.strip_prefix("https://") else { return false };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    // No user info, no port tricks: a plain host name
    if host.is_empty() || host.contains(['@', ':']) || endpoint.len() > 1024 {
        return false;
    }
    PUSH_HOSTS.iter().any(|h| if h.starts_with('.') { host.ends_with(h) } else { host == *h })
}

/// `https://host` of an endpoint: the audience of the VAPID token
fn origin(endpoint: &str) -> Option<&str> {
    let rest = endpoint.strip_prefix("https://")?;
    let end = rest.find('/').unwrap_or(rest.len());
    Some(&endpoint[..8 + end])
}

#[derive(Debug, thiserror::Error)]
pub enum PushError {
    #[error("bad key: {0}")]
    Key(&'static str),
    #[error("encryption failed")]
    Crypto,
}

/// The browser side of a subscription, as the Push API gives it
#[derive(Debug, Clone)]
pub struct Target {
    pub endpoint: String,
    /// The browser's P-256 key, base64url
    pub p256dh: String,
    /// The 16 byte auth secret, base64url
    pub auth: String,
}

/// Checks the keys of a subscription before storing it
pub fn valid_keys(p256dh: &str, auth: &str) -> bool {
    let key = B64.decode(p256dh.trim_end_matches('=')).ok();
    let auth = B64.decode(auth.trim_end_matches('=')).ok();
    key.is_some_and(|k| PublicKey::from_sec1_bytes(&k).is_ok()) && auth.is_some_and(|a| a.len() == 16)
}

/// Encrypts a payload for one browser (RFC 8291) with a given server key and
/// salt; `encrypt` picks fresh ones. Returns the `aes128gcm` body.
pub fn encrypt_with(
    as_secret: &SecretKey,
    salt: &[u8; 16],
    ua_public: &[u8],
    auth: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, PushError> {
    let ua = PublicKey::from_sec1_bytes(ua_public).map_err(|_| PushError::Key("p256dh"))?;
    let as_public = as_secret.public_key().to_encoded_point(false);
    let shared = p256::ecdh::diffie_hellman(as_secret.to_nonzero_scalar(), ua.as_affine());

    let mut key_info = b"WebPush: info\0".to_vec();
    key_info.extend_from_slice(ua_public);
    key_info.extend_from_slice(as_public.as_bytes());
    let mut ikm = [0u8; 32];
    Hkdf::<Sha256>::new(Some(auth), shared.raw_secret_bytes())
        .expand(&key_info, &mut ikm)
        .map_err(|_| PushError::Crypto)?;

    let prk = Hkdf::<Sha256>::new(Some(salt), &ikm);
    let mut cek = [0u8; 16];
    let mut nonce = [0u8; 12];
    prk.expand(b"Content-Encoding: aes128gcm\0", &mut cek).map_err(|_| PushError::Crypto)?;
    prk.expand(b"Content-Encoding: nonce\0", &mut nonce).map_err(|_| PushError::Crypto)?;

    // One record, the last: the payload then the 0x02 delimiter, no padding
    let mut record = plaintext.to_vec();
    record.push(2);
    let sealed = Aes128Gcm::new_from_slice(&cek)
        .map_err(|_| PushError::Crypto)?
        .encrypt(Nonce::from_slice(&nonce), record.as_slice())
        .map_err(|_| PushError::Crypto)?;

    let mut body = Vec::with_capacity(16 + 4 + 1 + 65 + sealed.len());
    body.extend_from_slice(salt);
    body.extend_from_slice(&RECORD_SIZE.to_be_bytes());
    body.push(as_public.as_bytes().len() as u8);
    body.extend_from_slice(as_public.as_bytes());
    body.extend_from_slice(&sealed);
    Ok(body)
}

/// Encrypts a payload for a subscription with a fresh key and salt
pub fn encrypt(target: &Target, plaintext: &[u8]) -> Result<Vec<u8>, PushError> {
    let ua = B64.decode(target.p256dh.trim_end_matches('=')).map_err(|_| PushError::Key("p256dh"))?;
    let auth = B64.decode(target.auth.trim_end_matches('=')).map_err(|_| PushError::Key("auth"))?;
    let mut rng = p256::elliptic_curve::rand_core::OsRng;
    let salt: [u8; 16] = rand::random();
    encrypt_with(&SecretKey::random(&mut rng), &salt, &ua, &auth, plaintext)
}

/// Opens an `aes128gcm` body with the browser's keys: what a browser does.
/// Tests use it to check what the server sends.
pub fn decrypt(ua_secret: &SecretKey, auth: &[u8], body: &[u8]) -> Result<Vec<u8>, PushError> {
    if body.len() < 21 {
        return Err(PushError::Crypto);
    }
    let salt = &body[..16];
    let idlen = body[20] as usize;
    let as_public_bytes = body.get(21..21 + idlen).ok_or(PushError::Crypto)?;
    let as_public = PublicKey::from_sec1_bytes(as_public_bytes).map_err(|_| PushError::Key("keyid"))?;
    let ua_public = ua_secret.public_key().to_encoded_point(false);
    let shared = p256::ecdh::diffie_hellman(ua_secret.to_nonzero_scalar(), as_public.as_affine());
    let mut key_info = b"WebPush: info\0".to_vec();
    key_info.extend_from_slice(ua_public.as_bytes());
    key_info.extend_from_slice(as_public_bytes);
    let mut ikm = [0u8; 32];
    Hkdf::<Sha256>::new(Some(auth), shared.raw_secret_bytes())
        .expand(&key_info, &mut ikm)
        .map_err(|_| PushError::Crypto)?;
    let prk = Hkdf::<Sha256>::new(Some(salt), &ikm);
    let mut cek = [0u8; 16];
    let mut nonce = [0u8; 12];
    prk.expand(b"Content-Encoding: aes128gcm\0", &mut cek).map_err(|_| PushError::Crypto)?;
    prk.expand(b"Content-Encoding: nonce\0", &mut nonce).map_err(|_| PushError::Crypto)?;
    let mut plain = Aes128Gcm::new_from_slice(&cek)
        .map_err(|_| PushError::Crypto)?
        .decrypt(Nonce::from_slice(&nonce), &body[21 + idlen..])
        .map_err(|_| PushError::Crypto)?;
    // Strip padding back to the delimiter
    while plain.last() == Some(&0) {
        plain.pop();
    }
    if plain.pop() != Some(2) {
        return Err(PushError::Crypto);
    }
    Ok(plain)
}

/// The server's VAPID identity
pub struct Vapid {
    key: SigningKey,
    /// `mailto:` or `https:` contact for push services
    subject: String,
}

impl Vapid {
    /// From the base64url private scalar in the config
    pub fn new(private_b64: &str, subject: String) -> Result<Self, PushError> {
        let bytes = B64.decode(private_b64.trim().trim_end_matches('=')).map_err(|_| PushError::Key("vapid"))?;
        let key = SigningKey::from_slice(&bytes).map_err(|_| PushError::Key("vapid"))?;
        Ok(Vapid { key, subject })
    }

    /// A fresh key pair, base64url: (private, public)
    pub fn generate() -> (String, String) {
        let key = SigningKey::random(&mut p256::elliptic_curve::rand_core::OsRng);
        let public = key.verifying_key().to_encoded_point(false);
        (B64.encode(key.to_bytes()), B64.encode(public.as_bytes()))
    }

    /// The public key browsers subscribe with (`applicationServerKey`)
    pub fn public_key(&self) -> String {
        B64.encode(self.key.verifying_key().to_encoded_point(false).as_bytes())
    }

    /// The `Authorization` header for one push service (RFC 8292)
    pub fn authorization(&self, endpoint: &str, now: i64) -> Option<String> {
        let header = B64.encode(br#"{"typ":"JWT","alg":"ES256"}"#);
        let claims = serde_json::json!({ "aud": origin(endpoint)?, "exp": now + 12 * 3600, "sub": self.subject });
        let claims = B64.encode(claims.to_string());
        let signed = format!("{header}.{claims}");
        let sig: Signature = self.key.sign(signed.as_bytes());
        Some(format!("vapid t={signed}.{}, k={}", B64.encode(sig.to_bytes()), self.public_key()))
    }
}

/// One message on its way to a push service
#[derive(Debug, Clone)]
pub struct Delivery {
    pub endpoint: String,
    pub authorization: String,
    pub body: Vec<u8>,
}

/// What became of a delivery
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Sent,
    /// The subscription is gone (404, 410): forget it
    Gone,
    Failed,
}

/// Where deliveries go: push services, or a list tests read
pub enum Transport {
    Http(reqwest::Client),
    Memory(Mutex<Vec<Delivery>>),
}

impl Transport {
    pub fn http() -> Self {
        Transport::Http(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("http client builds"),
        )
    }

    pub async fn send(&self, d: Delivery) -> Outcome {
        match self {
            Transport::Memory(sent) => {
                sent.lock().unwrap().push(d);
                Outcome::Sent
            }
            Transport::Http(client) => {
                let res = client
                    .post(&d.endpoint)
                    .header("authorization", d.authorization)
                    .header("content-encoding", "aes128gcm")
                    .header("content-type", "application/octet-stream")
                    .header("ttl", TTL_SECONDS.to_string())
                    .header("urgency", "normal")
                    .body(d.body)
                    .send()
                    .await;
                match res {
                    Ok(r) if r.status().is_success() => Outcome::Sent,
                    Ok(r) if r.status().as_u16() == 404 || r.status().as_u16() == 410 => Outcome::Gone,
                    Ok(r) => {
                        tracing::warn!(status = %r.status(), "push refused");
                        Outcome::Failed
                    }
                    Err(e) => {
                        tracing::warn!("push failed: {e}");
                        Outcome::Failed
                    }
                }
            }
        }
    }

    /// Deliveries the memory transport kept for one endpoint, taken out
    pub fn take_for(&self, endpoint: &str) -> Vec<Delivery> {
        match self {
            Transport::Memory(sent) => {
                let mut sent = sent.lock().unwrap();
                let (mine, rest) = std::mem::take(&mut *sent).into_iter().partition(|d| d.endpoint == endpoint);
                *sent = rest;
                mine
            }
            Transport::Http(_) => Vec::new(),
        }
    }
}

/// Web Push for the app: off when no VAPID key is configured
pub struct Push {
    pub vapid: Option<Vapid>,
    pub transport: Transport,
}

impl Push {
    pub fn disabled() -> Self {
        Push { vapid: None, transport: Transport::Memory(Mutex::default()) }
    }

    /// Encrypts and signs a message for one subscription
    pub fn prepare(&self, target: &Target, payload: &[u8], now: i64) -> Option<Delivery> {
        let vapid = self.vapid.as_ref()?;
        let body = encrypt(target, payload).map_err(|e| tracing::warn!("push encrypt: {e}")).ok()?;
        Some(Delivery {
            endpoint: target.endpoint.clone(),
            authorization: vapid.authorization(&target.endpoint, now)?,
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::VerifyingKey;
    use p256::ecdsa::signature::Verifier;

    fn b64(s: &str) -> Vec<u8> {
        B64.decode(s).unwrap()
    }

    /// RFC 8291, Appendix A
    #[test]
    fn encrypts_the_rfc_example() {
        let as_secret = SecretKey::from_slice(&b64("yfWPiYE-n46HLnH0KqZOF1fJJU3MYrct3AELtAQ-oRw")).unwrap();
        let ua_public = b64("BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4");
        let auth = b64("BTBZMqHH6r4Tts7J_aSIgg");
        let salt: [u8; 16] = b64("DGv6ra1nlYgDCS1FRnbzlw").try_into().unwrap();
        let body =
            encrypt_with(&as_secret, &salt, &ua_public, &auth, b"When I grow up, I want to be a watermelon").unwrap();
        assert_eq!(
            B64.encode(&body),
            "DGv6ra1nlYgDCS1FRnbzlwAAEABBBP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A_yl95bQpu6cVPTpK4Mqgkf1CXztLVBSt2Ks3oZwbuwXPXLWyouBWLVWGNWQexSgSxsj_Qulcy4a-fN"
        );
        let ua_secret = SecretKey::from_slice(&b64("q1dXpw3UpT5VOmu_cf_v6ih07Aems3njxI-JWgLcM94")).unwrap();
        assert_eq!(decrypt(&ua_secret, &auth, &body).unwrap(), b"When I grow up, I want to be a watermelon");
    }

    #[test]
    fn a_fresh_message_opens_with_the_browser_keys() {
        let ua = SecretKey::random(&mut p256::elliptic_curve::rand_core::OsRng);
        let auth: [u8; 16] = rand::random();
        let target = Target {
            endpoint: "https://fcm.googleapis.com/fcm/send/abc".into(),
            p256dh: B64.encode(ua.public_key().to_encoded_point(false).as_bytes()),
            auth: B64.encode(auth),
        };
        assert!(valid_keys(&target.p256dh, &target.auth));
        let body = encrypt(&target, b"{\"title\":\"hi\"}").unwrap();
        assert_eq!(decrypt(&ua, &auth, &body).unwrap(), b"{\"title\":\"hi\"}");
        assert!(decrypt(&ua, &[0; 16], &body).is_err(), "the auth secret matters");
    }

    #[test]
    fn vapid_tokens_are_signed_for_the_push_service() {
        let (private, public) = Vapid::generate();
        let vapid = Vapid::new(&private, "mailto:ops@example.org".into()).unwrap();
        assert_eq!(vapid.public_key(), public);
        let auth = vapid.authorization("https://updates.push.services.mozilla.com/wpush/v2/xyz", 1000).unwrap();
        let (token, k) = auth.strip_prefix("vapid t=").unwrap().split_once(", k=").unwrap();
        assert_eq!(k, public);
        let parts: Vec<&str> = token.split('.').collect();
        let claims: serde_json::Value = serde_json::from_slice(&b64(parts[1])).unwrap();
        assert_eq!(claims["aud"], "https://updates.push.services.mozilla.com");
        assert_eq!(claims["exp"], 1000 + 12 * 3600);
        let key = VerifyingKey::from_sec1_bytes(&b64(&public)).unwrap();
        let sig = Signature::from_slice(&b64(parts[2])).unwrap();
        key.verify(format!("{}.{}", parts[0], parts[1]).as_bytes(), &sig).expect("valid ES256 signature");
    }

    #[test]
    fn only_push_services_are_reachable() {
        for ok in [
            "https://fcm.googleapis.com/fcm/send/abc",
            "https://updates.push.services.mozilla.com/wpush/v2/x",
            "https://wns2-par02p.notify.windows.com/w/?token=x",
            "https://web.push.apple.com/QF0",
        ] {
            assert!(allowed_endpoint(ok), "{ok}");
        }
        for bad in [
            "http://fcm.googleapis.com/fcm/send/abc",
            "https://127.0.0.1/x",
            "https://localhost/x",
            "https://evil.com/fcm.googleapis.com",
            "https://fcm.googleapis.com.evil.com/x",
            "https://user@fcm.googleapis.com/x",
            "https://fcm.googleapis.com:8443/x",
            "https://notify.windows.com.evil/x",
        ] {
            assert!(!allowed_endpoint(bad), "{bad}");
        }
    }
}
