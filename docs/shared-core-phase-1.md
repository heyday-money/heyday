# Shared core: Phase 1 baseline and extraction inventory

Baseline desktop revision: `9a0e25915c1e3c960f0525dccc355f6949416959`.
The working tree was clean before implementation. Checks ran on macOS with
Bun 1.4.2 and Rust 1.98.1. The library declares the existing edition (2021) and
MSRV (1.89); CI checks the isolated library with Rust 1.89 on Linux.

Phase 1 adds only the library boundary and desktop path dependency. No application
logic, IPC, fixtures, migrations, or financial behavior moves in this change.
The library has no dependencies and deliberately exposes no placeholder services.
The first pure-rule extraction belongs to Phase 2.

## Validation

| Check | Starting revision | After Phase 1 |
| --- | --- | --- |
| `bun run build` (TypeScript and Vite) | Passed; existing large-chunk warning | Passed; same warning |
| `bun scripts/github-release.test.cjs` | Passed | Passed |
| `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml` | 214 passed, 7 socket tests blocked by sandbox EPERM | Identical: 214 passed, same 7 blocked |
| `bun run test:e2e -- --workers=2` | Blocked: sandbox denies listening on 127.0.0.1:1420 | Same environment restriction; unverified |
| `cargo check --locked --offline --manifest-path src-tauri/Cargo.toml` | Rust test compilation succeeded | Passed |
| `python3 scripts/check-core-consumer.py` | Not present | Passed: packaged crate and independent consumer, no platform dependencies |

Frontend unit/domain tests live in Playwright's `tests/*.spec.ts`, not Bun test
files under `src/`. The normal browser command includes those tests. Run the full
suite in existing frontend CI; do not interpret sandbox failures as passing tests.

The seven baseline Rust failures are the two `cloud_auth` loopback tests, three
`cloud_devices` HTTP tests, `cloud_e2ee`'s native API test, and `cloud_sync`'s
history staging HTTP test. All fail with permission-denied socket operations.
Existing macOS/Windows CI retains the full Rust suite without skips.
The native `cargo build --locked --offline --manifest-path src-tauri/Cargo.toml`
also passed after implementation, as did core tests and formatting. The empty
core has zero tests; none were removed from desktop. Local Rust 1.89 is not
installed, so MSRV execution awaits the new CI job. Packaged desktop smoke
testing and Windows verification remain external checks.

## Extraction inventory

Refresh this inventory before moving each module: inspect production calls and
its tests separately, then carry fixtures and compile-time resources with it.
The table lists the important coupling, not a promise that entire files can move.

| Area | Coupling and extraction constraints |
| --- | --- |
| `transactions.rs` | Owns date validation, exact amount/balance rules and SQL posting; uses root `Settings`, backups operation guard, loans allocation/deletion, salary groups, reconciliation evidence and cloud ledger protection/imports. Separate pure rules first; keep grouped SQL behavior intact. |
| `salary_payments.rs`, `income_deductions.rs`, `incomes.rs` | Payroll calls transaction insertion/date validation/reversal, known-contract sync checks and loan allocation validation. Command wrappers acquire backup guards. Existing SQLite tests need full schema, not just payroll tables. |
| `accounts.rs`, `loans.rs`, `installments.rs`, `card_billing.rs`, `reconciliation.rs` | Shared posting, debt signs, references, group reversal and review effects couple these operations. Historical tests embed alpha SQL and selected beta migrations. Move existing tests with each operation. |
| `cashflow.rs`, `planning.rs`, `selective_defaults.rs`, `card_limits.rs` | Database snapshots and forecast definitions use other financial modules and root settings. Preserve exact integer strings and stable month keys. |
| `lib.rs`, `backups.rs`, `reset.rs` | Startup and backup validation each embed migrations. Backup operation coordination serializes commands with restore/reset/sync. Root owns app-data paths and Tauri setup; `instance.rs` owns OS file locking. Do not create competing operation locks. |
| `cloud_crypto.rs` | Crypto primitives are a candidate for later extraction; tests embed `tests/fixtures/e2ee-v1.json`. Keep OS key storage in adapters. |
| `cloud_sync_protocol.rs`, `cloud_key_protocol.rs` | Depend on crypto; key protocol decodes store records, and protocol tests call store decoding. Move in dependency order, preserving canonical encodings and history verification. |
| `cloud_sync_store.rs`, `cloud_sync_ledger.rs`, `cloud_sync_income.rs`, `cloud_sync_loans.rs`, `cloud_sync_planner.rs`, `cloud_sync_metadata*.rs` | SQL queue/shadow/record application depends on financial operations, crypto/protocol formats and reference validation. Metadata rules embed seed JSON. `cloud_sync_complete_tests.rs` exercises cross-module integration. |
| `cloud_auth.rs`, `cloud_devices.rs`, `cloud_e2ee.rs`, `cloud_sync.rs` | Tauri state/events, HTTP, credential store, installation paths, timers and lifecycle orchestration must be separated from portable protocol/state transitions. |
| `logos.rs`, catalogs, subscriptions | PNG normalization, asset references, bundled seed data and pruning interact with reset, backups and sync. Catalog images are currently frontend assets. |
| `exports.rs`, `updates.rs`, `dock_icon.rs` | Native dialogs, Excel output, updater integration and macOS AppKit remain desktop-owned initially. |
| `src/lib/money.ts`, `period.ts`, `financial.ts`, `cashflow.ts`, `salary-payments.ts`, schedule/overview helpers | Frontend BigInt arithmetic, dates and forecasts must be inventoried alongside Rust before domain extraction; retain display/localization in TS and share behavioral fixtures later. Playwright specs cover these helpers. |

