# Remaining features and practical improvements

Reviewed 2026-11-06. Checked items record completed follow-up work; unchecked items remain.
These do not expand the agreed v1 financial scope. Preserve offline operation, exact monetary
arithmetic, local storage, and explicit confirmation for financial writes.

## Current verification

- Desktop version is `0.0.1-beta.5`. Current financial sync coverage is implemented
  through protocol 5; no additional currently editable financial entity was found
  waiting for its own sync implementation.
- The initial review passed frontend typecheck/build, all **201 native Rust tests**,
  all **232 browser tests**, and release-helper tests. Subsequent subscription
  integration checks are recorded below. Migration 0008 retains its pinned
  SHA-384 checksum; migration 0009 remains additive.
- Browser tests mock native IPC. These results do not verify installed macOS/Windows
  apps, real OS credential dialogs, live billing, production deployment, or an
  independent cryptographic audit.
- The frontend build emits a roughly **1.36 MB** main JavaScript chunk
  (**358 KB gzip**) and a size warning. Measure actual startup/navigation before
  choosing a splitting strategy.

## Cloud release priorities

- Implemented desktop subscription integration: verified account-scoped status,
  refresh after returning from website billing, timed expiry and access recovery for
  already-enabled sync. Pause and first-upload consent remain intact. Targeted
  verification covers 66 native cloud tests, 48 browser scenarios, 23 API tests and
  both PostgreSQL integration suites, including payment failure blocking encrypted
  push/pull and renewal restoring access to retained ciphertext.
- [ ] Run two packaged devices against the same test service/account: first sign-in,
  registration, E2EE setup/recovery, restart and returning sign-in, bidirectional
  edits, offline changes, conflicts, pause/resume, lost-response retry and revocation.
  Confirm the reported repeated Keychain prompt is resolved in installed builds.
- [ ] Complete the real Stripe sandbox-to-desktop flow: checkout/trial enables sync;
  renewal, payment failure, cancellation at period end and trial expiry change sync
  access correctly; encrypted cloud data remains retained and local use continues.
  Website checkout and automated entitlement tests alone do not prove this flow.
- [x] Implement dataset/recovery-key rotation and replacement, encrypted historical
  key archives, exact retries, stable record IDs and rejection of obsolete-key writes.
  Retain old history, paused sync and pending local edits.
- [x] Add trusted-device history codes and verify the complete pinned prefix before
  applying downloads. Explicit lost-device recovery retains and propagates a
  warning; it never lowers an existing checkpoint. See `docs/encryption-security.md`.
- Encryption verification: 74 native cloud tests, 52 browser scenarios, 25 API
  tests and three PostgreSQL suites. A real-encryption, three-SQLite-database
  round trip preserves financial IDs, balances, pending edits and old history
  across rotation. Local Docker runs additive service migration 0009 and is healthy.
- [ ] Obtain an independent security review and complete packaged two-device
  rotation/crash/recovery QA. The reviewer brief is prepared; no independent audit
  has occurred. Global freshness/transparency and cryptographic device enrollment
  are outside the implemented checkpoint-transfer protocol.
- [ ] Verify the release API origin, deployed HTTPS service, real email delivery,
  and production configuration with an installed app. This review did not inspect
  Railway, production credentials, or remote repository settings.
- [ ] Measure sync latency, large-dataset payloads and cloud history/storage growth.
  History is not currently pruned; implement compaction and expired-cursor recovery
  before adding any retention pruning.

## Transaction usability and scale

- [ ] Add date-range and description search alongside the existing account,
  type, payee and category filters. Clearly label totals for the filtered range.
- [ ] Add transaction pagination or virtualization, with bounded native queries.
  Keep exact spending totals over all matching records, not just the visible page.
  Benchmark a large synthetic history before choosing an approach. Keep Outlook's
  fixed seven-period table behavior independent from history pagination.
- [ ] Design direct transaction correction and undo. Preserve atomic balances,
  grouped payroll/loan payments, reconciliation evidence, and statement review
  flags. Define audit/history behavior before replacing delete-and-recreate.

## Data portability

- [ ] Export transactions as CSV with documented date, currency, minor-unit and
  account conventions. Handle spreadsheet formula injection in user-entered text.
- [ ] Import CSV through preview, column mapping, validation and duplicate review.
  Commit confirmed imports atomically; do not infer transfers from similar amounts.
  CSV exchange complements full SQLite backup/restore; it does not replace it.

## Scheduling and planning

- [ ] Extend income recurrence beyond monthly: weekly, fortnightly and yearly.
  Specify anchors, intervals, month-end behavior and migration rules explicitly.
- [ ] Add budgets and budget-versus-actual category reports, defining card-purchase
  spending separately from cash repayments to prevent double counting.
- [ ] Add suggested matches between planned payments and actual transactions, with
  user confirmation, partial payments, split payments and reversal handling.
- [ ] Add duplicate detection for manual planner rows and imported/recorded payments.
  Suggestions must not silently delete records or suppress legitimate payments.
- [ ] Consider opt-in recurring transaction automation only after defining missed
  runs, idempotency, catch-up review and safe reversal. Definitions remain estimates.

## Net worth history

- [ ] Design dated balance/valuation snapshots and historical net-worth charts.
  Define how corrections, archives and manual investment valuations affect history.
  Do not present reconstructed starting balances as historical bank snapshots.

## Distribution and native verification

- [ ] Configure Windows code signing and macOS Developer ID signing/notarization.
  This needs signing identities and external account configuration.
- [ ] Implement signed Windows in-app updates, including manifest publication,
  recovery backup, installer restart behavior and upgrade regression testing.
- [ ] Define MSI prerelease upgrade ordering. The current numeric override maps
  beta releases with the same major.minor.patch to the same MSI version; verify
  upgrade and downgrade behavior before promising ordered beta upgrades.
- [ ] Run packaged Windows/macOS smoke tests: fresh offline install, upgrade,
  backup picker cancellation/overwrite, restore, duplicate launch, and data retention.
  Browser tests mock native IPC and cannot replace these checks.
- [ ] Configure repository branch protection to require the CI frontend/browser
  and both platform Rust jobs. The workflow exists; repository rules are separate.

## Performance and maintenance

- [ ] Measure startup and navigation before splitting the roughly 1.34 MB frontend
  JavaScript bundle into route chunks. Preserve bundled offline assets.
- [ ] Reduce duplicated and outdated requirements in AGENTS.md and development docs,
  especially older alpha references superseded by beta payroll and backup features.

## Review issues addressed

- Platform-specific manual update instructions and tag-release Windows installers.
- Independent Windows installer jobs, preserving EXE output if MSI packaging fails.
- An OS file lock per database directory, acquired before migrations and held for
  the app lifetime; debug and release storage remain independent.
- Same-day financial refresh on window focus, with failed-load retry coverage.
- PR/main CI for the frontend build, browser suite, release helper and Rust tests
  on macOS and Windows. Native packaging and external branch rules still need QA.
