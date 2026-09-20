//! Web Push 加密与鉴权后端（从 push_http.rs 按域拆出）：
//! RFC 8291 载荷加密（aes128gcm）、RFC 8292 VAPID JWT、单订阅投递。

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::Aes128Gcm;
use base64::Engine;
use hkdf::Hkdf;
use p256::ecdh::EphemeralSecret;
use p256::elliptic_curve::sec1::FromEncodedPoint;
use p256::elliptic_curve::sec1::ToEncodedPoint;
use p256::{EncodedPoint, PublicKey};
use sha2::Sha256;

pub(super) fn b64url() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
}

pub(super) fn decode_client_key(p256dh_b64: &str) -> anyhow::Result<PublicKey> {
    let raw = b64url()
        .decode(p256dh_b64)
        .map_err(|e| anyhow::anyhow!("base64: {e}"))?;
    let point = EncodedPoint::from_bytes(raw)
        .map_err(|e| anyhow::anyhow!("sec1 point: {e}"))?;
    PublicKey::from_encoded_point(&point)
        .into_option()
        .ok_or_else(|| anyhow::anyhow!("非 P-256 公钥"))
}

/// 单订阅加密投递。返回推送服务响应分类。
#[derive(PartialEq)]
pub(super) enum PushResult {
    Delivered,
    Gone,         // 404/410：订阅失效，调用方应清理
    ServiceError, // 其它 4xx/5xx：推送服务问题，不清理
}

pub(super) struct VapidKeys {
    pub public_b64: String,
    pub secret: p256::SecretKey,
}

pub(super) fn vapid_from_env() -> Option<VapidKeys> {
    let public_b64 = std::env::var("VAPID_PUBLIC_KEY").unwrap_or_default();
    let private_b64 = std::env::var("VAPID_PRIVATE_KEY").unwrap_or_default();
    if public_b64.is_empty() || private_b64.is_empty() {
        return None;
    }
    let raw = b64url().decode(&private_b64).ok()?;
    let secret = p256::SecretKey::from_slice(&raw).ok()?;
    Some(VapidKeys { public_b64, secret })
}

/// RFC 8292：ES256 JWT，aud = 推送服务 origin，exp 12h，sub = mailto。
fn vapid_header(keys: &VapidKeys, endpoint: &str) -> anyhow::Result<String> {
    use p256::ecdsa::signature::Signer;
    use p256::ecdsa::SigningKey;
    let aud = {
        let u = url::Url::parse(endpoint)?;
        format!("{}://{}", u.scheme(), u.host_str().unwrap_or_default())
    };
    let header = b64url().encode(br#"{"typ":"JWT","alg":"ES256"}"#);
    let now = chrono::Utc::now().timestamp();
    let claims = serde_json::json!({
        "aud": aud,
        "exp": now + 12 * 3600,
        "sub": "mailto:ops@fluxtorrent.local",
    });
    let claims_b64 = b64url().encode(claims.to_string());
    let signing_input = format!("{header}.{claims_b64}");
    let signing_key = SigningKey::from(&keys.secret);
    let sig: p256::ecdsa::Signature =
        signing_key.sign(signing_input.as_bytes());
    let sig_b64 = b64url().encode(sig.to_bytes());
    Ok(format!(
        "vapid t={signing_input}.{sig_b64}, k={}",
        keys.public_b64
    ))
}

pub(super) async fn deliver(
    endpoint: &str,
    p256dh_b64: &str,
    auth_b64: &str,
    title: &str,
    body: &str,
    vapid: &Option<VapidKeys>,
) -> PushResult {
    let Some(vapid) = vapid else {
        return PushResult::ServiceError; // 未配置 VAPID：无法投递（订阅管理不受影响）
    };
    let payload =
        serde_json::json!({ "title": title, "body": body }).to_string();
    let body = match encrypt_payload(&payload, p256dh_b64, auth_b64) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(?e, "push encrypt failed");
            return PushResult::ServiceError;
        }
    };
    let auth_header = match vapid_header(vapid, endpoint) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!(?e, "vapid jwt failed");
            return PushResult::ServiceError;
        }
    };
    let client = match reqwest::Client::builder().build() {
        Ok(c) => c,
        Err(_) => return PushResult::ServiceError,
    };
    let res = client
        .post(endpoint)
        .header("TTL", "2419200")
        .header("Urgency", "normal")
        .header("Content-Encoding", "aes128gcm")
        .header("Authorization", auth_header)
        .body(body)
        .send()
        .await;
    match res {
        Ok(r) if r.status().is_success() => PushResult::Delivered,
        Ok(r) if [404u16, 410].contains(&r.status().as_u16()) => {
            PushResult::Gone
        }
        Ok(r) => {
            tracing::warn!(code = r.status().as_u16(), "push service error");
            PushResult::ServiceError
        }
        Err(e) => {
            tracing::warn!(?e, "push network error");
            PushResult::ServiceError
        }
    }
}

