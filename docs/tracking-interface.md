# Tracking and insurance desktop interface

TODO section 4 is implemented. Existing account tabs, sidebar totals, Tracking
forms/actions/history and insurance screens remain integrated with native operations,
backup/restore and encrypted sync. No schema or protocol change is needed here.

## Behavior

- Tracking uses current value labels and its own account/Net Worth group. Net Worth
  links open account details. Coverage is excluded; recorded insurance cash surrender
  value counts once. Linked-loan equity is informational, never a second balance.
- Asset details show a reminder when the latest unreversed valuation or purchase
  (opening date as fallback) is at least 90 calendar days old. This is a review
  prompt, not automatic depreciation. Receivables, archived and disposed assets are
  excluded. Dates use calendar days, avoiding daylight-saving boundary changes.
- Confirmations preview resulting value, cash change and the affected cash balance.
  Reversals preview both principal and interest returning together. Rust remains
  authoritative for stale revisions, limits, reconciliation and atomic posting.
- Transaction filters show recorded cash legs; an explanation links to asset history
  for non-cash valuations/write-offs. Planned receivable dates do not create forecast
  income or spendable cash. Ordinary payment pickers continue excluding Tracking.
- Linked-payment guidance directs users to the originating asset/policy or other
  financial record for reversal rather than suggesting ordinary transaction editing.
- Excel export adds subtype, opening/valuation dates, debtor/due date, linked loan
  ID/name/balance and informational equity. Latest dates ignore reversed valuations.
  Decimal text and i128 equity arithmetic preserve exact values, including lender
  credits beyond i64. All rows retain active/archive status and one consistent
  database snapshot. Read me explains double-counting and backup limitations.
- English/Thai copy, labelled native fields, Radix keyboard dialogs and discard
  protections are retained. History scroll regions can receive keyboard focus;
  policy cards wrap long content; account headers wrap on compact windows. Failed
  Tracking-form loads offer Retry, including new accounts.

## Verification — 2026-11-16

- 22 frontend domain tests passed, including calendar-day warning boundaries,
  exact confirmation amounts, principal-plus-interest reversal, existing Net Worth
  exclusions and insurance forecast deduplication.
- Two translation/locale checks passed; desktop typecheck and production build passed.
- Four native export tests passed, including exact equity, dates after native
  valuation/reversal, debtor links, existing statuses/shared limits and XLSX strings.
- Added compact-window keyboard/review/filter regression coverage to
  `tests/tracking-assets.spec.ts`. Attempted Tracking and Insurance browser suites;
  Vite failed to bind 127.0.0.1:1420 with EPERM before launching a browser. Therefore
  browser layout, focus and packaged native picker/Excel checks remain unverified.
  Existing packaged backup/sync/service rollout gates in root TODO are unchanged.
