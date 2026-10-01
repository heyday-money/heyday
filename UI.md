# Alpha.7 UX/UI Improvements

Target release: `v0.0.1-alpha.7`.

**Status: Complete — UX/UI implementation finished on 2026-10-01.**
Native verification listed below remains a release QA follow-up.

This release focuses on appearance, typography, and interface consistency.
All three appearances—Light, Dark, and Heyday—are required in this version.
Financial behavior and calculations remain governed by AGENTS.md.

## Design direction

Give the app the clarity and precision of a trading app: crisp typography,
compact financial tables, aligned numbers, restrained decoration, and clear
information hierarchy. Preserve Heyday's rounded surfaces and large Home island
hero. Avoid introducing market tickers, price charts, or trading features merely
to achieve the visual style.

## Required: three appearances

| Appearance | Surfaces | Actions and selection | Character |
| --- | --- | --- | --- |
| Light | White with subtle light-gray separation and borders | Charcoal primary buttons, neutral selection and hover states | Clean, precise, quiet |
| Dark | Neutral charcoal canvas with slightly lighter neutral cards | Light neutral primary buttons with dark text; neutral selection and hover states | Calm and focused |
| Heyday | Warm off-white canvas, white cards, soft lavender highlights | Purple primary buttons and restrained yellow accents | Distinct Heyday identity |

Heyday is a complete branded light preset for alpha.7. Dark is a complete neutral
preset. Both must ship alongside Light; neither is deferred.

- [x] Define theme-aware tokens for canvas, surfaces, text, muted text, borders,
      primary actions, hover, selection, and focus rings.
- [x] Remove fixed purple/yellow treatment from neutral Light and Dark interface
      controls. Retain original colors in logos and artwork.
- [x] Keep positive amounts green, negative amounts red, and zero neutral in all
      appearances. Preserve signs and explicit status labels.
- [x] Keep warnings, errors, and success states distinguishable from brand accents.
- [x] Use dark text on yellow surfaces and verify text contrast in every preset.
- [x] Apply the preset consistently to sidebar, header, cards, tables, dialogs,
      popovers, tooltips, forms, and the app-level Sonner toaster.
- [x] Keep the dark Heyday logo in all appearances.
- [x] Preserve existing saved Light/Dark choices. Use Heyday as the
      default for new installations; do not overwrite an existing preference.
- [x] Keep styling in Tailwind utilities and theme tokens; do not introduce custom
      component classes in `src/styles.css`.

## Required: visual appearance samples

Replace the current theme dropdown with three selectable sample cards in
Settings → General → Appearance.

- [x] Each sample shows the same miniature app composition: sidebar, heading,
      balance card, table rows, and primary action.
- [x] Render each preview in its own preset regardless of the currently selected
      appearance, so all three samples remain comparable.
- [x] Label the options Light, Dark, and Heyday. Show a checkmark and clear border
      for the selected option; do not rely on color alone.
- [x] Apply and persist the selection immediately, independently of Save period.
- [x] Support radio-group semantics, accessible names, keyboard selection, and
      visible focus. Preview decoration must not create extra tab stops.
- [x] Show samples in one row when space permits and stack them on narrow layouts.
- [x] Keep the appearance control in Settings.
- [x] Restore the saved appearance on launch without a visible flash of another
      preset. Map Light/Heyday to light native controls and Dark to dark controls.

## Required: trading-app typography

Replace the current Avenir Next / Segoe UI emphasis with a crisp, consistent font
system. Implemented direction: **Inter for interface text and financial figures**,
with tabular numerals for amounts. Bundle licensed font files locally so the app
works offline and looks consistent across supported systems. Include the font
license in the repository.

Use proportional text for names and descriptions. Reserve monospace for identifiers
or technical values where it improves readability; financial amounts should use
tabular numerals rather than making the entire app monospace.

