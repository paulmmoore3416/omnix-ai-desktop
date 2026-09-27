//! Offline license verification (Lifetime Pro, BYOK, Enterprise).
//!
//! A license key is one line, `OMNIX1.<payload>.<signature>`: base64url
//! JSON signed with Ed25519 by the vendor's key (`scripts/omnix-license.py`
//! issues them; the private key lives only in the vendor's OS keyring).
//! Verification is entirely local against [`TRUSTED_KEYS`]; nothing is sent
//! anywhere, so a license works air-gapped and `local_only` is unaffected.
//!
//! **Informational for now.** The tier is shown in Settings and recorded,
//! but no feature is locked behind it ([`LicenseStatus::enforced`] is always
//! `false`). A missing, invalid or expired key means the Community tier,
//! which is everything the app does today.
//!
//! The key is stored at `<config_dir>/omnix/license.key` (mode 600). It is
//! not a credential for any service, but it does carry the licensee's name
//! and e-mail, so it is kept private and never logged in full.

use crate::error::{AppError, AppResult};
use base64::Engine;
use chrono::NaiveDate;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Vendor public keys, `(key id, base64 raw 32-byte key)`. Add a line to
/// rotate keys; never remove one while licenses signed with it are in use.
pub const TRUSTED_KEYS: &[(&str, &str)] = &[("k1", "WQbkeq2yIQijx52Tk/rW7/ZUnuRUO9Ah/FYdkSyo+D8=")];

const PREFIX: &str = "OMNIX1";
/// Domain separation: the signed bytes are `DOMAIN || payload part`, so a
/// signature made for anything else cannot be replayed as a license.
const DOMAIN: &[u8] = b"omnix-license-v1.";
/// Longest key accepted (payloads are a few hundred bytes).
const MAX_KEY_LEN: usize = 4096;

/// License tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// No (valid) license: the free, fully local app.
    Community,
    /// One-time purchase for local power users.
    Pro,
    /// Monthly subscription for cloud models with the user's own API keys.
    Byok,
    /// Air-gapped installs with custom support.
    Enterprise,
}

impl Tier {
    /// What the tier covers, for display. Informational: nothing is gated.
    pub fn entitlements(self) -> &'static [&'static str] {
        match self {
            Tier::Community => &[
                "Local models, memory, voice and system control",
                "Community support",
            ],
            Tier::Pro => &[
                "Everything in Community, for life",
                "Advanced GPU monitoring and history",
                "Unlimited local memory",
                "Priority e-mail support",
            ],
            Tier::Byok => &[
                "Everything in Pro while the subscription is active",
                "Cloud providers (OpenAI, Anthropic) with your own API keys",
            ],
            Tier::Enterprise => &[
                "Everything in Pro and BYOK",
                "Air-gapped installation and hardening review",
                "Custom support agreement",
            ],
        }
    }
}

/// The signed claims.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LicensePayload {
    /// Format version (1).
    pub v: u32,
    /// License id (UUID).
    pub id: String,
    /// Signing key id (see [`TRUSTED_KEYS`]).
    pub kid: String,
    /// Tier granted.
    pub tier: Tier,
    /// Licensee name.
    pub licensee: String,
    /// Licensee e-mail.
    #[serde(default)]
    pub email: Option<String>,
    /// Issue date (`YYYY-MM-DD`).
    pub issued: NaiveDate,
    /// Last valid day (`YYYY-MM-DD`), or none for a lifetime license.
    #[serde(default)]
    pub expires: Option<NaiveDate>,
    /// Enterprise seat count.
    #[serde(default)]
    pub seats: Option<u32>,
}

