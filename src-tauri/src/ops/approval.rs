//! Signed approvals for unattended commands.
//!
//! When a user creates an automation or scheduled task that runs a command
//! needing confirmation, OMNIX shows a native dialog *once* and stores
//! `HMAC-SHA256(key, rule_id ‖ command ‖ cwd)` with the rule. At run time the
//! engine recomputes the MAC; any change to the command, working directory
//! or rule id, whether made in the UI, by the model or by editing `ops.json`
//! on disk, invalidates it, and the command is refused until re-approved.
//!
//! The 32-byte key lives in the OS keychain under an internal id that no IPC
//! command can read or write (`security::secrets::INTERNAL`).

use crate::error::{AppError, AppResult};
use crate::security::secrets::SecretStore;
use sha2::{Digest, Sha256};

const KEY_ID: &str = "internal.ops_signing";
const BLOCK: usize = 64;

/// HMAC-SHA256 (RFC 2104).
pub fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut k = [0u8; BLOCK];
    if key.len() > BLOCK {
        k[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let inner = Sha256::new()
        .chain_update(ipad)
        .chain_update(msg)
        .finalize();
    Sha256::new()
        .chain_update(opad)
        .chain_update(inner)
        .finalize()
        .into()
}

fn message(rule_id: &str, command: &str, cwd: Option<&str>) -> Vec<u8> {
    // Length-prefixed fields: no ambiguity between (a, bc) and (ab, c).
    let mut m = Vec::new();
    for part in [rule_id, command, cwd.unwrap_or("")] {
        m.extend_from_slice(&(part.len() as u64).to_be_bytes());
        m.extend_from_slice(part.as_bytes());
    }
    m
}

fn key(store: &dyn SecretStore, create: bool) -> AppResult<Vec<u8>> {
    if let Some(k) = store.get(KEY_ID)? {
        return hex::decode(k.trim())
            .map_err(|_| AppError::Secret("approval key is corrupt".into()));
    }
    if !create {
        return Err(AppError::Secret("no approval key yet".into()));
    }
    let mut k = [0u8; 32];
    // uuid v4 draws from the OS CSPRNG (getrandom); two give 244 random bits,
    // hashed into a 256-bit key.
    let seed = format!("{}{}", uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    k.copy_from_slice(&Sha256::digest(seed.as_bytes()));
    store.set(KEY_ID, &hex::encode(k))?;
    Ok(k.to_vec())
}

/// Sign an approval (creates the key on first use).
pub fn sign(
    store: &dyn SecretStore,
    rule_id: &str,
    command: &str,
    cwd: Option<&str>,
) -> AppResult<String> {
    let k = key(store, true)?;
    Ok(hex::encode(hmac_sha256(
        &k,
        &message(rule_id, command, cwd),
    )))
}

/// Constant-time verification.
pub fn verify(
    store: &dyn SecretStore,
    rule_id: &str,
    command: &str,
    cwd: Option<&str>,
    mac: &str,
) -> bool {
    let Ok(k) = key(store, false) else {
        return false;
    };
    let Ok(given) = hex::decode(mac) else {
        return false;
    };
    let want = hmac_sha256(&k, &message(rule_id, command, cwd));
    given.len() == want.len()
        && given
            .iter()
            .zip(want.iter())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::secrets::MemoryStore;

    #[test]
    fn rfc4231_test_case_2() {
        let mac = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(
            hex::encode(mac),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn rfc4231_long_key() {
        // Test case 6: 131-byte key of 0xaa.
        let mac = hmac_sha256(
            &[0xaa; 131],
            b"Test Using Larger Than Block-Size Key - Hash Key First",
        );
        assert_eq!(
            hex::encode(mac),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn any_change_invalidates() {
        let s = MemoryStore::default();
        let mac = sign(&s, "t_1", "rm -rf ~/tmp/x", Some("/home/u")).expect("sign");
        assert!(verify(&s, "t_1", "rm -rf ~/tmp/x", Some("/home/u"), &mac));
        assert!(!verify(&s, "t_1", "rm -rf ~/tmp/y", Some("/home/u"), &mac));
        assert!(!verify(&s, "t_2", "rm -rf ~/tmp/x", Some("/home/u"), &mac));
        assert!(!verify(&s, "t_1", "rm -rf ~/tmp/x", None, &mac));
        assert!(!verify(&s, "t_1", "rm -rf ~/tmp/x", Some("/home/u"), "00"));
        // Field boundaries can't be shifted.
        let a = sign(&s, "t", "ab", Some("c")).expect("sign");
        assert!(!verify(&s, "t", "a", Some("bc"), &a));
        // No key → nothing verifies.
        assert!(!verify(&MemoryStore::default(), "t_1", "x", None, &mac));
    }
}
