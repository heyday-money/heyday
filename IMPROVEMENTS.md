# Remaining features and practical improvements

Reviewed 2026-10-19. These are proposed follow-up tasks, not implemented features
or changes to the agreed v1 scope. Preserve offline operation, exact monetary
arithmetic, local storage, and explicit confirmation for financial writes.

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

- [ ] Measure startup and navigation before splitting the roughly 1.2 MB frontend
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
