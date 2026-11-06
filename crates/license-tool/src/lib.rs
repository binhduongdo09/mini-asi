//! Khoá bản quyền Mini-Asi Pro — xác thực OFFLINE bằng chữ ký ed25519.
//!
//! Định dạng: `MASI1.<payload base64url>.<chữ ký base64url>`
//! App chỉ chứa KHOÁ CÔNG KHAI → không ai tự tạo được khoá hợp lệ nếu không có khoá bí mật.

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct License {
    /// Email người mua
    pub e: String,
    /// Gói: "pro"
    pub p: String,
    /// Thời điểm cấp (unix giây)
    pub i: u64,
    /// Hết hạn (unix giây) — None = vĩnh viễn
    #[serde(default)]
    pub x: Option<u64>,
}

#[derive(Debug, PartialEq)]
pub enum LicenseError {
    Format,
    Signature,
    Expired,
}

impl std::fmt::Display for LicenseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            LicenseError::Format => "Khoá không đúng định dạng",
            LicenseError::Signature => "Khoá không hợp lệ",
            LicenseError::Expired => "Khoá đã hết hạn",
        })
    }
}

pub fn verify(key: &str, public_key_b64: &str, now_unix: u64) -> Result<License, LicenseError> {
    let key = key.trim();
    let mut parts = key.split('.');
    let (Some("MASI1"), Some(p), Some(s), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return Err(LicenseError::Format);
    };
    let payload = B64.decode(p).map_err(|_| LicenseError::Format)?;
    let sig_bytes = B64.decode(s).map_err(|_| LicenseError::Format)?;
    let sig = Signature::from_slice(&sig_bytes).map_err(|_| LicenseError::Format)?;
    let pk_bytes: [u8; 32] = B64
        .decode(public_key_b64)
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or(LicenseError::Format)?;
    let vk = VerifyingKey::from_bytes(&pk_bytes).map_err(|_| LicenseError::Format)?;
    vk.verify(&payload, &sig).map_err(|_| LicenseError::Signature)?;
    let lic: License = serde_json::from_slice(&payload).map_err(|_| LicenseError::Format)?;
    if let Some(x) = lic.x {
        if now_unix > x {
            return Err(LicenseError::Expired);
        }
    }
    Ok(lic)
}

pub fn sign(lic: &License, secret_key_b64: &str) -> Result<String, String> {
    let sk_bytes: [u8; 32] = B64
        .decode(secret_key_b64.trim())
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or("khoá bí mật không hợp lệ")?;
    let sk = SigningKey::from_bytes(&sk_bytes);
    let payload = serde_json::to_vec(lic).map_err(|e| e.to_string())?;
    let sig = sk.sign(&payload);
    Ok(format!("MASI1.{}.{}", B64.encode(&payload), B64.encode(sig.to_bytes())))
}

/// Tạo cặp khoá mới → (bí mật, công khai) dạng base64url.
pub fn new_keypair() -> (String, String) {
    let sk = SigningKey::generate(&mut rand_core::OsRng);
    (B64.encode(sk.to_bytes()), B64.encode(sk.verifying_key().to_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_tamper() {
        let (sk, pk) = new_keypair();
        let lic = License { e: "a@b.vn".into(), p: "pro".into(), i: 1, x: None };
        let key = sign(&lic, &sk).unwrap();
        assert_eq!(verify(&key, &pk, 100).unwrap(), lic);
        // sửa payload → chữ ký sai
        let forged = sign(&License { e: "hacker".into(), ..lic.clone() }, &new_keypair().0).unwrap();
        assert_eq!(verify(&forged, &pk, 100), Err(LicenseError::Signature));
        assert_eq!(verify("abc", &pk, 0), Err(LicenseError::Format));
    }

    #[test]
    fn expiry() {
        let (sk, pk) = new_keypair();
        let key = sign(&License { e: "a".into(), p: "pro".into(), i: 1, x: Some(50) }, &sk).unwrap();
        assert!(verify(&key, &pk, 10).is_ok());
        assert_eq!(verify(&key, &pk, 51), Err(LicenseError::Expired));
    }
}
