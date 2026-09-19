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

Lockfiles are included. Signing, notarization, release automation, and an
open-source license choice remain to be configured before public distribution.
