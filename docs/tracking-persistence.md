# Tracking implementation

Implemented locally on 2026-11-16 for desktop 0.0.1-beta.8. Root TODO section 1
and the native operations in section 2 are integrated. Insurance cash value is
supported. Separate insurance policies and premium schedules are now implemented;
see [insurance behavior and verification](insurance.md).

## Storage and accounting

Migration 0012 expands the account CHECK with a guarded SQLite schema-text change,
preserving account IDs and children. Migration 0013 adds cash actions, owned ledger
entries and sync triggers. Released migrations 0001–0011 are unchanged.

Each Tracking account owns immutable subtype/opening value/date, optional logo,
notes, debtor/due date and linked loan, plus revisioned events. Events replay exact
minor-unit strings with checked Rust arithmetic. Values and principal are nonnegative.
Dates are nondecreasing; future/backdated actions and stale revisions are rejected.
Identical event retries do not post twice. Reversal unwinds the latest active action.
A disposal must be reversed before another asset action.

Lending, repayment, buying and selling update cash and the asset in one transaction.
Principal is a transfer; interest is separate income. Valuation changes, write-offs
and disposal gains/losses stay in asset history rather than ordinary spending/income.
Sales use net proceeds; additional fees can be ordinary expenses. Partial disposals
are deferred. Owned transactions cannot be edited/deleted through ordinary history.
Reconciled cash legs block group reversal or conflicting replacement.

Net Worth counts the active asset once, and the linked loan once as a liability.
Displayed equity is informational. Tracking is excluded from spendable cash,
cash opening forecasts and ordinary transaction selectors. Archive retains history
and warns that a linked loan remains unchanged.

## Backup and restore

Native backups include Tracking tables, cash transactions and referenced logos.
Validation replays events and verifies exact cash-leg ownership, amounts, dates and
references. Staged restore retains original files and recovery copies, strips cloud
ownership/queues, and requires explicit reconnect. Older beta backups migrate;
alpha and newer unsupported backups remain rejected.

## Encrypted sync and compatibility

Protocol 6 uses an encrypted metadata:tracking aggregate containing account metadata
and its complete event stream. Cash legs are derived atomically, never uploaded twice
as generic ledger records. Cash accounts, linked loans and content-hash logo records
are dependencies. Full-financial consent is required in a connected database.
Competing edits require explicit conflict choice, preserving the local financial state
until resolved. Reconciliation protections also apply to remote replacement.

Existing encryption derivation, durable ciphertext retries and authenticated history
are unchanged. Local history is bounded before saving to fit the encrypted record
limit. Service migration 0011 and protocol-6 support must deploy before clients sync
Tracking. Once a dataset accepts protocol 6, older clients receive upgrade guidance.
Mobile keeps its pinned core until deliberately upgraded; no silent adoption.

## Verification

87 shared-core tests pass, including exact arithmetic, overpayment rollback, cash
overflow, disposal/reversal, reconciliation protection, migration preservation,
encrypted two-database repayment/retry/reversal and concurrent valuation conflicts.
160 native tests pass (seven socket-dependent tests excluded), including eleven backup tests, including Tracking lending/repayment/reversal round
trip and preservation of the original backup. Seventeen frontend financial/domain
tests, two translation checks, desktop build, Site typecheck and 28 API tests pass.

Browser tests are added but blocked by local socket EPERM. Native HTTP/loopback
tests have the same restriction. PostgreSQL suites require TEST_DATABASE_URL and
Docker socket access is denied. Packaged two-device, key-store, HTTP and deployment
acceptance remain release work. No original user database was opened or modified.