### Embedded resources, configuration and platform boundary

- Production schema: `src-tauri/migrations/*.sql`; startup in `lib.rs`, static
  backup migrator in `backups.rs`, and many module tests use `sqlx::migrate!`.
  `build.rs` watches `migrations`. Preserve every byte and the 0008 checksum.
- Historical schema: `src-tauri/tests/fixtures/alpha-migrations/` and its README.
  Tests also directly embed individual beta SQL files; paths are manifest-relative
  for SQLx and source-relative for `include_str!`.
- Crypto vectors: `tests/fixtures/e2ee-v1.json`, `sync-v1.json`, `sync-v3.json`,
  `sync-v4.json`, `sync-v5.json`, and `key-rotation-v1.json` are currently outside
  `src-tauri/`. Package the necessary vectors when extracting their tests.
- Seeds: `src-tauri/data/institutions.sql`, `subscription-providers.sql` are used
  by reset; `sync-metadata-seeds.json` is embedded by metadata rules. Frontend
  institution/provider images and licensed fonts remain under `public/`.
- Desktop assets: `src-tauri/assets/auth-callback.html` is generated by
  `scripts/build-auth-callback.ts`; `dock_icon.rs` embeds light/dark PNG icons.
- Compile-time environment: `HEYDAY_API_URL` in auth (watched by `build.rs`),
  `HEYDAY_UPDATER_PUBLIC_KEY` in updates. `HEYDAY_INSTANCE_TEST_DIRECTORY` is a
  subprocess test-only override. Keep application configuration out of core.
- Platform dependencies: Tauri and its build/dialog/opener/updater plugins,
  keyring native backends, macOS `objc2`, AppKit/Foundation, platform filesystem
  paths and instance locks. Excel export (`rust_xlsxwriter`) stays desktop-owned.
- Release scripts address `src-tauri/Cargo.toml`, `Cargo.lock`, configuration and
  targets. These paths remain unchanged; core's independent version must not be
  changed by desktop release version scripts.

### Consumer check

`scripts/check-core-consumer.py` packages only `heyday-core`, extracts regular
files to a temporary directory, creates an unrelated Cargo consumer, checks its
resolved dependency graph, compiles it and runs the packaged core tests. This
catches source-relative includes or path dependencies outside the distributed
crate. No desktop build, sibling repository or native toolkit is needed. The
check is offline; future dependencies must be fetched by the preceding core test
step in CI. It is a packaging guard, not yet a mobile Git-pin integration test.

Core includes its own unchanged copy of the desktop Apache-2.0 LICENSE. There is
no root NOTICE file. No font/image assets or third-party notices are copied into
the empty core. Preserve relevant notices as code/assets are extracted later.

### Production migration baseline (SHA-384)

Compared byte-for-byte with the starting revision; all unchanged.

```text
cea80c38884044fbe1fcc8aae1bcbffc893551fe99518aec779bc11bfa44295c2f73002431e879b71158b82a0ff9b40a  0001_initial.sql
e15357a7a786a2e28745b0cded14105afcdfe37d1df362c2f4ac06385bcfa86ac682e9bdd101a8910ecee7be68df98c8  0002_salary_payments.sql
5e0735c5e5415853f5538754606b4e048b13c3bf3175378a34c92102118d0ff164ac332b6726d11908513ddc46a9414e  0003_subscription_providers.sql
819585a3e0d0cd108a61af709d21fbf7e9bc65e215a6616365f9ab16bbaa97b4b313c6b210504d7ae231ad2ff7efc285  0004_category_sync.sql
90ec2938a35a37dfb3dc44040a6b5fe64157404c05c21b372e8b6c82ca19f299b57649aa8fa67d0d7a9b1cadb27cbdbd  0005_ledger_sync.sql
57b34e084860ccca6cd4912ac275cd871d0376e3968af1b480d7b2f3fb9e6aefa93d732f863c52cac5ca0fba3f1366a1  0006_income_sync.sql
d35abcebcc6b5b859559f3ff4532f81824a5c276606cde65ea7b5cb8595478fefdcb7d4da61407e01a5b53432c4382a9  0007_income_planner_sync.sql
735bacdce83d50164ad239aeecc79d7f8aa1dd5ba7c1b96daaed108284f0e9004b0aa0fd0d5c830f98876a3991775010  0008_complete_sync.sql
b1edd31e7d075715722d4cb32dd92f1a364a5bb57f8be7aa63e438c4bfcd4ffdcfe8c6988b0ed0cd43ea3da79c5e9bda  0009_sync_review_effects.sql
405de471647e7dc3f01b4affda96b7e8290c4f073bab8fe7f35beb77776fc7599fb1c0132f129903901c86875da2bd05  0010_irregular_income.sql
d241391b0ae53ab6a27f7ee77aca668d9e460ef91824244fd47a3b760c2e3f56e7533d54ce14f376b03a30f664a59016  0011_cash_account_logos.sql
```
