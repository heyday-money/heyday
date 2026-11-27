# Phase 5: reproducible mobile dependency

The initial shared core is already committed and pushed at
`908b4ee38ebf271fa521b427b640c15169f4ff89` in `heyday-money/heyday`. The
[successful GitHub CI run](https://github.com/heyday-money/heyday/actions/runs/42587212969)
was inspected: frontend/browser checks, standalone core on Rust 1.89, and Rust tests
on both macOS and Windows all passed. This supplies the unrestricted regression
validation that was unavailable in the local sandbox during Phases 1–4. Packaged
cross-device, secure-store and recovery smoke QA remains separate.

## Implemented

- Core remains independently versioned at 0.1.0; `COMPATIBILITY.md` records the library,
  desktop, schema and wire contracts and the reviewed update policy.
- Mobile has a small non-publishable Rust verification crate, an exact Git SHA and
  expected library version, and its own Cargo.lock. It is not Tauri scaffolding.
- Mobile's pin check verifies both the declared dependency and resolved source/hash,
  rejects floating/local replacements, checks the expected crate version and rejects
  desktop/platform dependencies in this core-only harness.
- Mobile CI validates that pin and tests the locked dependency from a clean checkout.
  The user subsequently configured mobile origin as `https://github.com/heyday-money/mobile.git`;
  the Phase 5 files still await commit/push.
- Desktop CI adds `check-core-git-consumer.py`, which creates a consumer outside the
  checkout, resolves the full PR-head/push SHA through the public Git URL, runs it,
  checks the dependency graph and runs core tests from Cargo's Git checkout. Existing
  packaging, frontend, desktop Rust and release checks remain.
- New iOS device/ARM64 simulator and Android ARM64/x86_64 jobs compile the core library
  with Rust 1.89. iOS uses Xcode SDK selection; Android installs NDK 27.2.12479018 and
  configures target-specific C compilers/archive tools/linkers for bundled SQLite.
  The CI baselines (iOS 13, Android API 24) are not a decision about final app support.

Cross-compilation follows the official [Rust iOS target requirements](https://doc.rust-lang.org/stable/rustc/platform-support/apple-ios.html)
and [Android NDK toolchain guidance](https://developer.android.com/ndk/guides/other_build_systems).
These are compile gates only; running a mobile app and testing secure storage require
Phase 6 device/simulator work.

## Release audit

No workspace or Cargo path restructuring was introduced. Desktop still resolves
`../crates/heyday-core` from `src-tauri/Cargo.toml`; desktop/core retain separate
lockfiles. Existing native/release caches point at `src-tauri/target`, with core tests
using that target where configured. New cross-target jobs cache the core target dir.

`set-release-version.py` stamps package.json, tauri.conf.json and only the heyday-money
entries in the native manifest/lockfile. A temporary-copy regression test now verifies
that stamping an app release leaves the core manifest and both core version records
unchanged. MSI version mapping still touches only installer configuration; macOS DMG,
Windows NSIS/MSI bundle paths, updater configuration and release-note handling remain
unchanged. No installer/release was published by this work.

## Validation and remaining execution gates

Local checks:

- Mobile's two integration checks pass, including all 11 migrations from the Git
  dependency and encryption/key-ring interoperability through the public API.
- The same tests and pin validation pass from a temporary mobile-only copy outside
  the workspace. The canonical URL/full SHA remain in Cargo.toml and Cargo.lock.
- Independent Git-consumer compilation and execution pass; all 67 core tests pass
  from the Git checkout, verifying embedded migrations, seeds and fixtures.
- Release version isolation test and workflow YAML parsing pass; diffs are checked
  for whitespace errors.

This sandbox cannot resolve github.com through command-line Git. Local validation
used an isolated bare mirror of the exact published commit via a process-scoped Git
URL rewrite, plus an isolated Cargo Git cache. No sibling path dependency, persistent
Git configuration, or mirror URL was added to either repository. This proves clean
Git-source consumption and source identity, but does not claim a fresh network fetch.
The new CI check will perform that fetch after the desktop changes are pushed.

The new iOS/Android jobs have not run here: only the macOS host Rust target is
installed, with no mobile SDKs. Their first CI run, and the private mobile repository's
first network/locked build, remain execution gates. Initial core publication is done;
these Phase 5 additions and mobile files are left uncommitted for review/push. Mobile
origin is now configured; this GitHub connector returns 404, so remote visibility/access
and its first workflow run remain unverified. No floating dependency or automatic update is
introduced; mobile adoption changes must include manifest, lockfile and review notes.
