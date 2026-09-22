# Desktop development

The desktop repository builds independently of the website. Assets are bundled
locally and financial data is stored in SQLite.

Install Bun, stable Rust, and the platform-specific
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

```sh
bun install --frozen-lockfile
bun run desktop:dev
```

The first native build downloads and compiles Rust dependencies. Tauri starts
Vite at `http://127.0.0.1:1420` automatically.

| Command | Purpose |
| --- | --- |
| `bun run dev` | Browser UI preview without native database access |
| `bun run build` | TypeScript validation and production frontend build |
| `bun run rust:check` | Rust compilation check |
| `bun run rust:test` | Migration and data-integrity tests |
| `bun run test:e2e` | Browser navigation, theme, and sidebar smoke test |
| `bun run desktop:build` | Build native app and platform installers |

Install the test browser once with `bunx playwright install chromium`.

## Structure

Shared UI controls live in `src/components/ui` and are installed from shadcn/ui
(Radix components). `components.json` configures the CLI and `@/` resolves to
`src/`. The generated components use the app's Tailwind theme tokens and local
`cn` helper. Add Account uses Dialog with a scrollable body, visible footer, and
explicit discard for unsaved changes. All successful saves use Sonner through
one app-level Toaster, placed top-center and synchronized with the app theme.

App icons come from the Icon Composer export `Icon-iOS-Dark-1024@1x.png`.
Run `bun run icons:generate` after replacing that source to regenerate the native
PNG, ICNS, and ICO assets. This uses the flattened PNG export, not a layered
Icon Composer `.icon` bundle. The sidebar logo remains a separate SVG asset.

`src-tauri/Info.plist` sets the macOS display name to `Heyday Money`, including
the metadata embedded by Tauri during development. The bundled executable also
uses that name. Quit and relaunch after changing native icons or metadata.

