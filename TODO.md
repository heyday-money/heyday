# Before Beta

Prioritized readiness checklist following the alpha.6 review. Beta should focus on
safe recovery, everyday record management, and reliable upgrades.

## Required before beta

### 1. Manual backup and restore

- [ ] Add Export backup and Restore backup in Settings.
- [ ] Export a consistent SQLite snapshot using the existing snapshot approach,
      including custom logos and their references.
- [ ] Validate the selected backup's integrity, application schema, and supported
      migration version before replacing any data. Reject incompatible backups
      with a useful error.
- [ ] Require explicit restore confirmation and preserve a recovery snapshot of
      the current database before replacement.
- [ ] Coordinate database connections and writes during restore; a failed restore
      must leave the original data usable.
- [ ] Verify round-trip restoration of accounts, transactions, incomes, deductions,
      billing records, schedules, planner overrides, settings, and logos.
- [ ] Test corrupt files, unsupported newer backups, cancellation, and write failures.

### 2. Income source management

- [ ] Allow editing source name, estimated amount, recurrence day, destination,
      and active status.
- [ ] Preserve source IDs, linked receipts, deduction IDs, and cycle overrides.
- [ ] Validate changes in Rust and protect unsaved drafts in the dialog.
- [ ] Explain that definition changes affect generated forecasts, including past
      estimates, while explicit overrides and recorded transactions remain intact.
- [ ] Verify that disabling a source removes its generated forecasts without
      deleting history or changing balances.
- [ ] Keep transaction automation disabled.

### 3. Account archive and restore

- [ ] Add confirmed archive and restore actions for general accounts, keeping the
      existing Mark as paid off flow distinct.
- [ ] Preserve balances, transactions, references, and reconciliation history.
- [ ] Explain outstanding balances and the effect of archiving on Net Worth and
      forecasts before confirmation.
- [ ] Exclude archived accounts from new-record selectors and generated forecasts,
      while retaining historical reporting and shared credit-limit membership.
- [ ] Provide access to archived account details and history.
- [ ] Validate archival/restoration and dependent forecast behavior in Rust tests.

### 4. Automated release checks

- [ ] Run the frontend build, Rust tests, and Playwright suite on pull requests.
- [ ] Require those checks before publishing a release.
- [ ] Add an installed-app smoke checklist covering currency setup, account
      creation, transaction recording, restart persistence, deletion reversal,
      and offline use with real SQLite data.
- [ ] Verify the packaged version matches the release tag. The reviewed checkout
      declared alpha.5; the release script applies the tag version during packaging.

### 5. Upgrade and recovery verification

- [ ] Upgrade a populated alpha.6 database using the packaged beta candidate.
- [ ] Verify account balances, transaction history, statement payment links,
      deductions, schedules, overrides, payoff markers, shared limits, and logos.
- [ ] Confirm versioned migrations preserve applied migration checksums and data.
- [ ] Test update failure and recovery from the pre-update snapshot.
- [ ] Verify that unavailable update services do not prevent offline app use.
- [ ] Test the universal macOS package on Apple Silicon and Intel, or document
      any platform not yet verified.

## Beta polish

- [ ] Investigate and fix the repeated `ResizeObserver loop completed with
      undelivered notifications` errors observed during browser tests.
- [ ] Make unexpected browser errors fail relevant UI tests.
- [ ] Add transaction date-range filtering and text search.
- [ ] Measure transaction/report performance with thousands of records; use the
      results to decide whether history needs pagination or virtualization.
- [ ] Review narrow-window layouts, keyboard navigation, focus handling, and both
      themes in the installed app.
- [ ] Review first-run guidance for currency, accounts, opening balances, income,
      and the distinction between actual activity and forecasts.
- [ ] Investigate the production bundle-size warning if installed-app startup
      measurements show a meaningful performance problem.

## Public beta distribution and documentation

- [ ] Configure Apple signing and notarization for public macOS distribution,
      or document installation steps and limitations for a limited tester beta.
- [ ] Document balance sign conventions and treatment of archived balances.
- [ ] Explain shared-currency reports versus the independent THB planner.
- [ ] Document delete-and-record-again transaction corrections and confirmations
      affecting reconciled entries, loan payment groups, and card statements.
- [ ] Document backup, restore, upgrade recovery, and local storage behavior.
- [ ] Publish beta release notes, known limitations, supported platforms, and
      bug-report instructions that avoid requesting private financial data.
- [ ] Update stale implementation-status summaries in project documentation.

## Can wait until after the first beta

- Transaction editing with atomic balance correction and change history.
- Cloud sync and automatic transaction matching.
- Automatic income transactions and advanced recurrence rules.
- Frozen historical forecast snapshots and historical Net Worth trends.
- More advanced reports and payment-allocation workflows.

## Review baseline

The local code review completed on 2026-10-01:

- Production build passed, with a bundle-size warning.
- 92 Rust tests passed.
- 122 Playwright browser/domain tests passed, with repeated ResizeObserver errors.
- Browser UI tests mock Tauri commands; they do not verify the complete native
  application and SQLite path.
- The installed alpha.6 binary and packaged upgrade path were not verified.

Beta readiness requires completing the required checklist and recording the
packaged-app verification results; passing the existing suites alone is insufficient.