| Role | Proposed size | Weight and treatment |
| --- | --- | --- |
| Page title | 24px | 600, compact line height |
| Section heading | 16–18px | 600 |
| Main balance or summary value | 28–32px | 600, tabular numerals |
| Body and form text | 14px | 400–500 |
| Financial table text | 13px | 400–500; totals 600 |
| Supporting labels | 12px | 400–500, sufficient contrast |

- [x] Use a consistent font family across navigation, forms, tables, and dialogs.
- [x] Apply tabular numerals to balances, amounts, percentages, and comparison
      figures. Keep financial values right-aligned.
- [x] Use weight, spacing, and alignment to establish hierarchy; avoid excessive
      bold text, oversized headings, and widely spaced uppercase labels.
- [x] Keep currency symbols, minus signs, decimal precision, and large values
      readable without clipping or wrapping ambiguously.
- [x] Verify regular, medium, and semibold weights and any supported non-Latin
      names, including Thai, with suitable local/system fallback fonts.
- Release QA follow-up: verify real WebView zoom. Minimum-window and 125% text-size
      browser checks passed.

## Shared controls and layout

- [x] Standardize button sizes, field heights, spacing, card padding, corner radii,
      and heading hierarchy through shared shadcn/ui components and utilities.
- [x] Replace individually styled controls where an existing shared component fits.
- [x] Review hover, selected, focus, disabled, loading, and error states in all
      three appearances. Neutral presets must not inherit yellow hover fills.
- [x] Maintain comfortable action targets even when table density is compact.
- [x] Keep sidebar/header fixed and main content independently scrollable.
- [x] Preserve sidebar width, collapse behavior, account scrolling, accessible
      names, and tooltips.
- [x] Make page title, supporting text, and primary action placement consistent.

## Settings

- [x] Separate General into clearly labeled Appearance, Currency, and Payday cycle
      sections, followed by existing storage/update information and Danger zone.
- [x] Make immediate appearance changes distinct from payday changes requiring Save.
- [x] Preserve unsaved General settings when switching Settings tabs or appearances.
- [x] Keep destructive controls visually distinct and away from ordinary preferences.

## Financial views

- [x] Standardize table density, numeric alignment, header treatment, dividers,
      subtotal emphasis, and muted supporting labels.
- [x] Use subtle backgrounds for groups and selections. Reserve strong color for
      meaningful financial states and primary actions.
- [x] Preserve explicit Actual, Forecast, estimate, unavailable, and review labels.
- [x] Shorten persistent Outlook guidance. Keep essential limitations visible and
      put detailed explanations in keyboard-accessible help controls.
- [x] Verify sticky labels, horizontally scrolling tables, expanded rows, and large
      negative/positive totals in every appearance.
- [x] Keep Home's large island hero and improve surrounding typographic hierarchy.

## Dialogs and feedback

- [x] Review consistent action placement, scrollable fields, initial focus, error
      placement, saving states, and unsaved-draft protection across dialogs.
- [x] Ensure switching appearance does not reset any open form or draft.
- [x] Keep success feedback on the shared top-center Sonner toaster, following the
      selected appearance and dismissing after four seconds.
- [x] Review empty, loading, failed-load, and retry states for consistent presentation.

## Verification before alpha.7

- [x] Investigate and resolve repeated ResizeObserver errors seen in the previous
      browser test run; check affected dropdowns and layout resizing.
- [x] Verify all three appearance samples, immediate switching, persisted selection,
      existing preference migration, and restart behavior.
- [x] Review Settings, Home, Accounts, Transactions, Income, Outlook, Net Worth,
      Installments, Subscriptions, and representative dialogs in all three presets.
- [x] Check keyboard navigation, focus visibility, contrast, long account names,
      large amounts, empty states, minimum window size, and increased zoom.
- [x] Run the production build, existing Rust tests, and browser tests. Update
      meaningful theme-related tests for the three-option appearance selector.
- Release QA follow-up: smoke-test the packaged desktop app offline to verify bundled fonts, native
      controls, startup appearance, and visual consistency.

## Scope boundaries

