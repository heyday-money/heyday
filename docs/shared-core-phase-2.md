# Shared core: Phase 2 domain extraction

Phase 2 builds on the uncommitted Phase 1 changes. Desktop baseline remains
`9a0e25915c1e3c960f0525dccc355f6949416959`; see shared-core-phase-1.md.

## Moved rules and dependencies

- `domain::transactions`: `NewTransaction` and input-only validation, preserving
  serde's `type` rename, optional fields, default cleared-account IDs, error text,
  and validation order. Desktop re-exports the type for existing callers.
- `domain::money`: payroll nonnegative i64 parser and checked account balance
  adjustment. Debt inflows reduce amounts owed; asset inflows increase balances.
  Wide edit deltas remain i128 and the final stored balance must fit i64. Public
  extreme i128 inputs now fail with the existing range error instead of overflowing.
- `domain::payroll`: `Deduction`, `PaymentInput`, confirmation/month/date/gross
  validation, principal allocation, checked deduction totals and net salary.
  Arithmetic functions consume already validated nonnegative minor-unit amounts.
  Desktop retains stale-reference checks in their original transactional order.
- `domain::dates`: existing strict date validator and independently clamped monthly
  scheduling function. Their two existing unit tests move with them. The scheduling
  function retains its existing valid-date/range preconditions.

No SQL writes, operation guards, migration bytes, transaction grouping/reversal,
clock reads, credentials, sync protocol, or Tauri IPC names move or change. Existing
serialization retains decimal strings, including amounts beyond JavaScript's safe
integer limit. `serde` is the only runtime dependency; `serde_json` is test-only.
Existing licenses are retained in the crate.

## TypeScript parity inventory

| Existing calculation | Phase 2 treatment |
| --- | --- |
| `money.ts`: decimal input and currency precision | Core counterparts with shared vectors for all 15 supported currencies, 0/2/3 decimal places, four-place interest input, i64 limits, invalid precision and negative zero. Excess precision is rejected, never rounded. Formatting and Intl localization remain frontend-owned. |
| `period.ts`, `financial.ts`: monthly dates and payday boundaries | Core current-period counterpart accepts an explicit local YYYY-MM-DD date and produces inclusive start/exclusive end. Shared vectors cover leap/century years, days 29–31 and year rollover. No platform clock/timezone dependency. |
| `financial.ts`: cash/debt classification, signed economic value, net worth | Core economic account-value helper and shared vectors cover every account type, overdrafts, debt credits and exact i64 extremes. Aggregate BigInt net worth and report assembly remain TS; existing tests still run. |
| `SalaryPaymentDialog.tsx`, `SalaryDeductions.tsx` | Draft previews remain BigInt and formatting remains local. Native posting now uses core payroll rules; core tests cover split/net vectors, zero-net salary and overflow. Database tests retain ownership, duplicate receipt, group posting and reversal checks. |
| `cashflow.ts`, `home.ts`, `expenses.ts`, `card-overview.ts`, `card-billing.ts`, `card-limits.ts` | Report/forecast aggregation remains frontend-owned until shared database/services supply canonical snapshots in Phase 3. Do not replace estimates with postings. |
| `installments.ts`, `subscriptions.ts`, `loans.ts`, `repayment-calendar.ts`, `selective-defaults.ts` | Schedule expansion and forecast exclusions remain TS; existing period/financial tests preserve their interactions. Monthly clamping is now available in core. |

Core decimal conversion supports explicit precision 0–4; unknown currency precision
returns None rather than guessing. Rust payday calculations reject dates outside
years 1–9999, matching stored date limits. These new core helpers do not replace
frontend presentation code or add IPC commands.

The single fixture source is `crates/heyday-core/tests/fixtures/domain.json`, included
in the packaged crate and consumed by Rust integration tests and Playwright domain
tests. `bun run test:domain` runs pure tests without a development server or browser,
so shared parity checks can run inside the restricted environment. Normal browser
CI still discovers these tests as part of the complete suite.

## Validation

- Core: 2 moved calendar tests and 4 new integration tests pass.
- TypeScript: 16 pure financial/domain tests pass; frontend and native desktop builds pass.
- Packaged external consumer compiles and packaged tests pass without Tauri/native dependencies.
- Release helper passes; migration files remain byte-identical to the baseline.
- Full desktop Rust: 212 passed, same 7 sandbox EPERM socket failures as baseline.
  Together with the 2 moved tests, the original 214 passing tests are preserved.
- Full browser suite still requires a local listening socket, blocked by this sandbox
  in Phase 1. Rust 1.89 execution and macOS/Windows CI remain external verification;
  all resolved core dependencies declare an MSRV no higher than 1.89.

Coverage accounting: desktop loses exactly the two calendar unit tests now run in
core. No database, crypto, sync, backup or reversal regression tests were removed.
