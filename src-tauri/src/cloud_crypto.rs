//! Versioned client-side cryptography. No network, key-store or financial DB access.
//! Record helpers stay native-only until a transactional sync queue is implemented.
#![allow(dead_code)]
use aes_gcm::{
    aead::{Aead, Payload},
    Aes256Gcm, KeyInit, Nonce,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

pub type Secret = Zeroizing<[u8; 32]>;
pub const INVALID: &str = "Invalid encrypted data or key package. Access was blocked.";
pub const RECOVERY_ERROR: &str =
    "The recovery key is incorrect or the encrypted key package was changed.";
pub const MAX_CIPHERTEXT: usize = 65_536;

pub fn random<const N: usize>() -> Result<[u8; N], String> {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).map_err(|_| "Could not generate a secure encryption key.")?;
    Ok(bytes)
}

pub fn decode(value: &str, min: usize, max: usize) -> Result<Zeroizing<Vec<u8>>, String> {
    if value.len() > (max * 4).div_ceil(3) {
        return Err(INVALID.into());
    }
    let bytes = Zeroizing::new(URL_SAFE_NO_PAD.decode(value).map_err(|_| INVALID)?);
    if bytes.len() < min || bytes.len() > max || URL_SAFE_NO_PAD.encode(&*bytes) != value {
        return Err(INVALID.into());
    }
    Ok(bytes)
}
pub fn secret(value: &str) -> Result<Secret, String> {
    let bytes = decode(value, 32, 32)?;
    let mut key = Zeroizing::new([0; 32]);
    key.copy_from_slice(&bytes);
    Ok(key)
}
fn id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(INVALID.into());
    }
    Ok(())
}
fn array(value: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&value).expect("static JSON array of strings")
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeyEnvelope {
    pub format: String,
    pub algorithm: String,
    pub kdf: String,
    pub dataset_id: String,
    pub key_id: String,
    pub salt: String,
    pub nonce: String,
    pub ciphertext: String,
}

impl KeyEnvelope {
    pub fn validate(&self) -> Result<(), String> {
        if self.format != "heyday-key-v1"
            || self.algorithm != "A256GCM"
            || self.kdf != "HKDF-SHA256"
        {
            return Err(INVALID.into());
        }
        id(&self.dataset_id)?;
        id(&self.key_id)?;
        decode(&self.salt, 32, 32)?;
        decode(&self.nonce, 12, 12)?;
        decode(&self.ciphertext, 48, 48)?;
        Ok(())
    }
    fn aad(&self, account: &str) -> Vec<u8> {
        array(serde_json::json!([
            self.format,
            self.algorithm,
            self.kdf,
            account,
            self.dataset_id,
            self.key_id,
            self.salt
        ]))
    }
    fn wrapping_key(&self, account: &str, recovery: &Secret) -> Result<Secret, String> {
        let salt = decode(&self.salt, 32, 32)?;
        let info = array(serde_json::json!([
            "heyday-recovery-v1",
            account,
            self.dataset_id,
            self.key_id
        ]));
        let mut key = Zeroizing::new([0; 32]);
        Hkdf::<Sha256>::new(Some(&salt), &**recovery)
            .expand(&info, &mut *key)
            .map_err(|_| INVALID)?;
        Ok(key)
    }
}

pub fn recovery_text(recovery: &Secret) -> Zeroizing<String> {
    let checksum = Sha256::digest(&**recovery);
    Zeroizing::new(format!(
        "HDK1.{}.{:02x}{:02x}{:02x}{:02x}",
        URL_SAFE_NO_PAD.encode(&**recovery),
        checksum[0],
        checksum[1],
        checksum[2],
        checksum[3]
    ))
}
pub fn parse_recovery(value: &str) -> Result<Secret, String> {
    if value.len() > 128 {
        return Err(RECOVERY_ERROR.into());
    }
    let text = value.trim();
    let parts: Vec<_> = text.split('.').collect();
    if parts.len() != 3 || parts[0] != "HDK1" {
        return Err(RECOVERY_ERROR.into());
    }
    let key = secret(parts[1]).map_err(|_| RECOVERY_ERROR)?;
    if recovery_text(&key).as_str() != text {
        return Err(RECOVERY_ERROR.into());
    }
    Ok(key)
}