/// What Settings shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LicenseStatus {
    /// Effective tier (Community when the key is missing, invalid or expired).
    pub tier: Tier,
    /// Tier on the installed key, even if it has expired.
    pub licensed_tier: Option<Tier>,
    /// Licensee name.
    pub licensee: Option<String>,
    /// Licensee e-mail.
    pub email: Option<String>,
    /// License id.
    pub id: Option<String>,
    /// Issue date.
    pub issued: Option<String>,
    /// Expiry date (none = lifetime).
    pub expires: Option<String>,
    /// True when the installed key has expired.
    pub expired: bool,
    /// Enterprise seats.
    pub seats: Option<u32>,
    /// Why an installed key was not accepted.
    pub error: Option<String>,
    /// What the effective tier covers.
    pub entitlements: Vec<String>,
    /// Always false: tiers are recorded and shown but gate nothing yet.
    pub enforced: bool,
}

impl LicenseStatus {
    fn community(error: Option<String>) -> Self {
        Self {
            tier: Tier::Community,
            licensed_tier: None,
            licensee: None,
            email: None,
            id: None,
            issued: None,
            expires: None,
            expired: false,
            seats: None,
            error,
            entitlements: owned(Tier::Community.entitlements()),
            enforced: false,
        }
    }
}

fn owned(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn b64url(s: &str) -> AppResult<Vec<u8>> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(s)
        .map_err(|_| AppError::InvalidInput("license key is not valid base64url".into()))
}

/// Verify `key` against `trusted` and return its claims. Expiry is not
/// checked here (see [`status_for`]).
pub fn verify(key: &str, trusted: &[(&str, &str)]) -> AppResult<LicensePayload> {
    let key = key.trim();
    if key.len() > MAX_KEY_LEN {
        return Err(AppError::InvalidInput("license key is too long".into()));
    }
    let mut parts = key.split('.');
    let (Some(PREFIX), Some(body), Some(sig), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(AppError::InvalidInput(
            "not an OMNIX license key (expected OMNIX1.….…)".into(),
        ));
    };
    let payload: LicensePayload = serde_json::from_slice(&b64url(body)?)
        .map_err(|e| AppError::InvalidInput(format!("license payload: {e}")))?;
    if payload.v != 1 {
        return Err(AppError::InvalidInput(format!(
            "license format v{} is newer than this app",
            payload.v
        )));
    }
    let (_, pk_b64) = trusted
        .iter()
        .find(|(kid, _)| *kid == payload.kid)
        .ok_or_else(|| AppError::InvalidInput(format!("unknown signing key '{}'", payload.kid)))?;
    let pk: [u8; 32] = base64::engine::general_purpose::STANDARD
        .decode(pk_b64)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| AppError::Internal("malformed trusted key".into()))?;
    let vk = VerifyingKey::from_bytes(&pk)
        .map_err(|_| AppError::Internal("malformed trusted key".into()))?;
    let sig = Signature::from_slice(&b64url(sig)?)
        .map_err(|_| AppError::InvalidInput("malformed license signature".into()))?;
    let mut msg = DOMAIN.to_vec();
    msg.extend_from_slice(body.as_bytes());
    // verify_strict rejects malleable signatures and weak keys.
    vk.verify_strict(&msg, &sig)
        .map_err(|_| AppError::InvalidInput("license signature does not verify".into()))?;
    if payload.tier == Tier::Community {
        return Err(AppError::InvalidInput(
            "a license cannot grant the Community tier".into(),
        ));
    }
    if payload.tier == Tier::Byok && payload.expires.is_none() {
        return Err(AppError::InvalidInput(
            "a BYOK license must have an expiry date".into(),
        ));
    }
    if payload.licensee.trim().is_empty() {
        return Err(AppError::InvalidInput("license has no licensee".into()));
    }
    Ok(payload)
}

/// Status for an installed key (or none) on `today`.
pub fn status_for(key: Option<&str>, trusted: &[(&str, &str)], today: NaiveDate) -> LicenseStatus {
    let Some(key) = key.filter(|k| !k.trim().is_empty()) else {
        return LicenseStatus::community(None);
    };
    match verify(key, trusted) {
        Err(e) => LicenseStatus::community(Some(e.to_string())),
        Ok(p) => {
            let expired = p.expires.is_some_and(|d| today > d);
            let tier = if expired { Tier::Community } else { p.tier };
            LicenseStatus {
                tier,
                licensed_tier: Some(p.tier),
                licensee: Some(p.licensee),
                email: p.email,
                id: Some(p.id),
                issued: Some(p.issued.to_string()),
                expires: p.expires.map(|d| d.to_string()),
                expired,
                seats: p.seats,
                error: None,
                entitlements: owned(tier.entitlements()),
                enforced: false,
            }
        }
    }
}

