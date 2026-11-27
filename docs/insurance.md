# Insurance policies and premiums

Implemented for desktop beta.8 in root TODO section 3. Open **Insurance** to add a
policy, set coverage and renewal dates, link Tracking assets, and schedule premiums.
The asset detail page links to its policies. Insurer, policy type, coverage,
deductible, notes and lifecycle state are independent of account balances.
Only up to four trailing policy-number characters are stored. The full number is
not collected. Coverage and premiums never increase a cash-value asset.

## Schedule and payment rules

Premiums support one-time, monthly, quarterly and yearly schedules. Each occurrence
clamps independently to month-end from the original anchor, including leap-day
yearly schedules. Coverage dates and schedule end dates are inclusive. Renewal dates
are reminders; they do not renew coverage or create payments automatically.

Schedules are immutable versions with strictly increasing effective dates. A new
version replaces unrecorded occurrences on and after its effective date; older dates
keep their previous schedule. Local changes take effect today or later and must
follow every recorded occurrence, including reversed history. Metadata edits do not
create a new schedule. Same-day schedule versions are rejected rather than overwriting
history. To stop premiums, cancel or expire the policy; the status date excludes
occurrences from that date onward. Coverage expiry stops future occurrences, with
earlier unpaid premiums retained. Archive hides forecasts and blocks payment/reversal
until restored. Reactivating resumes the retained schedule; no cash posts automatically.

An occurrence is identified by policy, schedule version and due date. Confirming a
full premium creates one expense on its paying cash/bank/wallet/card account; cards
increase debt rather than reduce cash. The payment date is explicit and cannot be
future. Early payments are allowed. Amount/account/category come from the chosen
schedule. Duplicate submissions reuse the stable action ID; a second active payment
for the same occurrence is rejected. Failed writes roll back the entire change.

Reversal retains the premium record and reversal ID and reverses its cash/debt effect.
The occurrence becomes unpaid again. Ordinary transaction editing/deletion and
generic ledger sync cannot mutate these owned transactions. Reconciliation history
blocks reversal and conflicting replacement. Partial premiums, refunds and provider
billing automation are outside this implementation.

## Forecasts and source replacement

Unpaid premiums appear in Account-Based Outlook, the THB cashflow planner and the
Payment Calendar. Cash forecasts exclude archived or non-cash paying accounts.
Card premiums are calendar reminders; card repayment planning remains separate.
Past unpaid premiums are shown as overdue in Insurance, not added to today's cash.

Explicitly selecting a subscription, dated expense plan or manual expense planner
row replaces that source's forecast with the policy schedule. One source can belong
to one policy. Enter the intended replacement amount/dates/account in Insurance;
no matching or copying is inferred. The original record, overrides and historical
transactions remain intact. While linked, its forecasts are suppressed, including
historical Forecast views; Actual views retain recorded transactions. Unlinking
restores the original forecast. Archiving/cancelling a policy does not silently
unlink it and resurrect a replaced forecast. The form explains these effects.
Existing recorded expenses are not automatically matched: the payment confirmation
requires that the premium has not already been recorded elsewhere.

## Storage, backups and encrypted sync

Additive SQLite migration 0014 stores bounded policy aggregates plus FK-protected
asset/account/category/source links and owned expense entries. A cash-value account
can be attached to only one policy; insured asset links may be shared. Net Worth
still counts accounts only. Reset clears policy children before their dependencies.

Backup validation checks policy identity/currency, schedules, exact monetary strings,
payment/occurrence ownership, transaction amounts/dates/categories and every reference.
Backup image validation also checks PNG decoding, bounded dimensions and matching
content hashes, while foreign-key validation rejects missing referenced logos.
Native staged restore preserves source backups, recovery copies, stable IDs and full
reversal history. Credentials, keys, bindings, cursors, pending ciphertext and queues
remain excluded. Reconnect/merge remains explicit.

Protocol 7 encrypts each policy, schedules and payment/reversal history as one
aggregate. Apply derives expenses and balances atomically. Account, category, Tracking
asset and replacement-source dependencies arrive first. Bootstrap excludes owned
premiums from generic ledger uploads. Concurrent policy/payment edits require explicit
conflict choice; selecting a version updates its cash effects under reconciliation
guards. Encryption, authenticated history, key ownership and durable retries remain
unchanged. Full-financial sync consent is required in connected databases, and the
policy form explicitly states that insurance details enter backups and encrypted sync.

Deploy Site migration 0012 and protocol-7 handlers before distributing this client.
Once a dataset accepts insurance, protocol-6 and older clients receive upgrade guidance.
Mobile retains its pinned core until deliberately upgraded. No service deployment
or mobile dependency change was performed.

## Verification (2026-11-16)

- 96 shared-core tests pass, including eight insurance domain/storage tests and
  encrypted bootstrap/payment/reversal/conflict scenarios.
- 161 native tests pass, including 12 backup tests and an insurance round trip with
  source replacement, payments, reversal, bank balance and original-file preservation.
  Seven HTTP/loopback tests are excluded because the sandbox denies listening sockets.
- 20 frontend financial/domain tests and two translation checks pass. Tests cover
  leap dates, schedule boundaries, payment suppression/reversal, both Outlook views,
  source replacement and unchanged Net Worth.
- Desktop typecheck/build and Site typecheck pass; 29 Site API tests pass.
- Browser create/pay/reverse and dirty-draft tests are added but blocked by Vite
  listen EPERM. Three PostgreSQL suites skip without TEST_DATABASE_URL. Protocol-7
  service acceptance/retry/older-client scenarios are included but not executed here.
- Packaged two-device/key-store/HTTP QA and coordinated service rollout remain release
  gates. Automated in-process sync tests do not substitute for those checks.
