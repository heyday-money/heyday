# Before Beta

Prioritized checklist for safe recovery, everyday record management, and reliable
upgrades. Completed implementation is summarized below; packaged-app verification
remains required before beta.

## Required before beta

### Release automation

- [ ] Run the frontend build, Rust tests, and Playwright suite on pull requests.
- [ ] Require those checks before publishing a release.
- [ ] Verify the packaged version matches the release tag.

### Installed-app verification

- [ ] Record a smoke checklist covering currency setup, account creation,
      transaction recording, restart persistence, deletion reversal, and offline
      use with real SQLite data.
- [ ] Verify native backup file pickers, export/restore, recovery snapshots, and
      cancellation in the packaged app.
- [ ] Verify account archive/restore and its effects on selectors and forecasts.
- [ ] Upgrade a populated alpha.6 database using the packaged beta candidate.
- [ ] Verify balances, transaction history, statement payment links, deductions,
      schedules, overrides, payoff markers, shared limits, and custom logos.
- [ ] Confirm migrations preserve applied migration checksums and user data.
- [ ] Test update failure and recovery from the pre-update snapshot.
- [ ] Verify unavailable update services do not prevent offline app use.
- [ ] Test the universal macOS package on Apple Silicon and Intel, or document
      any platform not yet verified.

## Beta polish

- [ ] Investigate and fix the repeated `ResizeObserver loop completed with
      undelivered notifications` errors observed during browser tests.
- [ ] Make unexpected browser errors fail relevant UI tests.
- [ ] Add transaction date-range filtering and text search.
- [ ] Measure transaction/report performance with thousands of records; decide
      whether history needs pagination or virtualization from the results.
- [ ] Review narrow-window layouts, keyboard navigation, focus handling, and all
      themes in the installed app.
- [ ] Verify Thai fonts, translated layouts, and native platform controls in the
      installed app.
- [ ] Review first-run guidance for currency, accounts, opening balances, income,
      and the distinction between actual activity and forecasts.
- [ ] Investigate the production bundle-size warning if installed-app startup
      measurements show a meaningful performance problem.

## Distribution and documentation

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

## After the first beta

- Transaction editing with atomic balance correction and change history.
- Cloud sync and automatic transaction matching.
- Automatic income transactions and advanced recurrence rules.
- Frozen historical forecast snapshots and historical Net Worth trends.
- More advanced reports and payment-allocation workflows.