/// RFC 8291 §4：ECDH on P-256 + HKDF → CEK/Nonce → aes128gcm 单块加密 + crams 头。
fn encrypt_payload(
    plaintext: &str,
    p256dh_b64: &str,
    auth_b64: &str,
) -> anyhow::Result<Vec<u8>> {
    let client_pub = decode_client_key(p256dh_b64)?;
    let auth_secret = b64url()
        .decode(auth_b64)
        .map_err(|e| anyhow::anyhow!("auth base64: {e}"))?;
    let server_secret = EphemeralSecret::random(&mut rand::rngs::OsRng);
    let server_pub = server_secret.public_key();
    let shared = server_secret.diffie_hellman(&client_pub);

    let server_pub_bytes = server_pub.to_encoded_point(false);

    // PRK = HKDF-Extract(salt=auth_secret, IKM=shared)
    let hk = Hkdf::<Sha256>::new(Some(&auth_secret), shared.raw_secret_bytes());
    // §4：key material = HKDF-Expand(PRK, "WebPush: info" || 0x01 || "aes128gcm", 16+12+16? )
    // 规范口径：context = "WebPush: info" || 0x01 || client_pub(65B) || server_pub(65B)
    // 两次 HKDF-Expand（RFC 8291 §3.2）：
    //   CEK   = Expand(PRK, "Content-Encoding: aes128gcm" || 0x01, 16)
    //   NONCE = Expand(PRK, "Content-Encoding: nonce"     || 0x01, 12)
    let mut cek = [0u8; 16];
    let mut nonce = [0u8; 12];
    hk.expand(b"Content-Encoding: aes128gcm\x01", &mut cek)
        .map_err(|e| anyhow::anyhow!("hkdf cek: {e}"))?;
    hk.expand(b"Content-Encoding: nonce\x01", &mut nonce)
        .map_err(|e| anyhow::anyhow!("hkdf nonce: {e}"))?;

    let gcm = Aes128Gcm::new_from_slice(&cek)?;
    // aes128gcm 内容层：plaintext || 0x02（最后块分隔符）
    let mut pt = plaintext.as_bytes().to_vec();
    pt.push(2u8);
    let cipher = gcm
        .encrypt(
            nonce.as_ref().into(),
            Payload {
                msg: &pt,
                aad: b"", // salt/cid 头不启用时 AAD 为空，头字段全零 rs=4096
            },
        )
        .map_err(|e| anyhow::anyhow!("aes-gcm: {e:?}"))?;

    // 组装二进制头：salt(16) | rs(4, BE 4096) | idlen(1=65) | server_pub(65) || ciphertext
    let mut out = Vec::with_capacity(16 + 4 + 1 + 65 + cipher.len());
    let salt: [u8; 16] = rand::random();
    out.extend_from_slice(&salt);
    out.extend_from_slice(&4096u32.to_be_bytes());
    out.push(65u8);
    // 非压缩点共 65B（0x04 || X || Y），原样作为 key id 写入
    out.extend_from_slice(server_pub_bytes.as_bytes());
    out.extend_from_slice(&cipher);
    Ok(out)
}
