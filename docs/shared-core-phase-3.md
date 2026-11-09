# Shared core: Phase 3 database ownership and financial services

Implemented on top of the Phase 1/2 working tree. The original desktop baseline
is `9a0e25915c1e3c960f0525dccc355f6949416959`. No application/database version changes,
new migrations, wire-format changes, mobile scaffolding, or user database operations
were introduced.

## Ownership and compatibility

`crates/heyday-core/migrations/` is the only production migration directory.
All 11 filenames, order and bytes match the baseline. In particular migration
0008 retains SHA-384
`735bacdce83d50164ad239aeecc79d7f8aa1dd5ba7c1b96daaed108284f0e9004b0aa0fd0d5c830f98876a3991775010`.
The unchanged `institutions.sql`, `subscription-providers.sql` and
`sync-metadata-seeds.json` now live in the core's `data/` directory.

`database::MIGRATOR` is shared by startup, backup validation, restore staging,
and all current-schema tests. Historical alpha fixtures stay in desktop's
`src-tauri/tests/fixtures/alpha-migrations/`; their SQL bytes remain unchanged.
Direct beta SQL includes in historical tests point to the canonical core files.
The core build script watches migrations and data; the desktop build script
also watches the new migration directory. Nothing depends on the process's
current working directory when embedding resources.

`database::operations` owns the single process-wide read/write gate formerly
in backups. Normal native commands and sync retain a shared guard for their full
operation. Restore waits for existing operations with the exclusive guard, and
queued restore rejects new commands with the existing error. Reset retains its
original database and authentication guards. There are no competing locks or
nested shared guards introduced by service delegation.

Application adapters must acquire the gate before calling top-level services.
Connection-level functions deliberately reuse the caller's SQL transaction.
Desktop still owns app-data selection, debug/release storage isolation, process
file locking, SQLite connection configuration, native dialogs, snapshot files,
backup exclusions, tokenized restore confirmation and Excel export.

## Services and adapters

Core `services/` owns the SQL operations and DTOs for:

- Accounts, transactions, incomes, salary payments and income deductions.
- Financial/expense/loan/card overview snapshots and cashflow/payment planning.
- Loan contracts, facilities, allocations, installments, card billing and limits.
- Reconciliation, selective defaults, archive/delete eligibility and payoff rules.
- Subscriptions/providers, institutions, payees/categories, PNG normalization and
  logo references/pruning, plus transactional data reset and seed restoration.

Settings reads/updates live in `database::settings`. Desktop modules retain the
same Tauri command names and parameter shapes, acquire the shared guard, and
call core services. Serde names/defaults and decimal-string money fields are
unchanged. The subscription management command reads a validated saved URL from
core but opens the browser through the existing desktop opener plugin. Core uses
`url::Url`, the same parser previously accessed through `reqwest::Url`.

Three sync-dependent calls are explicit through `database::ledger::LedgerHooks`:
imported-entry annotations, imported-group deletion protection, and missing loan
contract validation. `src-tauri/src/core_ledger.rs` delegates each call to the
existing desktop implementation, including its current transaction connection.
The `Label` and `Entry` DTOs move unchanged and are re-exported by desktop sync;
crypto, record encoding, merge rules, queue handling and transport remain in
desktop for Phase 4. The core provides no default hooks that could silently bypass
protection. No sync callback is moved outside its original transaction/order.

Existing cross-module database tests stay as desktop integration tests and call
the extracted implementation through re-exports or small test adapters. Public
service DTO fields and helper functions support this transition; this is still
an evolving 0.1 library API. No tests are removed or replaced with mocks.

## Packaging and release checks

Core embeds its migrations and seeds and packages its own fixture files. The
consumer check extracts only the packaged crate into a temporary directory,
compiles a separate consumer, and runs the packaged core tests. It seeds the
consumer lockfile from the package and checks the host-resolved dependency graph,
avoiding unrelated target downloads in offline checks. Tauri, AppKit, keyring,
browser dependencies and the desktop crate are absent from that graph.

The runtime dependencies added for services are SQLx/SQLite, Tokio coordination,
PNG, base64, SHA-256, URL parsing and serde JSON. Core chooses no native app path,
credential store, networking client or platform UI. All resolved dependency MSRV
metadata is compatible with Rust 1.89; actual 1.89 execution remains CI verification.

Desktop Cargo manifest, lockfile, target paths and installer commands remain in
`src-tauri/`. Release version scripts still update only `heyday-money`; they do
not change the independent core version. CI and macOS/Windows release workflows
now run core tests as well as the desktop suite. Native release workflows continue
to compile embedded resources from the full repository checkout.

## Validation

- All 11 production migrations and all 3 seed/reference files compare byte-for-byte
  with the baseline. Core tests pin every production migration's version/checksum.
- Core: 10 tests pass, including shared domain vectors, standalone financial SQL
  operations, transfer overflow rollback, archived-reference deletion protection,
  reversal, income snapshots, reset seeds, and queued restore gate behavior.
- Packaged consumer and packaged tests pass without desktop files or native UI.
- Desktop: 212 tests pass; the same 7 socket tests fail with sandbox EPERM as in
  Phase 2. Test names and outcomes match Phase 2 exactly, including crypto/sync,
  pinned 0008 upgrade/data preservation, backup/restore and rollback regressions.
- TypeScript domain tests: 16 pass. Frontend build, Rust check/native build and
  release-helper checks pass. The existing Vite large-chunk warning remains.
- Full browser tests cannot start their local server in this sandbox (Phase 1
  baseline restriction). Unrestricted socket/browser execution, actual MSRV CI,
  Windows execution and packaged native smoke tests remain external validation.
