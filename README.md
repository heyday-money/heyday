<p>
  <h1 align="center">Heyday Money — Your Money, Your Heyday</h1>
  <p align="center">
    Money sucked for years. Not anymore — we fixed it and made it grow.
    <br />
    <a href="https://heyday.money"><strong>Learn more »</strong></a>
  </p>
</p>

<div align="center">

[![CI](https://github.com/heyday-money/heyday/actions/workflows/ci.yml/badge.svg)](https://github.com/heyday-money/heyday/actions/workflows/ci.yml)
[![Build macOS DMG](https://github.com/heyday-money/heyday/actions/workflows/release-dmg.yml/badge.svg)](https://github.com/heyday-money/heyday/actions/workflows/release-dmg.yml)
[![Build Windows installers](https://github.com/heyday-money/heyday/actions/workflows/build-windows.yml/badge.svg)](https://github.com/heyday-money/heyday/actions/workflows/build-windows.yml)

</div>

## What is?

Heyday Money is an open-source personal finance tracking and simulation platform, designed for those seeking financial independence with a simple goal: to turn every penny into its heyday. Built on a foundation of data security and user control, Heyday Money enables users to track expenses, monitor investments, and achieve financial clarity—all securely on their local machines.

With our slogan, "Don't let money control you," we empower users to take full charge of their finances. The platform offers transparent, customizable tools that provide data-driven insights for mindful spending, saving, and investing, all while maintaining complete control over personal data.

Heyday Money combines practical finance tracking with a powerful simulator to guide users toward making the most of their financial resources, fostering wealth growth and long-term stability.

## Beta database reset

Version `0.0.1-beta.1` starts with a fresh database. Alpha was the MVP stage;
its database and backups are not compatible with the beta schema history.
Your alpha files remain untouched, but their records will not appear in beta.
Subsequent beta and v1 updates preserve the new database through versioned migrations.
See [database storage and migrations](docs/development.md) for file locations.

## Manage your data

Open **Settings → Data** to export accounts to Excel, back up or restore your
database, or clear app data. Excel exports include active, archived, and paid-off
accounts with their balances and details. Amounts are decimal text to preserve
exact precision; transaction history is not included. Use **Backup & restore**
for a complete, restorable copy of your data. Both exports work locally and offline.

## Connect to a development API

Cloud sign-in is optional; local finances work without an API server. The desktop
app defaults to **`https://heyday.money`**. Starting Docker does not change that
address: `HEYDAY_API_URL` is compiled into the Rust app and must be set when
building or running the native development app.

For a local API, follow the server setup in `site/README.md` in the separate site
checkout. In the shared workspace, start the configured stack from `site/`:

```sh
docker compose --env-file .env.compose up --build -d --wait
curl -i http://localhost:3000/api/auth/get-session
```

Without a session cookie, the auth check should return HTTP `200` with JSON `null`.
The server needs its auth environment variables and completed migrations; a
working homepage alone does not confirm that sign-in is ready.

For desktop development on the same computer, set the server's
`BETTER_AUTH_URL=http://localhost:3000`. From the **desktop checkout**, with the
[native development prerequisites](docs/development.md) installed, run:

```sh
bun install --frozen-lockfile
HEYDAY_API_URL=http://localhost:3000 bun run desktop:dev
```

Open **Settings → Heyday account → Sign in**. The browser should open your local
server. HTTP localhost is allowed only in debug builds. `bun run dev` alone starts
the frontend preview, not the native desktop app.

For a shared server, mobile testing, or a release build, use a trusted HTTPS
origin. Set the server's `BETTER_AUTH_URL` and the app's `HEYDAY_API_URL` to the
same origin, with no `/api` path:

```sh
HEYDAY_API_URL=https://api.your-domain.com bun run desktop:dev
# For a packaged release instead:
HEYDAY_API_URL=https://api.your-domain.com bun run desktop:build
```

Replace the example hostname with your deployed API or HTTPS reverse proxy/tunnel
to the local server. Recreate the Compose service after changing its environment
(run from `site/`):

```sh
docker compose --env-file .env.compose up -d --force-recreate service
```

If sign-in still opens `heyday.money`, stop the previous desktop dev process and
rerun the command with `HEYDAY_API_URL` set. Launch the newly built app; an existing
installed release keeps the address it was built with. Setting the variable only
when launching an already compiled executable does not change its API address.

If the browser displays `UNAVAILABLE` / “Sign-in is temporarily unavailable,” the
server responded with HTTP `503`. Check the server's required auth variables,
database connectivity, migration results, and logs for the response's `requestId`.
In Compose, use `docker compose --env-file .env.compose logs --tail=100 migrate service`
from `site/`. Successful sign-in does not automatically enable encrypted sync;
device registration, encryption setup/recovery, server-approved access, and sync
consent remain separate steps.

## Philosophy of Money and Finance
[Standford Encyclopedia of Philosophy — Philosophy of Money and Finance](https://plato.stanford.edu/entries/money-finance/)


> **Note:**
> `Heyday(noun)/ˈheɪdeɪ/:` The time when somebody/something had most power or success, or was most popular

## License

Heyday Money is licensed under the [Apache License 2.0](LICENSE).


## Tracking accounts (beta.8)

Choose **Accounts → Add account → Tracking** for a receivable, vehicle, property,
insurance cash value or other asset. Set the opening value/date and optionally a
logo, debtor/due date or linked loan. Account details provide lending, repayment,
write-off, valuation, buying/selling and latest-action reversal as appropriate.

Tracking contributes to Net Worth without adding spendable cash. A linked loan
remains a separate liability; displayed equity is informational. Insurance accounts
hold cash surrender value only; coverage and premiums do not increase that value.
Policy and premium scheduling are available under Insurance.

Tracking history and linked cash entries are included in local backup/restore and
encrypted financial sync. Protocol-6 service support must deploy before cloud rollout;
older clients need an upgrade after a dataset starts using Tracking. See
[implementation and verification](docs/tracking-persistence.md) for release gates.


## Insurance policies

Use **Insurance → Add policy** for coverage details, Tracking links and premium
schedules. Confirm actual payments to record expenses; reverse them from policy
history. Explicitly replace an existing subscription or expense plan to avoid
duplicate forecasts. Coverage never changes Net Worth and premiums never change
cash surrender value. Policies and payment history are backed up and encrypted
with protocol 7. See [insurance rules and verification](docs/insurance.md).
