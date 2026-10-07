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
- `src-tauri/migrations/`: beta baseline and future versioned SQL migrations.
- `src-tauri/tests/fixtures/alpha-migrations/`: frozen alpha SQL used only by regression tests.
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

Starting with `0.0.1-beta.1`, the release database is `v1/heyday.db` under
Tauri's application data directory for `money.heyday.desktop`, normally
`~/Library/Application Support/money.heyday.desktop/v1/heyday.db` on macOS.
Beta deliberately starts empty; alpha was the MVP data format. Existing alpha
databases and backups are not moved, copied, overwritten, or deleted. Alpha
backups cannot be restored into beta, and restore explains this explicitly.

`src-tauri/migrations/0001_initial.sql` creates the complete current schema and
bundled reference records in one migration. Currency starts unset and no personal
records or balances are seeded. Future releases must append migrations rather
than rewrite this baseline. The `v1` storage directory is a stable schema
generation, not an app version: subsequent beta and v1 releases reuse it.
The 40 alpha migrations remain only as test fixtures for schema equivalence and
historical regression checks; they are not part of application startup.

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

Debug builds (`bun run desktop:dev`) use `development/v1/heyday.db` under the app
data directory. On macOS this is
`~/Library/Application Support/money.heyday.desktop/development/v1/heyday.db`.
Release builds use `~/Library/Application Support/money.heyday.desktop/v1/heyday.db`.
Alpha files remain at `heyday.db` and `development/heyday.db` under the same app
data directory, including their sibling backup folders. No existing database is
moved or copied; both beta environments start empty and run the same baseline. `tauri dev --release` is a release build
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

- Repository variable `HEYDAY_UPDATER_PUBLIC_KEY`: contents of the generated `.pub` file.
- Secret `TAURI_SIGNING_PRIVATE_KEY`: contents of the private key file.
- Secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: its password, if used.

Use repository-level settings. This workflow does not select a GitHub environment,
so environment-scoped variables and secrets are unavailable.

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

## Salary deductions linked to debt

Migration 0018 adds an optional debt account reference to each salary deduction.
Income > salary creation or Manage Deductions allows linking an active loan or
credit card, including a Student Loan. The salary schedule supplies the deduction
amount in Outlook; the link is shown in its help tooltip. No extra Debt Payments
entry, ledger transaction, or loan balance change is generated. Enter payroll
repayments only under deductions; any separate Debt Payments amounts must represent
additional payments. Use **Record salary payment** in Income to confirm actual receipts and repayments (see below).

Existing deductions migrate with no link and keep IDs, amounts, and cycle overrides.
Archived linked accounts remain visible and may be retained or unlinked; new links
require active debt accounts. Invalid links fail the entire save. Unlinking preserves
the deduction and its overrides. Removing a deduction never removes its debt account.

The salary form labels deductions as amounts per month. Loan rows linked to active
salary deductions are labeled `Additional Payments` in Outlook; their help text
points payroll amounts back to Income Deductions. Existing separate cycle entries
are preserved as additional payments. Saved schedules remain unchanged; linked payroll deductions cover their forecast first, with only the positive remainder counted as separate cash payments.


## Confirmed salary payments (beta.2)

Income offers **Record salary payment** and **Salary payment history** for salaries.
Choose the salary occurrence month and actual payment date, confirm gross pay and
all deductions, and enter interest/fees for each linked loan. The remainder is
principal. Contracts require explicit selection when there is more than one.
The bank receives only net salary. Loan principal reduces outstanding debt without
a second bank withdrawal. Withheld interest, fees and other deductions are saved
in the payslip breakdown, not posted as extra bank expenses.

Migration `0002_salary_payments.sql` appends immutable payslip snapshots and ledger
links to the unchanged beta.1 baseline. Principal uses the ledger's debt inflow
operation and a payroll role; it is displayed as Payroll repayment and excluded
from Home income received. The same database transaction writes net receipt,
principal repayments, contract balances and the snapshot. Zero net pay is supported.
No actual transaction is generated by changing salary definitions or forecasts.

One salary occurrence can be recorded once. Existing linked ordinary income in the
payment month blocks recording; reverse it before recording the payslip. Unassigned
receipts cannot be matched automatically and require user review. Subsequent ordinary
receipts for an already recorded salary month are also rejected. Deleting any linked
transaction reverses the entire salary payment. Reconciled entries require explicit
confirmation; their evidence remains with a needs-review marker. Re-record to correct
a payslip. Definitions, forecast overrides and saved installment amounts are untouched.

