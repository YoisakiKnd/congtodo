//! 浏览器原生推送（Web Push）：RFC 8291 aes128gcm 加密 + RFC 8292 VAPID 签名。
//! 纯 Rust 实现（RustCrypto），不依赖 OpenSSL，方便多架构构建与 distroless 镜像。

use aes_gcm::{
    Aes128Gcm, KeyInit,
    aead::{Aead, Payload},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use hkdf::Hkdf;
use p256::{
    PublicKey, SecretKey,
    ecdsa::{Signature, SigningKey, signature::Signer},
};
use reqwest::{Client, RequestBuilder};
use rusqlite::Connection;
use serde_json::Value;
use sha2::Sha256;

use crate::db::get_server_setting;

/// 服务端 VAPID 密钥对（首次使用时生成并保存在 server_settings）
pub struct Vapid {
    secret: SecretKey,
}

impl Vapid {
    pub fn load_or_create(conn: &Connection) -> Result<Self, String> {
        if let Some(s) = get_server_setting(conn, "vapid_private") {
            let bytes = B64.decode(s).map_err(|e| e.to_string())?;
            return Ok(Vapid { secret: SecretKey::from_slice(&bytes).map_err(|e| e.to_string())? });
        }
        let secret = random_secret()?;
        conn.execute(
            "INSERT OR REPLACE INTO server_settings (key, value) VALUES ('vapid_private', ?)",
            [B64.encode(secret.to_bytes())],
        )
        .map_err(|e| e.to_string())?;
        Ok(Vapid { secret })
    }

    /// 浏览器订阅时使用的 applicationServerKey（未压缩公钥，base64url）
    pub fn public_key(&self) -> String {
        B64.encode(self.secret.public_key().to_sec1_bytes())
    }

    /// `Authorization: vapid t=<JWT>, k=<公钥>`
    fn authorization(&self, endpoint: &str) -> Result<String, String> {
        let url = reqwest::Url::parse(endpoint).map_err(|e| e.to_string())?;
        let aud = url.origin().ascii_serialization();
        let sub = std::env::var("CONGTODO_VAPID_SUBJECT").unwrap_or_else(|_| "mailto:congtodo@localhost".into());
        let exp = chrono::Utc::now().timestamp() + 12 * 3600;
        let header = B64.encode(br#"{"typ":"JWT","alg":"ES256"}"#);
        let claims = B64.encode(serde_json::json!({ "aud": aud, "exp": exp, "sub": sub }).to_string());
        let signing_input = format!("{header}.{claims}");
        let sig: Signature = SigningKey::from(&self.secret).sign(signing_input.as_bytes());
        Ok(format!("vapid t={signing_input}.{}, k={}", B64.encode(sig.to_bytes()), self.public_key()))
    }
}

fn random_secret() -> Result<SecretKey, String> {
    loop {
        let mut buf = [0u8; 32];
        getrandom::fill(&mut buf).map_err(|e| e.to_string())?;
        if let Ok(k) = SecretKey::from_slice(&buf) {
            return Ok(k);
        }
    }
}

fn hkdf(salt: &[u8], ikm: &[u8], info: &[u8], len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len];
    Hkdf::<Sha256>::new(Some(salt), ikm).expand(info, &mut out).expect("valid length");
    out
}

/// RFC 8291：用浏览器订阅的 p256dh / auth 加密消息体
fn encrypt(ua_public: &[u8], auth_secret: &[u8], payload: &[u8]) -> Result<Vec<u8>, String> {
    let ua = PublicKey::from_sec1_bytes(ua_public).map_err(|_| "p256dh 无效".to_string())?;
    let as_secret = random_secret()?;
    let as_public = as_secret.public_key().to_sec1_bytes();
    let shared = p256::ecdh::diffie_hellman(as_secret.to_nonzero_scalar(), ua.as_affine());

    let mut key_info = b"WebPush: info\0".to_vec();
    key_info.extend_from_slice(ua_public);
    key_info.extend_from_slice(&as_public);
    let ikm = hkdf(auth_secret, shared.raw_secret_bytes(), &key_info, 32);

    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).map_err(|e| e.to_string())?;
    let cek = hkdf(&salt, &ikm, b"Content-Encoding: aes128gcm\0", 16);
    let nonce = hkdf(&salt, &ikm, b"Content-Encoding: nonce\0", 12);

    // 单条记录：明文后加 0x02 分隔符
    let mut plain = payload.to_vec();
    plain.push(2);
    let cipher = Aes128Gcm::new_from_slice(&cek).map_err(|e| e.to_string())?;
    let nonce: [u8; 12] = nonce.try_into().unwrap();
    let ct = cipher
        .encrypt(&nonce.into(), Payload { msg: &plain, aad: b"" })
        .map_err(|e| e.to_string())?;

    let mut body = Vec::with_capacity(16 + 4 + 1 + as_public.len() + ct.len());
    body.extend_from_slice(&salt);
    body.extend_from_slice(&4096u32.to_be_bytes());
    body.push(as_public.len() as u8);
    body.extend_from_slice(&as_public);
    body.extend_from_slice(&ct);
    Ok(body)
}

