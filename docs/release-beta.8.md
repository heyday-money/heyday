# 0.0.1-beta.8 — release candidate notes

Status: local implementation complete for Tracking and insurance; distribution is
pending the release checks in the root TODO. These notes do not announce a deployment.

## Changes

- Tracking accounts for receivables, vehicles, property, insurance cash value and
  other assets. Record valuations, lending, repayments with separate interest,
  write-offs, purchases, sales and reversals with exact monetary amounts.
- Asset history, linked-loan equity, stale-valuation reminders, confirmation
  previews, Excel export details and English/Thai interface support.
- Independent insurance policies, versioned premium schedules, explicit premium
  payments and reversals, plus links to insured assets and cash-value accounts.
  Premium forecasts can explicitly replace an existing subscription or plan.
- Tracking and insurance history are included in native backups and encrypted sync.
  Restore rejects missing logo references, invalid PNG content, mismatched dimensions
  and content hashes. Validation leaves the selected backup and current data intact.

Tracking values contribute to Net Worth, not spendable cash. Insurance coverage is
informational; paying a premium never automatically increases cash surrender value.
Scheduled payments are forecasts until explicitly confirmed.

## Upgrade requirements

Desktop version: 0.0.1-beta.8. SQLite beta migrations: 0001–0014.
Financial sync protocol: 7. Key protocol: 2; encryption derivation is unchanged.
Existing beta databases migrate additively. Alpha databases remain separate;
alpha and newer unsupported backups are rejected. Restore requires explicit cloud
reconnection and excludes credentials, encryption keys and sync ownership/queues.

Deploy Site migration 0012 and protocol-7 handlers before distributing this client.
Once a dataset accepts insurance, older clients must upgrade before syncing.
Mobile must deliberately adopt a reviewed, published core revision and pass clean
build/device checks; its current pin has not been updated by this work.

## Validation and outstanding release checks

On 2026-11-16, 13 native backup tests, 22 frontend domain tests, frontend typecheck
and 29 Site API tests passed. Backup coverage includes older beta staging, recovery,
failed restore rollback, Tracking/insurance reversal histories and corrupt logos.

Browser tests cannot start because this environment denies the Vite listening socket.
Three PostgreSQL integration suites skip without TEST_DATABASE_URL. Packaged native
pickers, key-store access, two-device HTTP sync, interruption/retry, restore/reconnect,
account switching and clear-local-data acceptance remain required. Finalize these
notes with deployment/device evidence before publishing a release.

Full native run: 163 passed; seven HTTP/loopback tests failed at socket binding
with PermissionDenied (EPERM). No other native test failed.
