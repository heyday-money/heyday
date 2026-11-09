# Phase 4: encryption and sync extraction

Phase 4 moves the existing encryption protocols, durable sync state, financial record
handlers, and sync execution into `crates/heyday-core`. Desktop remains the native
adapter. This change does not alter the schema, service API, protocol versions,
subscription enforcement, serialized payloads, or command names.

## Ownership

| Core module | Existing implementation moved |
| --- | --- |
| `crypto/encryption.rs` | `cloud_crypto`: recovery wrapping, encryption primitives, authenticated context |
| `crypto/key_protocol.rs` | `cloud_key_protocol`: epochs, rotation archives, package pins, independent history proofs |
| `sync/protocol.rs` | `cloud_sync_protocol`: record-v2 derivation, opaque IDs, canonical requests, history verification |
| `sync/store.rs` | Durable dirty revisions, immutable pending ciphertext, shadow/conflicts, atomic page application |
| `sync/ledger.rs`, `income.rs`, `loans.rs`, `planner.rs` | Accounts, grouped transactions, payroll, contracts and forecasts |
| `sync/metadata.rs`, `metadata_rules.rs` | Allowlisted complete-financial metadata, dependencies, assets and review effects |
| `sync/engine.rs` | Bounded push/pull execution, complete-prefix validation, checkpoint ordering, ciphertext staging |

Desktop compatibility reexports preserve existing module call sites. The native sync
adapter retains request headers (`X-Heyday-Sync: 5`, `X-Heyday-Keys: 2`), bearer
authentication, bounded HTTP parsing, entitlement errors, and secure checkpoint writes.
Authentication, keyring access, enrollment/recovery/rotation orchestration, platform
paths, timers, and window/network events remain in the application. No server or
frontend product flows changed.

The engine's `Adapter` has only page retrieval, mutation submission, and checkpoint
persistence. It receives public/encrypted payloads; native `SyncKey` holds the key ring.
The application must authenticate/validate device access and package pins, serialize
account/key operations, and retain the shared database operation guard through the
engine invocation. Checkpoint storage must be durable and outside SQLite/backups.
The existing desktop lock order and checkpoint persistence implementation are retained.
A mobile caller can invoke the same engine from foreground/edit/reconnect events.

Temporary verification files still contain only ciphertext and public protocol data,
are bounded as before, and are removed on drop. They use ordinary OS temporary storage;
there is no dependency on a desktop app directory, Tauri, HTTP client, or keyring in
core. All supported financial kinds remain available; original allowlists, version
checks, and fail-closed unsupported-format handling are unchanged.

Phase 3's `LedgerHooks` and `DesktopLedger` are removed. Financial services now call
the canonical core imported-history, protected-deletion, and missing-contract checks.
Applications cannot substitute a permissive ledger hook. Core services still do not
reacquire the operation guard inside an existing operation/transaction.

## Compatibility and coverage

All eleven migration files, all three bundled seed files, and all six crypto/vector
JSON files match the original Git bytes. Vectors now live inside the packaged core;
the native encryption API test and browser recovery test reference that same source.
The moved tests preserve protocol 1/3/4/5 vectors, record-v2 derivation, rotation and
stable IDs, legacy pending ciphertext, multi-database financial replay, duplicate
posting prevention, conflict resolution, rollback and deletion protections.

54 existing tests moved from desktop into core, including the cross-domain complete
sync suite. No prior desktop test was removed from coverage. Two existing payroll
fixture helpers are also available privately to core sync tests. Three new engine
boundary tests cover:

- Ciphertext-only history staging and late-fork rejection without sockets (the original
  native HTTP/header regression is retained separately).
- Network retry with the exact persisted request, secure-store failure before financial
  application, successful checkpoint persistence, and pending-request acknowledgement.
- Complete-prefix rejection of a late fork before any downloaded category or database
  cursor is committed.

## Validation

Validated locally with Rust 1.98.1 and Bun 1.4.2:

- Core: **67 passed** (60 unit tests, 3 database integration tests, 4 domain tests).
- Desktop: **158 passed, 7 sandbox-blocked socket tests**. Those same seven failed with
  socket permission errors before extraction. Previously: 212 passed, 7 blocked;
  the 54 moved tests account for the count change.
- Packaged core extracted outside the checkout, independent consumer compilation,
  dependency graph check, and all packaged tests: passed.
- Native `cargo build --locked --offline`: passed.
- Frontend domain suite: **16 passed**; frontend production build and release-helper
  tests passed. The existing bundle-size warning remains.
- Migration/seed/vector byte comparisons and `git diff --check`: passed.
- Resolved host dependency MSRV declarations do not exceed 1.89. Actual Rust 1.89
  compilation remains a CI check; that toolchain is not installed here.

The seven socket restrictions affect two auth callback tests, three device API tests,
one native encryption API test, and one HTTP history staging test. Full browser tests
also require the previously blocked local server bind and remain unverified here.
Windows/MSRV CI, packaged credential-store recovery/rotation, and real two-client
interoperability against a pre-extraction build remain external release validation.
Passing fixtures and multi-database tests do not claim that packaged smoke QA is done.

No commit, push, publication, mobile dependency pin, or private repository creation
was performed. The library API remains 0.1 and evolving during extraction; Phase 5
must pin a reviewed full commit SHA separately from app/schema/protocol versions.
