# Encryption maintenance and recovery

Advanced security in the Heyday account panel now supports:

- **Replace recovery key**: save/re-enter a new user-held recovery key. This also
  rotates the dataset root for future changes. Earlier roots remain in an archive
  encrypted under the new root, preserving access to existing cloud history.
- **Finish / Cancel pending replacement**: safely handle interrupted requests.
  The exact encrypted request is saved before sending, and both roots remain in
  the OS credential store until the new package is acknowledged. Cancellation
  checks whether the server already committed; it never reverses a committed key.
- **Verify another device**: check cloud history against this device's saved pin
  and display a code to transfer directly to the new device with its recovery key.
  The code is a minimum-history checkpoint, not a key or global freshness proof.
- **Recovery without a trusted device**: a separate unchecked warning allows
  recovery when all devices/codes are lost. It never clears an existing pin. A
  persistent notice records the lack of independent verification, including after
  restart, key removal and rotation. Exported codes authenticate and propagate that
  notice; a later agreeing trusted code can clear it.

Rotation does not enable sync or resume a deliberate pause. Other devices must
restore with the latest recovery key. Local finances, queued changes and backups
remain available. Old keys cannot decrypt future mutations, but copies of old
plaintext/ciphertext/keys cannot be erased. Revoke a lost/untrusted device separately.
Support has no recovery key. SQLite financial data and financial backups remain
plaintext; cloud E2EE does not change that boundary.

The service must advertise key management version 2 before rotation is offered by
the native command. Initial `heyday-key-v1` wrappers and existing record derivations
are preserved. `cloud_key_protocol.rs` owns the archive, transition proof, stable
record IDs and authenticated checkpoint codes. `cloud_e2ee.rs` owns native keys,
pins and durable requests. `cloud_sync.rs` chooses epochs by authenticated sequence
boundaries and verifies a complete pinned prefix before applying downloaded data.
Temporary staging files contain only ciphertext and are removed on drop. Unverified
recovery is an explicit exception to having an independent initial anchor, not an
exception to AEAD/MAC, entity validation, epoch boundaries or saved-pin checks.

The scoped `cloud-key-packages` app-data directory contains encrypted/public retry
requests only. The small credential-store entry holds raw current/pending keys and
their package pins; historical roots are decrypted only in native memory. These
files and credentials are outside SQLite and financial backups. Remember-key
preferences survive rotation; explicit forgetting removes both usable roots.

Protocol parsing bounds are 128 KiB archive ciphertext, 192 KiB key responses and
requests, and 2 KiB decoded verification codes. Full-prefix verification stages at
most 512 MiB / 8,192 pages and returns a clear error rather than bypassing checks.
These are implementation bounds, not subscription quotas. No cloud history is
pruned. Large-history compaction/streaming remains follow-up work.

## Validation and independent review

Public `tests/fixtures/key-rotation-v1.json` vectors are independently checked by
Rust and the companion service's Web Crypto tests. Native tests cover multiple
epochs, stable IDs, old-key future-write rejection, tampering, account/origin pins,
recovery-warning inheritance, late-page forks, encrypted staging and retention of
pending keys/pins. Browser tests cover saved-key confirmation, interrupted retry,
new-device verification and warned recovery while preserving Pause and consent.

**Independent security review is not completed.** The service repository contains
the full protocol specification and a reviewer brief in `docs/key-rotation.md` and
`docs/security-review.md`. An external reviewer must assess a fixed pair of app/API
commits and record findings, remediation verification and a release recommendation.
Automated tests and implementation self-review are not independent assurance.

Before production, run installed macOS and Windows devices through setup, keychain
access failure, restart, rotation, cancellation after a lost response, recovery,
offline queued edits, account/session revocation and service upgrade. Browser mocks
do not validate OS credential storage or a real multi-device network deployment.
This implementation does not provide a transparency witness, continuous global
fork detection, device-specific signing identity or post-compromise erasure.
