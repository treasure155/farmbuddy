Rust backend for FarmBuddy. Commands are split by domain in `src/` (auth, inventory, sales, reports, settings, email, templates) and registered in `src/lib.rs`.

- Add a new command: write it in the right module with `#[tauri::command]`, add it to `generate_handler![]` in `lib.rs`, then call it from the frontend through `call()` in `src/api.ts`.
- Schema changes: add `migrations/003_*.sql` and a `version < 3` block in `db::migrate`. Never edit an existing migration.
- Run tests with `cargo test`.
