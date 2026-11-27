# Tracking accounting contract

Section 1 foundation, implemented 2026-11-15 in
`crates/heyday-core/src/domain/tracking.rs`.

## Activation boundary

The shared domain module defines the rules below. Migration 0012 and the native
metadata/valuation service now implement the first persistence milestone; see
[the persistence audit](tracking-persistence.md). Desktop IPC/UI and protocol 5 do
not admit creation/sync yet. Generic account validation still rejects Tracking;
only the dedicated core service creates it. Cash-linked actions remain calculations
until their atomic ledger services are implemented.

## Account identity and metadata

The new account type is `tracking`. Subtypes are `receivable`, `vehicle`, `property`,
`insurance_cash_value` and `other_asset`. Subtype, currency and opening value/date
are immutable. Use the configured app currency, canonical decimal integer strings
for serialized money and checked integer arithmetic. Values/principal cannot be
negative; zero is valid. Name/notes follow existing account length limits.

Optional debtor/due date apply only to receivables. An overdue receivable can be
entered with a due date before its opening date. Optional existing-loan links apply
to vehicles, property and other financed assets. Insurance cash value and receivables
do not accept loan links in this first version. Logo IDs and loan IDs are references:
future services must verify existence, valid loan type, reference availability and
currency inside the database transaction. Pure draft validation cannot verify them.

Persist current value as a projection of opening value plus immutable dated events.
Latest valuation date is the opening date until a valuation event exists; repayments
must not masquerade as new appraisals. The native service now enforces increasing revisions and nondecreasing event dates;
backdated events are rejected. No frontend input can directly replace a derived balance.

## Financial classification

| Action | Asset effect | Cash effect | Reporting |
| --- | --- | --- | --- |
| Set value | New nonnegative value | None | Signed valuation gain/loss |
| Lend | Increase receivable principal | Outflow | Principal transfer |
| Receive | Reduce principal | Principal plus interest inflow | Only interest is income |
| Write off | Reduce principal | None | Realized loss, separate from cash spending |
| Buy | Initialize zero-valued physical asset at cost | Outflow | Asset acquisition, no ordinary expense |
| Sell/dispose | Remove entire positive carrying value | Proceeds, including zero | Proceeds minus carrying value is realized gain/loss |

Receivables cannot use manual revaluation; use explicit lending, repayment or
write-off. Reject principal above the outstanding amount. Interest-only receipts
are allowed; zero-total receipts are not. Buy/sell apply only to vehicle, property
and other assets. Partial disposals, fees and insurance surrender require separate
future operations. Buying requires zero carrying value; later lifecycle persistence
must prevent duplicate purchases/disposals with durable event IDs and revision checks.

Insurance cash value accepts manual valuation only. Premium payment and coverage
amount are not inputs to the asset calculation. Policy schedules/payments belong
to section 3; they never automatically increase cash value.

Calculation outputs are not commands: the future caller must validate action date,
current persisted subtype/value, currency, cash-account eligibility, reconciliation,
linked references and resulting cash balance, then commit both sides atomically.
It must not trust a caller-supplied current value/archive flag or recalculate a
remote event against an unrelated revision. Corrections/reversals need event history.

## Net Worth, cash and archiving

Active Tracking values contribute to a distinct Net Worth asset group. They contribute
zero spendable cash and zero cash forecast balance and cannot be ordinary payment or
income-destination accounts. Forecasting a receivable receipt requires an explicit
plan, separate from outstanding principal.

Loan links are informational. The existing loan account contributes its liability
once. Equity is asset value minus signed loan balance; it is never added to Net Worth
as another asset. Keep lender credit signs and wide-integer precision intact.

Archived Tracking accounts contribute zero to active totals, matching existing
account visibility. History and detail equity remain available. The archive UI warns:
“Archiving excludes this asset from Net Worth. Its history is kept. A linked loan
remains unchanged.” Archived accounts reject financial changes until restored.

## Integration and verification

The rules are wired through dedicated native IPC, Tracking forms/details, Net Worth,
cash selectors, atomic cash actions, native backup validation and protocol-6 encrypted
sync. See [persistence and verification](tracking-persistence.md) for test evidence,
compatibility requirements and outstanding release checks. Insurance policy/premium
management is implemented separately in [Insurance](insurance.md); only insurance
cash value belongs to this account.