/// `<config_dir>/omnix/license.key`, next to `settings.json`.
pub fn default_path(settings_path: &Path) -> PathBuf {
    settings_path.with_file_name("license.key")
}

/// Status of the installed key.
pub fn load(path: &Path) -> LicenseStatus {
    let key = std::fs::read_to_string(path).ok();
    status_for(
        key.as_deref(),
        TRUSTED_KEYS,
        chrono::Local::now().date_naive(),
    )
}

/// Verify and install `key`. An invalid key is rejected and the installed
/// one (if any) is left alone.
pub fn install(path: &Path, key: &str) -> AppResult<LicenseStatus> {
    verify(key, TRUSTED_KEYS)?;
    crate::settings::write_atomic(path, &format!("{}\n", key.trim()))?;
    Ok(load(path))
}

/// Remove the installed key (back to Community).
pub fn remove(path: &Path) -> AppResult<LicenseStatus> {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    Ok(load(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::json;

    fn signer() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn trusted_b64(k: &SigningKey) -> String {
        base64::engine::general_purpose::STANDARD.encode(k.verifying_key().to_bytes())
    }

    fn sign(k: &SigningKey, payload: serde_json::Value) -> String {
        let body = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&payload).expect("json"));
        let mut msg = DOMAIN.to_vec();
        msg.extend_from_slice(body.as_bytes());
        let sig = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(k.sign(&msg).to_bytes());
        format!("{PREFIX}.{body}.{sig}")
    }

    fn payload(tier: &str, expires: Option<&str>) -> serde_json::Value {
        json!({"v": 1, "id": "00000000-0000-4000-8000-000000000001", "kid": "t1", "tier": tier,
               "licensee": "Test User", "email": "t@example.com", "issued": "2026-01-01",
               "expires": expires, "seats": null})
    }

    fn day(s: &str) -> NaiveDate {
        s.parse().expect("date")
    }

    #[test]
    fn valid_pro_license_verifies() {
        let k = signer();
        let pk = trusted_b64(&k);
        let trusted = [("t1", pk.as_str())];
        let key = sign(&k, payload("pro", None));
        let s = status_for(Some(&key), &trusted, day("2030-01-01"));
        assert_eq!(s.tier, Tier::Pro);
        assert_eq!(s.licensee.as_deref(), Some("Test User"));
        assert!(!s.expired && s.error.is_none() && !s.enforced);
    }

    #[test]
    fn tampered_payload_is_rejected() {
        let k = signer();
        let pk = trusted_b64(&k);
        let trusted = [("t1", pk.as_str())];
        let key = sign(&k, payload("pro", None));
        let (_, rest) = key.split_once('.').expect("dot");
        let (_, sig) = rest.split_once('.').expect("dot");
        let forged_body = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&payload("enterprise", None)).expect("json"));
        let forged = format!("{PREFIX}.{forged_body}.{sig}");
        assert!(verify(&forged, &trusted).is_err());
        let s = status_for(Some(&forged), &trusted, day("2030-01-01"));
        assert_eq!(s.tier, Tier::Community);
        assert!(s.error.is_some());
    }

    #[test]
    fn other_signing_key_is_rejected() {
        let k = signer();
        let other = SigningKey::from_bytes(&[9u8; 32]);
        let pk = trusted_b64(&other);
        let trusted = [("t1", pk.as_str())];
        assert!(verify(&sign(&k, payload("pro", None)), &trusted).is_err());
        // Unknown key id.
        assert!(verify(&sign(&k, payload("pro", None)), &[]).is_err());
    }

    #[test]
    fn expired_subscription_falls_back_to_community() {
        let k = signer();
        let pk = trusted_b64(&k);
        let trusted = [("t1", pk.as_str())];
        let key = sign(&k, payload("byok", Some("2026-02-01")));
        let on = status_for(Some(&key), &trusted, day("2026-02-01"));
        assert_eq!(on.tier, Tier::Byok);
        let after = status_for(Some(&key), &trusted, day("2026-02-02"));
        assert_eq!(after.tier, Tier::Community);
        assert_eq!(after.licensed_tier, Some(Tier::Byok));
        assert!(after.expired);
    }

    #[test]
    fn byok_without_expiry_and_community_grants_are_rejected() {
        let k = signer();
        let pk = trusted_b64(&k);
        let trusted = [("t1", pk.as_str())];
        assert!(verify(&sign(&k, payload("byok", None)), &trusted).is_err());
        assert!(verify(&sign(&k, payload("community", None)), &trusted).is_err());
    }

    #[test]
    fn garbage_is_rejected_without_panicking() {
        for bad in [
            "",
            "OMNIX1",
            "OMNIX1..",
            "OMNIX2.a.b",
            "OMNIX1.!!.??",
            "OMNIX1.a.b.c",
        ] {
            assert!(verify(bad, TRUSTED_KEYS).is_err(), "{bad}");
        }
        assert!(verify(&"A".repeat(MAX_KEY_LEN + 1), TRUSTED_KEYS).is_err());
        assert_eq!(
            status_for(None, TRUSTED_KEYS, day("2026-01-01")).tier,
            Tier::Community
        );
    }

    #[test]
    fn trusted_keys_are_well_formed() {
        for (_, b64) in TRUSTED_KEYS {
            let raw: [u8; 32] = base64::engine::general_purpose::STANDARD
                .decode(b64)
                .expect("base64")
                .try_into()
                .expect("32 bytes");
            VerifyingKey::from_bytes(&raw).expect("valid point");
        }
    }

    /// Signed by `scripts/omnix-license.py` with the real `k1` key: the
    /// Python issuer and this verifier must agree. It expired on 2026-09-27,
    /// so it grants nothing.
    const INTEROP_FIXTURE: &str = "OMNIX1.eyJlbWFpbCI6bnVsbCwiZXhwaXJlcyI6IjIwMjYtMDktMjciLCJpZCI6ImY2NGI2MjFjLWY0ZjYtNDY2Ni1iZDhiLTcyYTBiZDRkMGRlOSIsImlzc3VlZCI6IjIwMjYtMDktMjYiLCJraWQiOiJrMSIsImxpY2Vuc2VlIjoiSW50ZXJvcCBUZXN0IEZpeHR1cmUiLCJzZWF0cyI6bnVsbCwidGllciI6ImJ5b2siLCJ2IjoxfQ.WIWvyhwL0qfpw2Oh7I4DD7Iad-LP9qWP7SNn0XHl7xQPrhG-SRmOsGWhSwRFipapNaZdAOGOTOG959ryxx6QAg";

    #[test]
    fn python_issued_key_verifies_with_trusted_keys() {
        let p = verify(INTEROP_FIXTURE, TRUSTED_KEYS).expect("verifies");
        assert_eq!(p.tier, Tier::Byok);
        assert_eq!(p.licensee, "Interop Test Fixture");
        let s = status_for(Some(INTEROP_FIXTURE), TRUSTED_KEYS, day("2026-10-01"));
        assert_eq!(s.tier, Tier::Community);
        assert!(s.expired);
    }

    #[test]
    fn install_rejects_invalid_and_keeps_existing() {
        let d = tempfile::tempdir().expect("tempdir");
        let p = d.path().join("license.key");
        std::fs::write(&p, "previous").expect("write");
        assert!(install(&p, "OMNIX1.bad.key").is_err());
        assert_eq!(std::fs::read_to_string(&p).expect("read"), "previous");
        assert_eq!(remove(&p).expect("remove").tier, Tier::Community);
        assert!(!p.exists());
        // Removing twice is fine.
        remove(&p).expect("remove again");
    }
}
