//! Client-owned record encryption and authenticated ordered history.
use crate::cloud_crypto::{self as crypto, Secret, INVALID};
use aes_gcm::{
    aead::{Aead, Payload},
    Aes256Gcm, KeyInit, Nonce,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

pub const ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";
pub const UNSAFE: &str = "Cloud history could not be verified. Sync stopped to protect your data.";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Head {
    pub sequence: String,
    pub hash: String,
}
impl Default for Head {
    fn default() -> Self {
        Self {
            sequence: "0".into(),
            hash: ZERO.into(),
        }
    }
}
impl Head {
    pub fn number(&self) -> Result<i64, String> {
        if self.hash.len() != 64
            || !self
                .hash
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err(UNSAFE.into());
        }
        number(&self.sequence)
    }
}
pub fn number(value: &str) -> Result<i64, String> {
    let n = value.parse::<i64>().map_err(|_| UNSAFE)?;
    if n < 0 || n.to_string() != value {
        return Err(UNSAFE.into());
    }
    Ok(n)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Envelope {
    pub format: String,
    pub algorithm: String,
    pub key_id: String,
    pub salt: String,
    pub nonce: String,
    pub ciphertext: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Mutation {
    pub protocol: u8,
    pub dataset_id: String,
    pub mutation_id: String,
    pub record_id: String,
    pub base_version: String,
    pub previous: Head,
    pub operation: String,
    pub payload: Envelope,
    pub commit_hash: String,
    pub commit_mac: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub sequence: String,
    pub version: String,
    pub request: Mutation,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Page {
    pub dataset_id: String,
    pub head: Head,
    pub boundary: String,
    pub changes: Vec<Change>,
    pub cursor: String,
    pub has_more: bool,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PushResult {
    Accepted { change: Change },
    Conflict { head: Head, current: Option<Change> },
}

fn json(value: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&value).expect("JSON string array")
}
fn derive(root: &Secret, salt: Option<&[u8]>, info: &[u8]) -> Result<Secret, String> {
    let mut key = Zeroizing::new([0; 32]);
    Hkdf::<Sha256>::new(salt, &**root)
        .expand(info, &mut *key)
        .map_err(|_| INVALID)?;
    Ok(key)
}
fn mac_key(root: &Secret, dataset: &str, key_id: &str) -> Result<Secret, String> {
    derive(
        root,
        None,
        &json(serde_json::json!(["heyday-sync-mac-v1", dataset, key_id])),
    )
}
pub fn wire_id(root: &Secret, dataset: &str, key_id: &str, id: &str) -> Result<String, String> {
    let key = derive(
        root,
        None,
        &json(serde_json::json!(["heyday-record-id-v1", dataset, key_id])),
    )?;
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&*key).map_err(|_| INVALID)?;
    let (entity, id) = id.split_once(':').unwrap_or(("category", id));
    mac.update(&json(serde_json::json!([entity, id])));
    Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
}
impl Mutation {
    fn commit_data(&self) -> Result<Vec<u8>, String> {
        let next = self
            .previous
            .number()?
            .checked_add(1)
            .ok_or(UNSAFE)?
            .to_string();
        Ok(json(serde_json::json!([
            match self.protocol {
                1 => "heyday-sync-v1",
                2 => "heyday-sync-v2",
                3 => "heyday-sync-v3",
                4 => "heyday-sync-v4",
                _ => "heyday-sync-v5",
            },
            self.dataset_id,
            next,
            self.previous.hash,
            self.record_id,
            self.mutation_id,
            self.base_version,
            self.operation,
            self.payload.key_id,
            self.payload.salt,
            self.payload.nonce,
            self.payload.ciphertext
        ])))
    }
    fn aad(&self) -> Vec<u8> {
        json(serde_json::json!([
            "heyday-record-v2",
            "A256GCM-HKDF-SHA256",
            self.dataset_id,
            self.record_id,
            self.mutation_id,
            self.base_version,
            self.operation,
            self.payload.key_id,
            self.payload.salt
        ]))
    }
    fn record_key(&self, root: &Secret) -> Result<Secret, String> {
        let salt = crypto::decode(&self.payload.salt, 32, 32)?;
        derive(
            root,
            Some(&salt),
            &json(serde_json::json!([
                "heyday-record-key-v2",
                self.dataset_id,
                self.record_id,
                self.mutation_id,
                self.payload.key_id
            ])),
        )
    }
    fn validate(&self) -> Result<(), String> {
        if !matches!(self.protocol, 1 | 2 | 3 | 4 | 5)
            || self.payload.format != "heyday-record-v2"
            || self.payload.algorithm != "A256GCM-HKDF-SHA256"
            || !matches!(self.operation.as_str(), "put" | "delete")
        {
            return Err(UNSAFE.into());
        }
        for id in [
            &self.dataset_id,
            &self.mutation_id,
            &self.record_id,
            &self.payload.key_id,
        ] {
            if id.is_empty()
                || id.len() > 128
                || !id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
            {
                return Err(UNSAFE.into());
            }
        }
        number(&self.base_version)?;
        crypto::decode(&self.record_id, 32, 32)?;
        crypto::decode(&self.payload.salt, 32, 32)?;
        crypto::decode(&self.payload.nonce, 12, 12)?;
        crypto::decode(
            &self.payload.ciphertext,
            16,
            ciphertext_limit(self.protocol),
        )?;
        Ok(())
    }
    pub fn decrypt(
        &self,
        root: &Secret,
        dataset: &str,
        key_id: &str,
    ) -> Result<Zeroizing<Vec<u8>>, String> {
        self.validate()?;
        if self.dataset_id != dataset || self.payload.key_id != key_id {
            return Err(UNSAFE.into());
        }
        let digest = Sha256::digest(self.commit_data()?);
        if format!("{digest:x}") != self.commit_hash {
            return Err(UNSAFE.into());
        }
        let key = mac_key(root, dataset, key_id)?;
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&*key).map_err(|_| INVALID)?;
        mac.update(&digest);
        mac.verify_slice(&crypto::decode(&self.commit_mac, 32, 32)?)
            .map_err(|_| UNSAFE)?;
        let key = self.record_key(root)?;
        let cipher = Aes256Gcm::new_from_slice(&*key).map_err(|_| INVALID)?;
        let nonce = crypto::decode(&self.payload.nonce, 12, 12)?;
        let ciphertext = crypto::decode(
            &self.payload.ciphertext,
            16,
            ciphertext_limit(self.protocol),
        )?;
        Ok(Zeroizing::new(
            cipher
                .decrypt(
                    Nonce::from_slice(&nonce),
                    Payload {
                        msg: &ciphertext,
                        aad: &self.aad(),
                    },
                )
                .map_err(|_| UNSAFE)?,
        ))
    }
}
#[cfg(test)]
pub fn encrypt(
    root: &Secret,
    dataset: &str,
    key_id: &str,
    local_id: &str,
    base_version: i64,
    previous: &Head,
    deleted: bool,
    plaintext: &[u8],
) -> Result<Mutation, String> {
    encrypt_with_protocol(
        root,
        dataset,
        key_id,
        local_id,
        base_version,
        previous,
        deleted,
        plaintext,
        if local_id.contains(':') { 2 } else { 1 },
    )
}
fn ciphertext_limit(protocol: u8) -> usize {
    match protocol {
        1 => 8192,
        2 => 262144,
        _ => 2_097_152,
    }
}
#[cfg(test)]
pub fn encrypt_with_protocol(
    root: &Secret,
    dataset: &str,
    key_id: &str,
    local_id: &str,
    base_version: i64,
    previous: &Head,
    deleted: bool,
    plaintext: &[u8],
    protocol: u8,
) -> Result<Mutation, String> {
    encrypt_with_identity(
        root,
        dataset,
        key_id,
        root,
        key_id,
        local_id,
        base_version,
        previous,
        deleted,
        plaintext,
        protocol,
    )
}
pub fn encrypt_with_identity(
    root: &Secret,
    dataset: &str,
    key_id: &str,
    identity: &Secret,
    identity_id: &str,
    local_id: &str,
    base_version: i64,
    previous: &Head,
    deleted: bool,
    plaintext: &[u8],
    protocol: u8,
) -> Result<Mutation, String> {
    if !matches!(protocol, 1 | 2 | 3 | 4 | 5)
        || plaintext.len() > ciphertext_limit(protocol) - 16
        || base_version < 0
    {
        return Err(INVALID.into());
    }
    let mut m = Mutation {
        protocol,
        dataset_id: dataset.into(),
        mutation_id: URL_SAFE_NO_PAD.encode(crypto::random::<32>()?),
        record_id: wire_id(identity, dataset, identity_id, local_id)?,
        base_version: base_version.to_string(),
        previous: previous.clone(),
        operation: if deleted { "delete" } else { "put" }.into(),
        payload: Envelope {
            format: "heyday-record-v2".into(),
            algorithm: "A256GCM-HKDF-SHA256".into(),
            key_id: key_id.into(),
            salt: URL_SAFE_NO_PAD.encode(crypto::random::<32>()?),
            nonce: URL_SAFE_NO_PAD.encode(crypto::random::<12>()?),
            ciphertext: String::new(),
        },
        commit_hash: String::new(),
        commit_mac: String::new(),
    };
    let key = m.record_key(root)?;
    let nonce = crypto::decode(&m.payload.nonce, 12, 12)?;
    m.payload.ciphertext = URL_SAFE_NO_PAD.encode(
        Aes256Gcm::new_from_slice(&*key)
            .map_err(|_| INVALID)?
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: &m.aad(),
                },
            )
            .map_err(|_| INVALID)?,
    );
    let digest = Sha256::digest(m.commit_data()?);
    m.commit_hash = format!("{digest:x}");
    let key = mac_key(root, dataset, key_id)?;
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&*key).map_err(|_| INVALID)?;
    mac.update(&digest);
    m.commit_mac = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
    Ok(m)
}
pub fn verify_change(change: &Change, previous: &Head) -> Result<Head, String> {
    if &change.request.previous != previous
        || number(&change.sequence)? != previous.number()?.checked_add(1).ok_or(UNSAFE)?
        || number(&change.version)?
            != number(&change.request.base_version)?
                .checked_add(1)
                .ok_or(UNSAFE)?
    {
        return Err(UNSAFE.into());
    }
    Ok(Head {
        sequence: change.sequence.clone(),
        hash: change.request.commit_hash.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protocol_three_matches_node_fixture_and_rejects_version_downgrade() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/sync-v3.json")).unwrap();
        let root = crypto::secret(fixture["rootKey"].as_str().unwrap()).unwrap();
        let mut change: Change = serde_json::from_value(fixture["change"].clone()).unwrap();
        let m = &change.request;
        let record = crate::cloud_sync_store::Record::decode(
            &change,
            &root,
            &m.dataset_id,
            &m.payload.key_id,
        )
        .unwrap();
        assert_eq!(record.key(), fixture["localId"].as_str().unwrap());
        assert_eq!(
            &*m.decrypt(&root, &m.dataset_id, &m.payload.key_id).unwrap(),
            fixture["plaintext"].as_str().unwrap().as_bytes()
        );
        change.request.protocol = 2;
        assert!(change
            .request
            .decrypt(
                &root,
                &change.request.dataset_id,
                &change.request.payload.key_id
            )
            .is_err());
    }
    #[test]
    fn protocol_four_matches_node_fixture_and_rejects_version_downgrade() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/sync-v4.json")).unwrap();
        let root = crypto::secret(fixture["rootKey"].as_str().unwrap()).unwrap();
        let mut change: Change = serde_json::from_value(fixture["change"].clone()).unwrap();
        let m = &change.request;
        let record = crate::cloud_sync_store::Record::decode(
            &change,
            &root,
            &m.dataset_id,
            &m.payload.key_id,
        )
        .unwrap();
        assert_eq!(
            record.key(),
            format!("income-plan:{}", fixture["localId"].as_str().unwrap())
        );
        assert_eq!(
            &*m.decrypt(&root, &m.dataset_id, &m.payload.key_id).unwrap(),
            fixture["plaintext"].as_str().unwrap().as_bytes()
        );
        change.request.protocol = 3;
        assert!(change
            .request
            .decrypt(
                &root,
                &change.request.dataset_id,
                &change.request.payload.key_id
            )
            .is_err());
    }
    #[test]
    fn protocol_five_matches_node_fixture_and_rejects_version_downgrade() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/sync-v5.json")).unwrap();
        let root = crypto::secret(fixture["rootKey"].as_str().unwrap()).unwrap();
        let mut change: Change = serde_json::from_value(fixture["change"].clone()).unwrap();
        let m = &change.request;
        let record = crate::cloud_sync_store::Record::decode(
            &change,
            &root,
            &m.dataset_id,
            &m.payload.key_id,
        )
        .unwrap();
        assert_eq!(
            record.key(),
            format!("metadata:{}", fixture["localId"].as_str().unwrap())
        );
        assert_eq!(
            &*m.decrypt(&root, &m.dataset_id, &m.payload.key_id).unwrap(),
            fixture["plaintext"].as_str().unwrap().as_bytes()
        );
        change.request.protocol = 4;
        assert!(change
            .request
            .decrypt(
                &root,
                &change.request.dataset_id,
                &change.request.payload.key_id
            )
            .is_err());
    }
    #[test]
    fn independent_web_crypto_vector_matches_keys_aad_mac_and_opaque_ids() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/sync-v1.json")).unwrap();
        let root = crypto::secret(fixture["rootKey"].as_str().unwrap()).unwrap();
        let change: Change = serde_json::from_value(fixture["change"].clone()).unwrap();
        let m = &change.request;
        assert_eq!(
            wire_id(
                &root,
                &m.dataset_id,
                &m.payload.key_id,
                fixture["localId"].as_str().unwrap()
            )
            .unwrap(),
            m.record_id
        );
        assert_eq!(
            std::str::from_utf8(&m.decrypt(&root, &m.dataset_id, &m.payload.key_id).unwrap())
                .unwrap(),
            fixture["plaintext"].as_str().unwrap()
        );
        assert_eq!(
            verify_change(&change, &Head::default()).unwrap().hash,
            m.commit_hash
        );
        for path in [
            "datasetId",
            "recordId",
            "mutationId",
            "baseVersion",
            "operation",
            "commitHash",
            "commitMac",
        ] {
            let mut value = fixture["change"]["request"].clone();
            value[path] = serde_json::Value::String("changed".into());
            let changed: Mutation = serde_json::from_value(value).unwrap();
            assert!(
                changed
                    .decrypt(&root, &m.dataset_id, &m.payload.key_id)
                    .is_err(),
                "{path}"
            );
        }
        let mut changed = change.clone();
        changed.sequence = "2".into();
        assert!(verify_change(&changed, &Head::default()).is_err());
        changed = change.clone();
        changed.version = "2".into();
        assert!(verify_change(&changed, &Head::default()).is_err());
        changed = change.clone();
        changed.request.previous.hash = "1".repeat(64);
        assert!(verify_change(&changed, &Head::default()).is_err());
    }
    #[test]
    fn every_encryption_uses_a_separate_key_and_round_trips_with_bounds() {
        let root = Zeroizing::new([7; 32]);
        let h = Head::default();
        let first = encrypt(
            &root,
            "dataset",
            "key",
            "record",
            0,
            &h,
            false,
            b"private data",
        )
        .unwrap();
        let second = encrypt(
            &root,
            "dataset",
            "key",
            "record",
            0,
            &h,
            false,
            b"private data",
        )
        .unwrap();
        assert_ne!(first.payload.salt, second.payload.salt);
        assert_ne!(first.mutation_id, second.mutation_id);
        assert_ne!(
            *first.record_key(&root).unwrap(),
            *second.record_key(&root).unwrap()
        );
        assert_eq!(
            &*first.decrypt(&root, "dataset", "key").unwrap(),
            b"private data"
        );
        assert!(first
            .decrypt(&Zeroizing::new([8; 32]), "dataset", "key")
            .is_err());
        assert!(encrypt(
            &root,
            "dataset",
            "key",
            "record",
            0,
            &h,
            false,
            &vec![0; 8177]
        )
        .is_err());
    }
}
