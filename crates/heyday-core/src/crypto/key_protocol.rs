//! Key epochs and independently transferred history checkpoints. No I/O.
use crate::{
    cloud_crypto::{self as crypto, KeyEnvelope, Secret, INVALID},
    cloud_sync_protocol::{Change, Head, UNSAFE},
};
use aes_gcm::{
    aead::{Aead, Payload},
    Aes256Gcm, KeyInit, Nonce,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

pub const CHANGED: &str =
    "Encryption changed on another device. Restore with the latest recovery key.";
pub const HISTORY: &str = "Verify cloud history with a code from a trusted device before syncing.";
const MAX_HISTORY: usize = 131_072;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Archive {
    pub salt: String,
    pub nonce: String,
    pub ciphertext: String,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Rotation {
    pub format: String,
    pub previous_key: KeyEnvelope,
    pub key: KeyEnvelope,
    pub head: Head,
    pub history: Archive,
    pub proof: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoricalKey {
    pub envelope: KeyEnvelope,
    pub key: String,
    pub until: Head,
    pub pin: String,
}
impl Drop for HistoricalKey {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}
pub struct Ring {
    pub key: Secret,
    pub envelope: KeyEnvelope,
    pub history: Vec<HistoricalKey>,
    pin: String,
}
fn bytes(value: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&value).expect("JSON array")
}
fn envelope_data(e: &KeyEnvelope) -> serde_json::Value {
    serde_json::json!([
        e.format,
        e.algorithm,
        e.kdf,
        e.dataset_id,
        e.key_id,
        e.salt,
        e.nonce,
        e.ciphertext
    ])
}
fn derive(root: &Secret, salt: Option<&[u8]>, info: &[u8]) -> Result<Secret, String> {
    let mut key = Zeroizing::new([0; 32]);
    Hkdf::<Sha256>::new(salt, &**root)
        .expand(info, &mut *key)
        .map_err(|_| INVALID)?;
    Ok(key)
}
fn mac(root: &Secret, data: &[u8]) -> Result<Hmac<Sha256>, String> {
    let key = derive(root, None, b"heyday-control-mac-v1")?;
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&*key).map_err(|_| INVALID)?;
    mac.update(data);
    Ok(mac)
}
impl Rotation {
    pub fn validate(&self) -> Result<(), String> {
        self.previous_key.validate()?;
        self.key.validate()?;
        self.head.number()?;
        if self.format != "heyday-rotation-v1"
            || self.key.dataset_id != self.previous_key.dataset_id
            || self.key.key_id == self.previous_key.key_id
        {
            return Err(INVALID.into());
        }
        crypto::decode(&self.history.salt, 32, 32)?;
        crypto::decode(&self.history.nonce, 12, 12)?;
        crypto::decode(&self.history.ciphertext, 16, MAX_HISTORY)?;
        crypto::decode(&self.proof, 32, 32)?;
        Ok(())
    }
    fn data(&self, account: &str) -> Vec<u8> {
        bytes(serde_json::json!([
            self.format,
            account,
            envelope_data(&self.previous_key),
            envelope_data(&self.key),
            self.head.sequence,
            self.head.hash,
            self.history.salt,
            self.history.nonce,
            self.history.ciphertext
        ]))
    }
    pub fn fingerprint(&self, account: &str) -> String {
        format!(
            "{:x}",
            Sha256::digest(bytes(serde_json::json!([
                URL_SAFE_NO_PAD.encode(self.data(account)),
                self.proof
            ])))
        )
    }
    fn archive_key(&self, account: &str, root: &Secret) -> Result<Secret, String> {
        derive(
            root,
            Some(&crypto::decode(&self.history.salt, 32, 32)?),
            &bytes(serde_json::json!([
                "heyday-key-history-v1",
                account,
                self.key.dataset_id,
                self.key.key_id
            ])),
        )
    }
    fn aad(&self, account: &str) -> Vec<u8> {
        bytes(serde_json::json!([
            "heyday-key-history-v1",
            account,
            envelope_data(&self.key)
        ]))
    }
    pub fn create(
        account: &str,
        old: &Ring,
        new_key: &Secret,
        envelope: KeyEnvelope,
        head: &Head,
    ) -> Result<Self, String> {
        if head.number()?
            < old
                .history
                .last()
                .map(|e| e.until.number())
                .transpose()?
                .unwrap_or(0)
        {
            return Err(UNSAFE.into());
        }
        let mut history = old.history.clone();
        history.push(HistoricalKey {
            envelope: old.envelope.clone(),
            key: URL_SAFE_NO_PAD.encode(&*old.key),
            until: head.clone(),
            pin: old.pin.clone(),
        });
        let plaintext = Zeroizing::new(serde_json::to_vec(&history).map_err(|_| INVALID)?);
        if plaintext.len() > MAX_HISTORY - 16 {
            return Err(
                "Encryption key history is too large. Update the app before rotating again.".into(),
            );
        }
        let mut rotation = Self {
            format: "heyday-rotation-v1".into(),
            previous_key: old.envelope.clone(),
            key: envelope,
            head: head.clone(),
            history: Archive {
                salt: URL_SAFE_NO_PAD.encode(crypto::random::<32>()?),
                nonce: URL_SAFE_NO_PAD.encode(crypto::random::<12>()?),
                ciphertext: String::new(),
            },
            proof: String::new(),
        };
        let wrapping = rotation.archive_key(account, new_key)?;
        rotation.history.ciphertext = URL_SAFE_NO_PAD.encode(
            Aes256Gcm::new_from_slice(&*wrapping)
                .map_err(|_| INVALID)?
                .encrypt(
                    Nonce::from_slice(&crypto::decode(&rotation.history.nonce, 12, 12)?),
                    Payload {
                        msg: &plaintext,
                        aad: &rotation.aad(account),
                    },
                )
                .map_err(|_| INVALID)?,
        );
        rotation.proof = URL_SAFE_NO_PAD.encode(
            mac(&old.key, &rotation.data(account))?
                .finalize()
                .into_bytes(),
        );
        rotation.validate()?;
        Ok(rotation)
    }
}
pub fn fingerprint(account: &str, envelope: &KeyEnvelope, rotation: Option<&Rotation>) -> String {
    rotation.map(|r| r.fingerprint(account)).unwrap_or_else(|| {
        format!(
            "{:x}",
            Sha256::digest(bytes(serde_json::json!([
                "heyday-key-pin-v1",
                account,
                envelope_data(envelope)
            ])))
        )
    })
}
impl Ring {
    pub fn open(
        account: &str,
        key: Secret,
        envelope: &KeyEnvelope,
        rotation: Option<&Rotation>,
    ) -> Result<Self, String> {
        envelope.validate()?;
        let history: Vec<HistoricalKey> = if let Some(r) = rotation {
            r.validate()?;
            if &r.key != envelope {
                return Err(INVALID.into());
            }
            let wrapping = r.archive_key(account, &key)?;
            let plain = Zeroizing::new(
                Aes256Gcm::new_from_slice(&*wrapping)
                    .map_err(|_| INVALID)?
                    .decrypt(
                        Nonce::from_slice(&crypto::decode(&r.history.nonce, 12, 12)?),
                        Payload {
                            msg: &crypto::decode(&r.history.ciphertext, 16, MAX_HISTORY)?,
                            aad: &r.aad(account),
                        },
                    )
                    .map_err(|_| INVALID)?,
            );
            let history: Vec<HistoricalKey> =
                serde_json::from_slice(&plain).map_err(|_| INVALID)?;
            let last = history.last().ok_or(INVALID)?;
            if last.envelope != r.previous_key || last.until != r.head {
                return Err(INVALID.into());
            }
            mac(&crypto::secret(&last.key)?, &r.data(account))?
                .verify_slice(&crypto::decode(&r.proof, 32, 32)?)
                .map_err(|_| INVALID)?;
            history
        } else {
            Vec::new()
        };
        let mut ids = std::collections::HashSet::new();
        ids.insert(envelope.key_id.clone());
        let mut previous = Head::default();
        for item in &history {
            item.envelope.validate()?;
            crypto::secret(&item.key)?;
            if item.pin.len() != 64
                || !item
                    .pin
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            {
                return Err(INVALID.into());
            }
            if item.envelope.dataset_id != envelope.dataset_id
                || !ids.insert(item.envelope.key_id.clone())
                || item.until.number()? < previous.number()?
                || (item.until.sequence == previous.sequence && item.until.hash != previous.hash)
            {
                return Err(INVALID.into());
            }
            previous = item.until.clone();
        }
        Ok(Self {
            key,
            envelope: envelope.clone(),
            history,
            pin: fingerprint(account, envelope, rotation),
        })
    }
    pub fn identity(&self) -> Result<(Secret, String), String> {
        match self.history.first() {
            Some(h) => Ok((crypto::secret(&h.key)?, h.envelope.key_id.clone())),
            None => Ok((self.key.clone(), self.envelope.key_id.clone())),
        }
    }
    #[cfg(test)]
    pub fn contains(&self, envelope: &KeyEnvelope) -> bool {
        self.envelope == *envelope || self.history.iter().any(|h| &h.envelope == envelope)
    }
    pub fn matches_pin(
        &self,
        account: &str,
        envelope: &KeyEnvelope,
        rotation_hash: Option<&str>,
    ) -> bool {
        let pin = rotation_hash
            .map(str::to_owned)
            .unwrap_or_else(|| fingerprint(account, envelope, None));
        (self.envelope == *envelope && self.pin == pin)
            || self
                .history
                .iter()
                .any(|h| &h.envelope == envelope && h.pin == pin)
    }
    pub fn decode(&self, change: &Change) -> Result<crate::cloud_sync_store::Record, String> {
        let sequence = crate::cloud_sync_protocol::number(&change.sequence)?;
        let mut start = Head::default();
        let mut selected = None;
        for item in &self.history {
            if sequence > start.number()? && sequence <= item.until.number()? {
                selected = Some((crypto::secret(&item.key)?, item.envelope.key_id.clone()));
            }
            if sequence == item.until.number()? && change.request.commit_hash != item.until.hash {
                return Err(UNSAFE.into());
            }
            if change.request.previous.sequence == item.until.sequence
                && change.request.previous.hash != item.until.hash
            {
                return Err(UNSAFE.into());
            }
            start = item.until.clone();
        }
        let (root, id) =
            selected.unwrap_or_else(|| (self.key.clone(), self.envelope.key_id.clone()));
        if sequence <= start.number()? && !self.history.iter().any(|h| h.envelope.key_id == id) {
            return Err(UNSAFE.into());
        }
        let (identity, identity_id) = self.identity()?;
        crate::cloud_sync_store::Record::decode_epoch(
            change,
            &root,
            &self.envelope.dataset_id,
            &id,
            &identity,
            &identity_id,
        )
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HistoryProof {
    origin: String,
    account: String,
    dataset: String,
    key_pin: String,
    head: Head,
    unverified_history: bool,
    mac: String,
}
impl HistoryProof {
    fn data(&self) -> Vec<u8> {
        bytes(serde_json::json!([
            "heyday-history-proof-v1",
            self.origin,
            self.account,
            self.dataset,
            self.key_pin,
            self.head.sequence,
            self.head.hash,
            self.unverified_history
        ]))
    }
}
pub fn history_code(
    origin: &str,
    account: &str,
    ring: &Ring,
    rotation: Option<&Rotation>,
    head: &Head,
    unverified_history: bool,
) -> Result<String, String> {
    head.number()?;
    let mut proof = HistoryProof {
        origin: origin.into(),
        account: account.into(),
        dataset: ring.envelope.dataset_id.clone(),
        key_pin: fingerprint(account, &ring.envelope, rotation),
        head: head.clone(),
        unverified_history,
        mac: String::new(),
    };
    proof.mac = URL_SAFE_NO_PAD.encode(mac(&ring.key, &proof.data())?.finalize().into_bytes());
    Ok(format!(
        "HDV1.{}",
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&proof).map_err(|_| INVALID)?)
    ))
}
pub fn verify_code(
    code: &str,
    origin: &str,
    account: &str,
    ring: &Ring,
    rotation: Option<&Rotation>,
) -> Result<(Head, bool), String> {
    let proof: HistoryProof = serde_json::from_slice(&crypto::decode(
        code.trim().strip_prefix("HDV1.").ok_or(HISTORY)?,
        1,
        2048,
    )?)
    .map_err(|_| HISTORY)?;
    if proof.origin != origin
        || proof.account != account
        || proof.dataset != ring.envelope.dataset_id
        || proof.key_pin != fingerprint(account, &ring.envelope, rotation)
    {
        return Err(HISTORY.into());
    }
    proof.head.number()?;
    mac(&ring.key, &proof.data())?
        .verify_slice(&crypto::decode(&proof.mac, 32, 32)?)
        .map_err(|_| HISTORY)?;
    if let Some(last) = ring.history.last() {
        if proof.head.number()? < last.until.number()?
            || (proof.head.sequence == last.until.sequence && proof.head.hash != last.until.hash)
        {
            return Err(UNSAFE.into());
        }
    }
    Ok((proof.head, proof.unverified_history))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud_sync_protocol::{self as sync, Change};
    fn fixture() -> serde_json::Value {
        serde_json::from_str(include_str!("../../tests/fixtures/key-rotation-v1.json")).unwrap()
    }
    fn first() -> Ring {
        let f = fixture();
        let r: Rotation = serde_json::from_value(f["rotation"].clone()).unwrap();
        Ring::open(
            "test-owner",
            crypto::secret(f["oldKey"].as_str().unwrap()).unwrap(),
            &r.previous_key,
            None,
        )
        .unwrap()
    }
    fn mutation(ring: &Ring, previous: &Head, version: i64) -> Change {
        let (identity, id) = ring.identity().unwrap();
        let request=sync::encrypt_with_identity(&ring.key,&ring.envelope.dataset_id,&ring.envelope.key_id,&identity,&id,"category-one",version,previous,false,br#"{"schema":1,"entity":"category","id":"category-one","value":{"name":"Food","icon":"utensils","isArchived":false}}"#,1).unwrap();
        Change {
            sequence: (previous.number().unwrap() + 1).to_string(),
            version: (version + 1).to_string(),
            request,
        }
    }
    #[test]
    fn independent_node_vector_decrypts_history_and_verifies_transferred_checkpoint() {
        let f = fixture();
        let rotation: Rotation = serde_json::from_value(f["rotation"].clone()).unwrap();
        let recovery = crypto::secret(f["recoveryKey"].as_str().unwrap()).unwrap();
        let key = crypto::unwrap(
            "test-owner",
            &rotation.key.dataset_id,
            &rotation.key,
            &recovery,
        )
        .unwrap();
        assert_eq!(
            *key,
            *crypto::secret(f["newKey"].as_str().unwrap()).unwrap()
        );
        let ring = Ring::open("test-owner", key, &rotation.key, Some(&rotation)).unwrap();
        assert_eq!(ring.history.len(), 1);
        assert_eq!(ring.history[0].key, f["oldKey"].as_str().unwrap());
        let code = history_code(
            "https://heyday.money/",
            "test-owner",
            &ring,
            Some(&rotation),
            &Head::default(),
            false,
        )
        .unwrap();
        assert_eq!(code, f["historyCode"].as_str().unwrap());
        assert_eq!(
            verify_code(
                &code,
                "https://heyday.money/",
                "test-owner",
                &ring,
                Some(&rotation)
            )
            .unwrap(),
            (Head::default(), false)
        );
        assert!(verify_code(
            &code,
            "http://localhost/",
            "test-owner",
            &ring,
            Some(&rotation)
        )
        .is_err());
        assert!(verify_code(
            &code,
            "https://heyday.money/",
            "bob",
            &ring,
            Some(&rotation)
        )
        .is_err());
    }
    #[test]
    fn rotation_preserves_ids_and_old_history_but_rejects_old_key_future_writes() {
        let old = first();
        let a = mutation(&old, &Head::default(), 0);
        let boundary = sync::verify_change(&a, &Head::default()).unwrap();
        let new = Zeroizing::new([42; 32]);
        let recovery = Zeroizing::new([43; 32]);
        let envelope =
            crypto::wrap("test-owner", &old.envelope.dataset_id, &new, &recovery).unwrap();
        let rotation =
            Rotation::create("test-owner", &old, &new, envelope.clone(), &boundary).unwrap();
        let ring = Ring::open("test-owner", new.clone(), &envelope, Some(&rotation)).unwrap();
        ring.decode(&a).unwrap();
        let b = mutation(&ring, &boundary, 1);
        ring.decode(&b).unwrap();
        assert_eq!(a.request.record_id, b.request.record_id);
        assert!(old.decode(&b).is_err());
        let forged = mutation(&old, &boundary, 1);
        assert!(ring.decode(&forged).is_err());
        let old_recovery = Zeroizing::new([9; 32]);
        assert!(
            crypto::unwrap("test-owner", &envelope.dataset_id, &envelope, &old_recovery).is_err()
        );
        let boundary2 = sync::verify_change(&b, &boundary).unwrap();
        let next = Zeroizing::new([44; 32]);
        let e = crypto::wrap(
            "test-owner",
            &envelope.dataset_id,
            &next,
            &Zeroizing::new([45; 32]),
        )
        .unwrap();
        let r = Rotation::create("test-owner", &ring, &next, e.clone(), &boundary2).unwrap();
        let latest = Ring::open("test-owner", next, &e, Some(&r)).unwrap();
        latest.decode(&a).unwrap();
        latest.decode(&b).unwrap();
        latest.decode(&mutation(&latest, &boundary2, 2)).unwrap();
        assert!(latest.contains(&old.envelope));
        assert!(latest.contains(&envelope));
        assert!(latest.matches_pin("test-owner", &old.envelope, None));
        assert!(latest.matches_pin(
            "test-owner",
            &envelope,
            Some(&rotation.fingerprint("test-owner"))
        ));
        assert!(!latest.matches_pin("test-owner", &envelope, Some(&"a".repeat(64))));
        let mut wrong = a.clone();
        wrong.request.commit_hash = "a".repeat(64);
        assert!(latest.decode(&wrong).is_err());
    }
    #[test]
    fn rotation_rejects_tampering_owner_substitution_and_stale_history_codes() {
        let f = fixture();
        let original: Rotation = serde_json::from_value(f["rotation"].clone()).unwrap();
        let key = crypto::secret(f["newKey"].as_str().unwrap()).unwrap();
        assert!(Ring::open("bob", key.clone(), &original.key, Some(&original)).is_err());
        for field in ["salt", "nonce", "ciphertext"] {
            let mut value = serde_json::to_value(&original).unwrap();
            let text = value["history"][field].as_str().unwrap();
            let mut bytes = URL_SAFE_NO_PAD.decode(text).unwrap();
            bytes[0] ^= 1;
            value["history"][field] = serde_json::json!(URL_SAFE_NO_PAD.encode(bytes));
            let r: Rotation = serde_json::from_value(value).unwrap();
            assert!(Ring::open("test-owner", key.clone(), &r.key, Some(&r)).is_err());
        }
        let mut r = original.clone();
        r.proof = URL_SAFE_NO_PAD.encode([0; 32]);
        assert!(Ring::open("test-owner", key.clone(), &r.key, Some(&r)).is_err());
        let old = first();
        let code = history_code(
            "https://heyday.money/",
            "test-owner",
            &old,
            None,
            &Head::default(),
            false,
        )
        .unwrap();
        let ring = Ring::open("test-owner", key, &original.key, Some(&original)).unwrap();
        assert!(verify_code(
            &code,
            "https://heyday.money/",
            "test-owner",
            &ring,
            Some(&original)
        )
        .is_err());
        let mut changed: serde_json::Value = serde_json::from_slice(
            &crypto::decode(
                f["historyCode"]
                    .as_str()
                    .unwrap()
                    .strip_prefix("HDV1.")
                    .unwrap(),
                1,
                2048,
            )
            .unwrap(),
        )
        .unwrap();
        changed["head"]["sequence"] = serde_json::json!("1");
        let tampered = format!(
            "HDV1.{}",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&changed).unwrap())
        );
        assert!(verify_code(
            &tampered,
            "https://heyday.money/",
            "test-owner",
            &ring,
            Some(&original)
        )
        .is_err());
    }
    #[test]
    fn history_proof_preserves_unverified_recovery_warning_and_prevents_flag_tampering() {
        let ring = first();
        let code = history_code(
            "https://heyday.money/",
            "test-owner",
            &ring,
            None,
            &Head::default(),
            true,
        )
        .unwrap();
        assert_eq!(
            verify_code(&code, "https://heyday.money/", "test-owner", &ring, None).unwrap(),
            (Head::default(), true)
        );
        let mut proof: serde_json::Value = serde_json::from_slice(
            &crypto::decode(code.strip_prefix("HDV1.").unwrap(), 1, 2048).unwrap(),
        )
        .unwrap();
        proof["unverifiedHistory"] = serde_json::json!(false);
        let changed = format!(
            "HDV1.{}",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&proof).unwrap())
        );
        assert!(verify_code(&changed, "https://heyday.money/", "test-owner", &ring, None).is_err());
    }
}