- `src/`: React dashboard, TanStack routes, styling, and typed native calls.
- `public/`: bundled branding and artwork.
- `src-tauri/src/`: native startup, database setup, and Rust commands.
- `src-tauri/migrations/`: versioned SQL migrations.
- `src-tauri/tauri.conf.json`: window, build, and bundle configuration, following
  the [Tauri Vite guide](https://v2.tauri.app/start/frontend/vite/).
- `AGENTS.md`: requirements and implementation guidance.

## Current implementation

The scaffold includes a persistent collapsible sidebar, light/dark themes, Home,
Accounts, Income, and Settings routes, native icons, and SQLite initialization.
Settings reads the native database and saves the period start day, with a cycle
preview and app version below the panel. Users can choose a shared currency in
Settings and add cash, bank, credit card, loan, and investment accounts. Account
cards show persisted opening balances and type-specific details. Income supports
adding salary, variable, investment, and other sources with destination accounts,
estimated amounts, monthly schedules, and active/inactive status. Saving a source
does not create transactions or change balances. Currency changes are blocked
once financial records exist.
Currency starts unset and the period starts on day 1 (calendar month).

Next: account editing/archiving, income editing, dashboard period summaries, and safe
backup/restore. Transactions and automatic transaction creation remain TODO.

The database is `heyday.db` under Tauri's application data directory for
`money.heyday.desktop`, normally
`~/Library/Application Support/money.heyday.desktop/heyday.db` on macOS. It is
created on first launch and migrations run at startup.

Lockfiles are included. The project is licensed under Apache-2.0; see `LICENSE`.
Developer ID signing and notarization remain to be configured before public distribution.

## Cashflow planner

Outlook defaults to the English THB cashflow planner. The previous account-derived
outlook is available using **Account-Based Outlook**. Migration 0012 adds independent
planner data; it does not change ledger currency, transactions, balances, or existing
schedules. The planner can be used before setting up accounts.

The seven columns follow the payday start day in Settings. A `YYYY-MM` key identifies
the calendar month in which a cycle starts. Selecting another window only changes
visibility. Changing the payday setting updates date boundaries while retaining
entries under their original keys. Recurrence runs once per cycle; installment
counts include the first cycle and their final cycle is inclusive.

Select a name to edit item details or a bordered amount to edit just one cycle.
Explicit entries override schedules, including zero. Clearing an entry restores the
scheduled amount, if any. Schedule edits update generated figures across their
range, including past cycles, while keeping individual overrides. Deleting an item
removes its entries and changes historical planner totals after confirmation.

Opening cash is a nullable initial-cycle anchor. Carry-forward includes all cycles
between that anchor and the selected window. All financial arithmetic uses BigInt
satang, with individual persisted amounts validated as SQLite i64 values in Rust.
Completion is explicitly set by the user; amounts alone never complete a cycle.

General expenses link to transaction categories. The migration seeds ten category
names when absent and reuses existing normalized names, including archived ones.
Planner item names remain independently editable. No transaction totals are imported.
Gross income queries current Income definitions from the same SQLite snapshot. Active
THB sources with available destination accounts generate expected amounts using their
monthly recurrence within each payday cycle. Non-THB sources are excluded without
conversion. Inactive/unavailable sources generate no estimates; entered overrides stay.
Source details link to Income. Additional manual Gross income rows remain available.
Migration 0013 stores source-specific cycle overrides and removes only untouched income
placeholders, preserving customizations and entered amounts (including zero).
Estimates use current definitions even for past cycles; save an override to retain a
cycle's entered amount. Clearing it restores the estimate. No receipts are inferred.
Migration 0014 links Debt rows to loan accounts and card installment rows to the
Installments page, in the same SQLite snapshot. Loan balances are reference details,
never payment amounts. Enter a cycle's loan payment manually unless it has legacy
loan-linked schedules, which are included once in the loan row. Card schedules use
actual due dates grouped into payday cycles, including independently clamped month
ends, and stop at the final installment. Multiple plans for one card remain separate.
Unavailable accounts generate no schedule amounts. Source-specific overrides replace
one cycle's generated amount (including zero) and affect carry-forward. Linked rows
and their overrides require THB; other currencies are excluded without conversion.
Source links open Accounts or Installments; manual additions remain available.
Only untouched debt/installment placeholders are removed, preserving entered figures
and customizations. Check retained manual rows for duplicates of linked sources.
Editing a source schedule updates generated amounts; deleting it also removes its
linked planner overrides. No balances or transactions change through planner edits.
Migration 0015 connects recorded card payments. Migration 0016 labels this section
**Credit Cards** and applies consistent title case to the other planner sections and
subtotals, without changing stable IDs or financial data.
It sums cash/bank repayments and transfers into each credit card per payday cycle,
using exact BigInt arithmetic on transaction amount strings. Purchases, refunds and
debt-to-debt transfers are excluded. Archived account history stays included. Linked
card rows are read-only and link to Transactions; edits/deletions refresh the planner.
Transactions do not identify installment portions. In a cycle containing recorded
payments, the full recorded card total replaces that card's linked installment
forecasts, including cycle overrides, without changing saved entries or marking any
installment paid. Partial payments count only the cash recorded, and further expected
payments in the same cycle are not inferred. If transactions are removed, saved
installment forecasts/overrides resume. The category is renamed to avoid describing
the unsplit total as only the non-installment part. Manual rows remain additive and
must not duplicate recorded payments; only untouched default card rows are removed.
Subscriptions and one-time payment plans remain separate. No installment allocation,
paid-status matching, or automatic duplicate detection for manual rows is implemented.

Validation: `bun run build`, `cargo test --manifest-path src-tauri/Cargo.toml --lib`,
and `bun run test:e2e -- tests/cashflow.spec.ts tests/cashflow-ui.spec.ts`.
Browser tests mock Tauri IPC; Rust tests exercise actual SQLite migrations and reopening.

### Salary deductions

Migration 0017 adds reusable deductions owned by Salary income sources. Create them
with a new salary, or use **Manage Deductions** on an existing salary in Income.
Suggested blank rows cover withholding tax, social security, provident fund, payroll
loan payments, and other deductions; custom names and descriptions are supported.
Amounts are per salary payment, in the shared currency's minor units. No percentage
rates or automation are assumed. Source deduction totals cannot exceed gross salary.
A salary and its initial deductions save atomically; updates preserve stable deduction
IDs and cycle overrides. Removing a deduction explicitly removes its cycle overrides.

Outlook retains separate **Gross Income** and **Income Deductions** sections. Linked
deductions follow their salary's recurrence and account availability, use THB sources,
and are subtracted exactly once from gross income. Cycle overrides support explicit
zero and remain independent of other cycles. The section's Manage link opens Income.
The separate account-based outlook forecasts net salary received into cash accounts.
Income definitions and deductions never create transactions or change balances.

Previously entered/manual Outlook deductions remain, since the app cannot safely
assign them to a salary. Only untouched default placeholders are removed. Review old
manual deductions before adding equivalent source deductions to avoid duplication.
General income-source editing remains future work; deduction management is available
for existing salaries, including inactive salaries and archived destinations.

The planner table groups Gross Income and Income Deductions under **Income**, ending
with highlighted Net Income. **Expenses** groups debt, card installments, card
payments and general expenses, followed by Total Expenses and Total Outflows Including
Deductions. **Cash Balance** finishes with Opening Cash, Cycle Surplus / Deficit and
Cumulative Closing Cash. Tinted headers and thicker dividers separate the blocks;
negative amounts remain red in both themes. Calculations and stored data are unchanged.

### Clear local data

Settings > General includes a Danger Zone with a typed `DELETE ALL DATA` confirmation. The Rust command deletes all user financial records, planner entries, schedules, payees, and categories in one foreign-key-safe transaction, then resets currency to unset and payday start to 1. Schema, migration history, built-in Outlook sections, and appearance preferences remain. Success reloads the interface and discards drafts; failure rolls back all deletions. This action is irreversible; backup/restore remains future work.

## Tag-triggered macOS release

`.github/workflows/release-dmg.yml` runs on pushed `v*` tags. Tags must be semantic
versions, such as `v0.0.0-alpha.0` or `v1.0.0`. Commit the workflow and source first,
then create and push the tag:

```sh
git tag v0.0.0-alpha.0
git push origin v0.0.0-alpha.0
```

The workflow installs the packageManager-pinned Bun version and locked dependencies,
sets the app version from the tag in the CI checkout (including Cargo.lock), runs
Rust tests, and builds a universal Apple Silicon/Intel DMG. Tauri's build hook runs
TypeScript checks and Vite. It publishes the DMG on the tag's GitHub Release and also
keeps a workflow artifact for 30 days. Prerelease tags are marked as prereleases.
Only the built-in GITHUB_TOKEN with contents-write permission is needed.

Builds use ad-hoc signing, without Apple notarization; macOS may require approval
in Privacy & Security. Developer ID signing/notarization remain future work. Signed in-app updates
require the one-time key setup described below. No local tags or releases are created by implementing this workflow.

Based on the [Tauri GitHub Actions guide](https://v2.tauri.app/distribute/pipelines/github/).

### Local date and development storage

The WebView reads the computer's local date (no network clock). Outlook's default
window follows the current payday cycle, refreshing every 30 seconds and on focus
or visibility changes. Explicitly selected cycles remain selected; Current cycle
returns to following the computer date. Settings shows the computer date.

Debug builds (`bun run desktop:dev`) use `development/heyday.db` under the app data
directory. On macOS this is
`~/Library/Application Support/money.heyday.desktop/development/heyday.db`.
Release builds retain `~/Library/Application Support/money.heyday.desktop/heyday.db`.
No existing database is moved or copied; development starts with a separate empty
database and runs the same migrations. `tauri dev --release` is a release build
and uses production storage; use the normal debug dev command for isolation.

## In-app release checks and signed updates

Settings > General > App Updates checks public releases from
`heyday-money/heyday` only when requested. It selects the highest newer semantic
version among the 100 most recent GitHub releases, ignoring drafts and invalid
tags. Stable builds exclude prereleases; alpha builds include them. Network
failures and GitHub rate limits are shown with a retry action. Financial features
remain offline. Development builds may check but cannot install updates.

Installation uses Tauri's updater to verify signatures, then creates a consistent
SQLite `VACUUM INTO` snapshot in the app data directory's `backups/` folder before
replacing the app and restarting. A failed download, signature, or backup stops
installation. The existing database path and app identifier are unchanged.
Snapshots are retained; restoration is currently manual. Save open drafts before
installing. Startup continues to apply the existing versioned SQLx migrations.

### One-time signing setup

Generate and securely retain a Tauri updater signing key on your own computer:

```sh
bun run tauri signer generate -w "$HOME/.tauri/heyday-updater.key"
```

In GitHub repository Settings > Secrets and variables > Actions, configure:

- Variable `HEYDAY_UPDATER_PUBLIC_KEY`: contents of the generated `.pub` file.
- Secret `TAURI_SIGNING_PRIVATE_KEY`: contents of the private key file.
- Secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: its password, if used.

Never commit the private key. Keep the same key for future releases and retain
an offline backup. This signing key is separate from Apple Developer ID signing.

The tag workflow injects the public key into the native app and enables updater
artifacts only when both public/private keys are configured. It builds `app,dmg`;
tauri-action uploads the signed app archive, signature, and `latest.json` as well
as the DMG. Each release's own manifest supports alpha updates without relying
on GitHub's stable-only latest release URL. Missing keys leave DMG releases working
but disable in-app installation. A partially configured key pair fails CI early.

Existing published apps without the updater must first be upgraded manually to a
build containing this feature and your public key. A real signed release-to-release
installation still needs verification on an installed macOS app; mocked browser
tests do not exercise native app replacement or Apple Gatekeeper.

Reference: https://v2.tauri.app/plugin/updater/