/// 构造推送请求。cfg：{ endpoint, p256dh, auth }
pub fn request(http: &Client, vapid: &Vapid, cfg: &Value, payload: &Value) -> Result<RequestBuilder, String> {
    let s = |k: &str| cfg.get(k).and_then(Value::as_str).ok_or(format!("webpush: 缺少 {k}"));
    let endpoint = s("endpoint")?;
    let p256dh = B64.decode(s("p256dh")?).map_err(|e| e.to_string())?;
    let auth = B64.decode(s("auth")?).map_err(|e| e.to_string())?;
    let body = encrypt(&p256dh, &auth, payload.to_string().as_bytes())?;
    Ok(http
        .post(endpoint)
        .header("authorization", vapid.authorization(endpoint)?)
        .header("content-encoding", "aes128gcm")
        .header("content-type", "application/octet-stream")
        .header("ttl", "86400")
        .header("urgency", "high")
        .body(body))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 按 RFC 8291 用接收方私钥解密，验证加密结果可被浏览器正确解出
    fn decrypt(ua_secret: &SecretKey, auth: &[u8], body: &[u8]) -> Vec<u8> {
        let salt = &body[..16];
        let idlen = body[20] as usize;
        let as_public = &body[21..21 + idlen];
        let ct = &body[21 + idlen..];
        let ua_public = ua_secret.public_key().to_sec1_bytes();
        let shared = p256::ecdh::diffie_hellman(
            ua_secret.to_nonzero_scalar(),
            PublicKey::from_sec1_bytes(as_public).unwrap().as_affine(),
        );
        let mut key_info = b"WebPush: info\0".to_vec();
        key_info.extend_from_slice(&ua_public);
        key_info.extend_from_slice(as_public);
        let ikm = hkdf(auth, shared.raw_secret_bytes(), &key_info, 32);
        let cek = hkdf(salt, &ikm, b"Content-Encoding: aes128gcm\0", 16);
        let nonce: [u8; 12] = hkdf(salt, &ikm, b"Content-Encoding: nonce\0", 12).try_into().unwrap();
        let mut plain = Aes128Gcm::new_from_slice(&cek).unwrap().decrypt(&nonce.into(), ct).unwrap();
        assert_eq!(plain.pop(), Some(2));
        plain
    }

    #[test]
    fn encrypt_roundtrip() {
        let ua = random_secret().unwrap();
        let auth = [7u8; 16];
        let body = encrypt(&ua.public_key().to_sec1_bytes(), &auth, b"{\"title\":\"hi\"}").unwrap();
        assert_eq!(u32::from_be_bytes(body[16..20].try_into().unwrap()), 4096);
        assert_eq!(decrypt(&ua, &auth, &body), b"{\"title\":\"hi\"}");
    }

    #[test]
    fn vapid_jwt_verifies() {
        use p256::ecdsa::{VerifyingKey, signature::Verifier};
        let v = Vapid { secret: random_secret().unwrap() };
        let auth = v.authorization("https://fcm.googleapis.com/fcm/send/abc").unwrap();
        let jwt = auth.strip_prefix("vapid t=").unwrap().split(", k=").next().unwrap();
        let (input, sig) = jwt.rsplit_once('.').unwrap();
        let claims: Value = serde_json::from_slice(&B64.decode(input.split('.').nth(1).unwrap()).unwrap()).unwrap();
        assert_eq!(claims["aud"], "https://fcm.googleapis.com");
        let sig = Signature::from_slice(&B64.decode(sig).unwrap()).unwrap();
        assert!(VerifyingKey::from(SigningKey::from(&v.secret)).verify(input.as_bytes(), &sig).is_ok());
        assert_eq!(B64.decode(v.public_key()).unwrap().len(), 65);
    }
}
