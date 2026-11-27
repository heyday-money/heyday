# Core 0.1.0 compatibility

The shared library version is independent of application, database, and wire versions.
A mobile dependency update must identify all of them; an app release tag does not
implicitly update the library or its protocols.

| Contract | Initial extracted core |
| --- | --- |
| Rust library | `heyday-core` 0.1.0, edition 2021, Rust 1.89 minimum |
| Desktop integration | 0.0.1-beta.8 |
| SQLite schema | Beta migrations 0001 through 0014; additive upgrades only |
| Sync request header | `X-Heyday-Sync: 7` |
| Key protocol header | `X-Heyday-Keys: 2` |
| Record encryption | `heyday-record-v2`, existing HKDF derivation preserved |
| Legacy history | Existing supported record protocols, immutable retry ciphertext and epoch history retained |

This is an evolving 0.x Rust API. Pin the full Git commit and commit the consuming
application's Cargo.lock, even when the crate version is unchanged. An incompatible
public Rust API change should increment the minor library version; compatible fixes
may increment the patch. Neither change permits rewriting released migrations or
changing serialized/wire formats without a separate compatibility plan.

Desktop consumes a path dependency in this repository. Mobile consumes package
`heyday-core` from `https://github.com/heyday-money/heyday.git` with a full `rev` and
an exact expected library version. No floating branch/tag, sibling path, local patch,
automatic adoption, or runtime library update is supported for release builds.

Before adopting a core revision, review migration/checksum changes, money serialization,
record kind support, retry/conflict behavior, key derivation, history checkpoints and
adapter API changes. Run host regression tests, clean Git-consumer checks and mobile
target builds; changes touching sync/keys also require packaged cross-device QA.
A passing cross-compile is not device secure-storage or authentication validation.

The core embeds its migrations, SQL/reference data and test vectors. It has no Tauri,
HTTP client, keyring or particular app-directory dependency. Applications supply
secure storage, authenticated transport, lifecycle scheduling and operation guards;
see README.md for the adapter contract. Retain the Apache-2.0 LICENSE and attribution
when distributing the library as part of a proprietary application.

Tracking adds migrations 0012/0013 and protocol-6 encrypted asset/event aggregates.
Desktop exposes dedicated IPC/UI and atomic derived cash legs. Deploy Site migration
0011 and protocol-6 support before rollout. Older clients are gated once a dataset
accepts protocol 6. Mobile remains pinned until a deliberate compatibility update.
Full financial sync consent is required for Tracking in connected databases.


Insurance adds migration 0014 and protocol-7 encrypted policy/schedule/premium
aggregates. Site migration 0012 and handlers must deploy first. Older clients are
blocked after the dataset accepts protocol 7. Premium cash entries are derived
atomically; generic ledger uploads exclude them. Mobile adoption remains explicit.