Backup/restore, income editing, account archiving, transaction editing, and other
financial features remain tracked separately in TODO.md. Alpha.7 implements the
three appearances and UI improvements without changing ledger calculations or
relabeling financial data.

## Implementation remarks — 2026-10-01

Completed implementation items are marked above. All three presets are available
in Settings with independent preview cards, immediate persistence, keyboard radio
selection, and pre-paint preference restoration using a CSP-compatible local script.
Inter's variable font and SIL Open Font License are bundled under `public/fonts`.
Tables and financial figures use tabular numerals; headings, card corners, shared
controls, Settings sections, and Outlook guidance have been updated.

The dropdown ResizeObserver loop was traced to dialog scale animations changing
anchor measurements while popovers were opening. Dialogs now fade without scaling.
Transaction UI tests fail on window errors, including errors handled by Vite.

Browser checks cover all three presets across the main pages, Outlook views, and
representative account/transaction dialogs. They verify text-token contrast, local
font loading, Thai account labels, long names, independent preview colors, keyboard
selection, saved Light/Dark preferences, payday draft preservation, and minimum-size
window layouts. Screenshots are generated under `test-results/appearance-*.png`.
Text-size checks use a 125% root font size; real WebView zoom still requires manual QA.

Shared dialogs were reviewed for scrolling, focus, save/discard behavior, and action
placement. Default dialogs now have a viewport height limit and scrolling; long forms
retain their fixed action footers. A provider-update test confirms an open transaction
draft survives an appearance change.

Verification: the production build and 92 Rust tests passed; all 128 browser/domain
tests passed, followed by 47 targeted dialog/appearance regression checks after the
final shared dialog adjustments. The existing bundle-size warning remains.

The UX/UI implementation is complete. Native packaged-app verification, including
actual zoom, native controls, and offline launch, remains a release QA follow-up.
Browser UI tests mock native commands and cannot establish those results. No release has been
published; the existing release-tag workflow still controls packaged version numbers.


### Completed follow-up: theme-aware tabs and quieter planner

- [x] Shared tab lists use the selected appearance: neutral Light/Dark surfaces
      and actions, with lavender/purple treatment in Heyday. This applies to
      Settings, Accounts, Outlook, and loan details.
- [x] Remove the opening-cash setup banner and Forecast/Actual explanation banner
      from the planner. Keep Set opening cash, unavailable values, explicit column
      labels, and the expandable detailed guidance.

Follow-up validation: production build and 28 relevant browser/domain tests passed.


### Completed follow-up: Settings alignment and institution labels

- [x] Center the complete Settings content column within the main area, retaining
      its responsive 740px maximum width.
- [x] Use 10px text for the secondary institution name in shared account labels.

### Completed follow-up: Home overview

- [x] Keep the large island hero visible with and without financial records.
- [x] Show the selected payday cycle with a month selector and Current cycle reset.
- [x] Show current available cash, recorded income, and recorded spending for the selected cycle.
- [x] Add reminders for saved card statements needing review or approaching their due date,
      and saved payment plans needing review, without inferring unpaid installments.
- [x] Show upcoming expected income, including estimated net salary after deductions.
- [x] Show the five most recent transactions with links to history.
- [x] Provide loading, retry, currency setup, and first-account states; check all three
      appearances and narrow layouts with an expanded sidebar.

Available cash uses current active cash, bank, and wallet balances. Income and spending
use recorded transactions within the selected cycle through today; spending includes
card purchases and excludes transfers and repayments. Upcoming income remains an
estimate and does not change these figures. Statement reminders use explicit payment
allocations; older statement balances are review references rather than added debt.
Home reads one consistent financial snapshot and refreshes after financial changes
and on window focus. The island artwork remains bundled locally.

Validation: production build, all 135 browser/domain tests, and all 92 Rust tests passed.
Home tests cover exact large amounts, month-end dates, reminders, snapshot refresh,
setup/error states, and responsive layouts. Packaged desktop QA remains as noted above.
