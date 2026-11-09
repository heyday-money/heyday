# heyday-core

Shared Rust library for Heyday.Money, licensed under Apache-2.0 (see LICENSE).
This library is part of the Heyday.Money desktop repository and retains its
license. No third-party assets or mobile-specific proprietary code are included.

The domain module provides pure transaction validation and input types, payroll rules,
checked balances, decimal conversion, and calendar helpers under `domain`.
The core owns canonical migrations, seed data, database coordination, financial
SQL services, encryption, and sync execution. Platform integration remains application-owned. Rust edition 2021 and
MSRV 1.89 match desktop. The library version is independent of application,
database and sync protocol versions; this is not yet a complete mobile backend.

From the desktop repository root, run:

```sh
cargo test --locked --manifest-path crates/heyday-core/Cargo.toml
python3 scripts/check-core-consumer.py
```

The consumer check packages the crate, extracts it outside the checkout, and
compiles an independent consumer. It checks the resolved dependency graph for
platform dependencies. It does not publish anything. No workspace is required;
the desktop application keeps its manifest and lockfile in src-tauri/.

See ../../docs/shared-core-phase-1.md for the baseline and
../../docs/shared-core-phase-2.md for extraction scope and validation.

Application adapters acquire `database::operations::operation()` before calling a
top-level service and keep the guard through all reads/writes and imported-history checks.
Restore acquires `database::operations::exclusive()` instead. Never acquire a
second shared guard inside an operation; queued restore must reject new work.
Connection-level methods run within the caller's transaction.

Financial services use the canonical sync ledger directly for imported history,
protected deletion, and contract checks. There is no application-supplied ledger hook.

`crypto::encryption` and `crypto::key_protocol` provide existing key wrapping,
record encryption, rotation archives, package pins, and transferred history proofs.
`sync` owns record validation/application, durable queue/shadow state, and the engine.
All financial record kinds are supported independently of an application's editing UI.

`sync::engine::perform` accepts a pool, canonical origin/account, a native `SyncKey`,
and an `Adapter`. The adapter provides authenticated page/push transport and durable
secure-store checkpoint writes. It must preserve protocol headers and response bounds,
return existing protocol errors (including key changes) without rewriting them, and
persist checkpoints before reporting success. Applications validate device access and
the pinned key package before constructing this context. Keep keys and credentials
outside frontend IPC, SQLite, financial backups, and logs.

Callers serialize authentication/key operations and hold the shared database guard
for the whole engine invocation, including adapter calls. The engine does bounded work;
foreground, edit, reconnect, and timer scheduling belong to the application. History
verification stages ciphertext in automatically removed temporary files, using ordinary
OS temporary storage with no dependency on a desktop app directory. No decrypted
financial pages or raw keys are written to these files.

The package includes unchanged crypto vectors and database regression fixtures.
See ../../docs/shared-core-phase-4.md for the extraction and validation record.

See ../../docs/shared-core-phase-3.md for database/service boundaries, baseline
checksums, packaging, and validation. The 0.1 library API is still evolving during
extraction; desktop IPC and financial formats remain compatible.
