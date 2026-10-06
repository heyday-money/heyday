# Alpha migration fixtures

`alpha-migrations/` is the immutable 40-migration alpha chain, retained only for
Rust regression tests. It is not included in the application's migration runner.
Historical tests still exercise the original upgrades; the beta baseline test
compares the final alpha schema and all reference data against the single beta
initial migration. The Fee/Interest category seed now has a stable ID.

Production migrations live in `src-tauri/migrations/`. Beta starts in a separate
`v1` storage directory and intentionally does not import alpha databases/backups.