pub fn wrap(
    account: &str,
    dataset: &str,
    key: &Secret,
    recovery: &Secret,
) -> Result<KeyEnvelope, String> {
    id(dataset)?;
    let mut envelope = KeyEnvelope {
        format: "heyday-key-v1".into(),
        algorithm: "A256GCM".into(),
        kdf: "HKDF-SHA256".into(),
        dataset_id: dataset.into(),
        key_id: URL_SAFE_NO_PAD.encode(random::<32>()?),
        salt: URL_SAFE_NO_PAD.encode(random::<32>()?),
        nonce: URL_SAFE_NO_PAD.encode(random::<12>()?),
        ciphertext: String::new(),
    };
    let wrapping = envelope.wrapping_key(account, recovery)?;
    let nonce = decode(&envelope.nonce, 12, 12)?;
    let cipher = Aes256Gcm::new_from_slice(&*wrapping).map_err(|_| INVALID)?;
    envelope.ciphertext = URL_SAFE_NO_PAD.encode(
        cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &**key,
                    aad: &envelope.aad(account),
                },
            )
            .map_err(|_| INVALID)?,
    );
    Ok(envelope)
}
pub fn unwrap(
    account: &str,
    expected_dataset: &str,
    envelope: &KeyEnvelope,
    recovery: &Secret,
) -> Result<Secret, String> {
    envelope.validate()?;
    if envelope.dataset_id != expected_dataset {
        return Err(INVALID.into());
    }
    let wrapping = envelope.wrapping_key(account, recovery)?;
    let nonce = decode(&envelope.nonce, 12, 12)?;
    let ciphertext = decode(&envelope.ciphertext, 48, 48)?;
    let cipher = Aes256Gcm::new_from_slice(&*wrapping).map_err(|_| INVALID)?;
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: &envelope.aad(account),
                },
            )
            .map_err(|_| RECOVERY_ERROR)?,
    );
    if plaintext.len() != 32 {
        return Err(INVALID.into());
    }
    let mut key = Zeroizing::new([0; 32]);
    key.copy_from_slice(&plaintext);
    Ok(key)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EncryptedRecord {
    pub format: String,
    pub algorithm: String,
    pub key_id: String,
    pub nonce: String,
    pub ciphertext: String,
}
pub struct RecordContext<'a> {
    pub dataset_id: &'a str,
    pub record_id: &'a str,
    pub mutation_id: &'a str,
    pub base_version: &'a str,
    pub operation: &'a str,
}
fn record_aad(context: &RecordContext, key_id: &str) -> Result<Vec<u8>, String> {
    for value in [
        context.dataset_id,
        context.record_id,
        context.mutation_id,
        key_id,
    ] {
        id(value)?;
    }
    let version = context.base_version.parse::<u64>().map_err(|_| INVALID)?;
    if version > i64::MAX as u64
        || version.to_string() != context.base_version
        || !matches!(context.operation, "put" | "delete")
    {
        return Err(INVALID.into());
    }
    Ok(array(serde_json::json!([
        "heyday-record-v1",
        "A256GCM",
        context.dataset_id,
        context.record_id,
        context.mutation_id,
        context.base_version,
        context.operation,
        key_id
    ])))
}
pub fn encrypt_record(
    key: &Secret,
    key_id: &str,
    context: &RecordContext,
    plaintext: &[u8],
) -> Result<EncryptedRecord, String> {
    if plaintext.len() > MAX_CIPHERTEXT - 16 {
        return Err(INVALID.into());
    }
    let aad = record_aad(context, key_id)?;
    let nonce = random::<12>()?;
    let cipher = Aes256Gcm::new_from_slice(&**key).map_err(|_| INVALID)?;
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| INVALID)?;
    Ok(EncryptedRecord {
        format: "heyday-record-v1".into(),
        algorithm: "A256GCM".into(),
        key_id: key_id.into(),
        nonce: URL_SAFE_NO_PAD.encode(nonce),
        ciphertext: URL_SAFE_NO_PAD.encode(ciphertext),
    })
}
pub fn decrypt_record(
    key: &Secret,
    expected_key_id: &str,
    context: &RecordContext,
    record: &EncryptedRecord,
) -> Result<Zeroizing<Vec<u8>>, String> {
    if record.format != "heyday-record-v1"
        || record.algorithm != "A256GCM"
        || record.key_id != expected_key_id
    {
        return Err(INVALID.into());
    }
    let aad = record_aad(context, expected_key_id)?;
    let nonce = decode(&record.nonce, 12, 12)?;
    let ciphertext = decode(&record.ciphertext, 16, MAX_CIPHERTEXT)?;
    let cipher = Aes256Gcm::new_from_slice(&**key).map_err(|_| INVALID)?;
    Ok(Zeroizing::new(
        cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| INVALID)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_round_trip_binds_owner_dataset_and_envelope() {
        let key = Zeroizing::new([7; 32]);
        let recovery = Zeroizing::new([9; 32]);
        let envelope = wrap("alice", "dataset-1", &key, &recovery).unwrap();
        assert_eq!(
            *parse_recovery(&recovery_text(&recovery)).unwrap(),
            *recovery
        );
        assert_eq!(
            *unwrap("alice", "dataset-1", &envelope, &recovery).unwrap(),
            *key
        );
        assert!(unwrap("bob", "dataset-1", &envelope, &recovery).is_err());
        assert!(unwrap("alice", "other", &envelope, &recovery).is_err());
        assert!(unwrap("alice", "dataset-1", &envelope, &Zeroizing::new([1; 32])).is_err());
        for field in [
            "keyId",
            "salt",
            "nonce",
            "ciphertext",
            "algorithm",
            "format",
            "kdf",
        ] {
            let mut value = serde_json::to_value(&envelope).unwrap();
            let text = value[field].as_str().unwrap();
            value[field] = serde_json::Value::String(format!(
                "{}{}",
                if text.starts_with('A') { "B" } else { "A" },
                &text[1..]
            ));
            let changed: KeyEnvelope = serde_json::from_value(value).unwrap();
            assert!(
                unwrap("alice", "dataset-1", &changed, &recovery).is_err(),
                "{field}"
            );
        }
    }
    #[test]
    fn records_authenticate_every_context_field_and_enforce_bounds() {
        let key = Zeroizing::new([4; 32]);
        let mut context = RecordContext {
            dataset_id: "dataset",
            record_id: "record",
            mutation_id: "mutation",
            base_version: "0",
            operation: "put",
        };
        let record = encrypt_record(&key, "key", &context, b"private finances").unwrap();
        assert_eq!(
            &*decrypt_record(&key, "key", &context, &record).unwrap(),
            b"private finances"
        );
        assert_ne!(
            encrypt_record(&key, "key", &context, b"private finances")
                .unwrap()
                .nonce,
            record.nonce
        );
        context.dataset_id = "other";
        assert!(decrypt_record(&key, "key", &context, &record).is_err());
        context.dataset_id = "dataset";
        context.record_id = "other";
        assert!(decrypt_record(&key, "key", &context, &record).is_err());
        context.record_id = "record";
        context.mutation_id = "other";
        assert!(decrypt_record(&key, "key", &context, &record).is_err());
        context.mutation_id = "mutation";
        context.base_version = "1";
        assert!(decrypt_record(&key, "key", &context, &record).is_err());
        context.base_version = "0";
        context.operation = "delete";
        assert!(decrypt_record(&key, "key", &context, &record).is_err());
        context.operation = "put";
        assert!(decrypt_record(&key, "other", &context, &record).is_err());
        assert!(encrypt_record(&key, "key", &context, &vec![0; MAX_CIPHERTEXT]).is_err());
        context.base_version = "00";
        assert!(encrypt_record(&key, "key", &context, b"").is_err());
        assert!(parse_recovery("password123").is_err());
    }
    #[test]
    fn aes256_gcm_matches_nist_empty_plaintext_vector() {
        let cipher = Aes256Gcm::new_from_slice(&[0; 32]).unwrap();
        let tag = cipher
            .encrypt(Nonce::from_slice(&[0; 12]), b"".as_ref())
            .unwrap();
        assert_eq!(
            tag.iter().map(|v| format!("{v:02x}")).collect::<String>(),
            "530f8afbc74536b9a963b4f1c4cb738b"
        );
    }

    #[test]
    fn native_crypto_reads_web_crypto_key_and_record_vectors() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/e2ee-v1.json")).unwrap();
        let envelope: KeyEnvelope = serde_json::from_value(fixture["envelope"].clone()).unwrap();
        let recovery = secret(fixture["recoveryKey"].as_str().unwrap()).unwrap();
        let key = unwrap(
            fixture["owner"].as_str().unwrap(),
            &envelope.dataset_id,
            &envelope,
            &recovery,
        )
        .unwrap();
        assert_eq!(
            URL_SAFE_NO_PAD.encode(&*key),
            fixture["dataKey"].as_str().unwrap()
        );
        let ctx = &fixture["context"];
        let context = RecordContext {
            dataset_id: ctx["datasetId"].as_str().unwrap(),
            record_id: ctx["recordId"].as_str().unwrap(),
            mutation_id: ctx["mutationId"].as_str().unwrap(),
            base_version: ctx["baseVersion"].as_str().unwrap(),
            operation: ctx["operation"].as_str().unwrap(),
        };
        let record: EncryptedRecord = serde_json::from_value(fixture["record"].clone()).unwrap();
        let plaintext = decrypt_record(&key, &envelope.key_id, &context, &record).unwrap();
        assert_eq!(
            std::str::from_utf8(&plaintext).unwrap(),
            fixture["plaintext"].as_str().unwrap()
        );
    }
}