Loan accounts now support **Transactions & reconciliation**, using positive debt
and negative overpayment conventions. Payslip details can be opened from transaction
history or Income, including account-filtered payroll history for interest-only
payments. Actual Outlook cashflow remains based on net bank receipts; gross and
withheld details live in Salary payment history and are never deducted a second time.

The THB planner subtracts effective linked payroll deductions (including per-cycle
zero overrides and clamped recurrence) from generated loan installments/contracts.
Coverage is capped at the scheduled amount per cycle. Explicit debt cycle entries
still replace the uncovered estimate as additional payments. Carry-forward uses the
same rule. The account-based outlook covers scheduled loan repayments using its
salary definitions, consistent with its existing net-income forecasts; independent
manual payment plans remain additive. Review unlinked manual duplicates.

Payroll tables participate in backup, restore and Clear all data. Beta.1 backups
upgrade only in a staged copy before restore; original backups remain unchanged.


Salary-linked loan details show **Deducted from salary** beside the monthly
installment, with a link to manage deductions in Income. The account editor keeps
the saved installment read-only while any salary deduction links to that account,
including inactive salaries and zero deductions. Rust rechecks the link when saving
and rejects installment changes; other account fields remain editable. Removing
all salary deduction links unlocks the field. No amounts or balances are rewritten.


Income displays one source per row in the shared semantic data table, with source
type/status, estimated gross, deductions, estimated net, destination and schedule.
Salary rows keep Record payment visible; Edit is an accessible icon and the More
actions menu contains deduction management and payment history. The table scrolls
horizontally on narrow windows without widening the page. Draft protections and
exact monetary formatting are unchanged.


Account-Based Outlook review: this view projects active cash/bank/wallet balances
from the current payday cycle through six future cycles. Standalone loan monthly
installment estimates and Cashflow Planner cycle overrides are not dated cash plans
and are not imported. Ordinary income/schedule payments are not automatically
matched to early transactions; Card Billing uses explicit allocations. Missing debt
plans are checked per debt account, so planning one card does not hide an unplanned
loan. Known repayment amounts remain visible alongside the partial-forecast warning.


Home includes a Repayment Calendar above Needs attention for the current calendar month with previous,
next and current-month navigation. Active credit cards and loans show recurring
reminders from their saved payment due day. Each date shows institution/account icons
with accessible names and hover/focus tooltips linking to account details. Days beyond
month-end clamp independently; no weekend/holiday adjustment, paid-status inference,
statement matching or forecast changes occur. Missing due days are listed for setup.
Archived/paid-off accounts are excluded. The calendar refreshes through the existing
financial snapshot events and local date rollover, with English and Thai labels.


Subscription providers (beta migration 0003) are reusable records managed in Settings.
A starter catalog of 12 services ships with local monogram icons: Netflix, Spotify,
YouTube Premium, Apple Music, iCloud+, Google One, Microsoft 365, Disney+, Prime Video,
Adobe Creative Cloud, Dropbox and Canva. No network or external logo service is used.
Users can add and rename providers, upload a reusable logo, restore bundled icons,
and archive/restore providers. Names are whitespace-normalized and unique including
archived records. Icons inherit live through the provider link; subscription names
remain user-entered. Existing subscriptions are left unlinked, preserving logos,
management metadata, amounts, dates and balances.

Add/Edit subscription offers an optional provider selector. An explicit selection
fills the editable name and uses the provider icon without an upload; Custom
subscription retains manual name/logo entry. Archived providers cannot be newly
selected but existing links remain editable. Management platform/link, price and
billing schedule are independent per subscription. Provider forms protect drafts
and failed saves. Logo pruning retains provider assets; backups include catalog
edits and links. Clear all data removes custom providers and restores the seed list.


Settings uses a vertical section sidebar with icons and a selected-state highlight,
with content on the right and a wider 1080px page limit. On narrow content areas the
vertical list stacks above the panel. Radix vertical keyboard navigation uses Up/Down;
switching sections continues preserving unsaved General settings.

Settings now shares one rounded outer panel across navigation and content, with no
column gap and a full-height sidebar divider. Content padding is owned by that
panel, so Payees/Categories descriptions and every section remain inside its body.
Descriptions wrap, fieldsets shrink, and controls reflow at narrow widths or larger
text sizes.


Home's Payment Calendar combines repayment due-day reminders and active subscription
billing occurrences. All/Repayments/Subscriptions filters affect only calendar
visibility. Subscription icons link to Subscriptions, with amount and payment account
in the tooltip. Monthly/yearly schedules clamp independently and respect first/end
dates, pauses, and account availability. Credit-card subscriptions remain visible as
billing reminders without adding cash expenses or inferring payment status.
