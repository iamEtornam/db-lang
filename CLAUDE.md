# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**Query Studio** (binary identifier `db-lang`, bundle id `dev.etornam.db-lang`) is a Tauri 2 desktop app that lets users connect to multiple database engines and translate natural-language prompts into SQL/MQL/Redis/Firestore queries via an LLM (Gemini by default, with OpenAI/Anthropic/Ollama/custom providers configurable at runtime).

## Commands

All commands run from the repo root.

- `npm run tauri dev` — full dev loop: Nuxt on :3000 + Tauri shell with HMR. This is the primary way to run the app.
- `npm run dev` — Nuxt-only on :3000 (no native window; mostly useful when iterating on UI without the Rust backend).
- `npm run generate` — pre-rendered static site output to `.output/public` (what Tauri bundles as `frontendDist`).
- `npm run tauri build` — production native bundle. On macOS, `./build-dmg.sh` wraps this with `--bundles app,dmg` and assumes the frontend is already generated into `dist/` (a symlink to `.output/public`).
- `(cd src-tauri && cargo build)` / `cargo check` / `cargo test` — Rust-side compile/test. There is no top-level lint or test npm script; type errors surface through `nuxt prepare`/Vite at dev time and `vue-tsc` is available as a dev dep.

Release flow is tag-driven — `git tag vX.Y.Z && git push origin vX.Y.Z` triggers `.github/workflows/release.yml`, which stamps the version into `tauri.conf.json` / `package.json` / `src-tauri/Cargo.toml`, builds signed bundles for macOS (arm64+x86_64), Linux, Windows, and publishes a `latest.json` manifest for the in-app updater. See README "Releasing & in-app updates" for the signing key one-time setup.

## Architecture

### Two-process split

- **Frontend** (`app/`) — Nuxt 4 SPA (`ssr: false`, `compatibilityVersion: 4`), Vue 3 + TypeScript, Pinia stores, shadcn-vue components under `app/components/ui/`, Tailwind 4. Color mode forced to dark fallback. Fonts: Geist via `@nuxt/fonts`.
- **Backend** (`src-tauri/src/`) — Rust, Tokio runtime, exposes everything via `#[tauri::command]` in `lib.rs::run()`. The frontend never speaks to databases or LLM APIs directly; all I/O goes through `invoke('command_name', ...)`.

The Tauri config wires `beforeDevCommand: npm run dev` and `beforeBuildCommand: npm run generate`, so `npm run tauri dev|build` is enough — don't start Nuxt separately.

### Credentials never leave Rust

Connection records are stored in a local SQLite app DB (`AppDatabase` in `app_db.rs`, kept under `~/Library/Application Support/QueryStudio` on macOS / `dirs::data_dir()/QueryStudio` elsewhere). The frontend works exclusively with **`connection_id`s** — it sends an ID + a query, and `resolve_connection()` in `lib.rs` looks up the row, calls `build_connection_string()`, and instantiates a driver. Passwords / service-account JSON / API keys never round-trip through Vue.

This is load-bearing: when adding new commands that touch a saved DB, take a `connection_id`, not a connection string. The `test_connection` command is the exception (used for unsaved connections from the UI dialog).

### Driver layer

`src-tauri/src/drivers/` has one module per engine and a single `DatabaseDriver` async trait in `mod.rs`. `create_driver(engine, conn_str)` is the factory. Supported engines:

- SQL: `postgres`, `mysql` (also matches `mariadb`), `sqlite`
- NoSQL/KV: `mongodb`, `redis`
- Firebase: `firestore`, `firebase_rtdb` — both use `FirebaseConnBlob` (base64-encoded JSON, see `firebase_auth.rs`) instead of a URI, and authenticate via service-account JWT. RTDB additionally supports live streaming through `rtdb_subscribe` / `rtdb_unsubscribe`, which emit Tauri events back to the frontend.

The trait surface (`execute_query`, `execute_query_paginated`, `get_tables`, `get_table_columns`, `get_relationships`, `preview_table_data`, `query_language`, `engine_name`) is intentionally narrow. Each driver returns rows as `Vec<serde_json::Value>` so the frontend gets a uniform shape regardless of engine. `QueryLanguage` (`Sql`/`Mql`/`Redis`/`Firestore`/`FirebaseRtdb`) is what the LLM prompt code reads to pick the right syntax.

Helpers `quote_identifier(engine, name, schema)` and `strip_pagination(query)` live in `drivers/mod.rs` — reuse them rather than re-inventing per driver.

### LLM layer (`gemini.rs`)

Despite the filename, this module is multi-provider. Config is read from the SQLite app DB (`LlmConfig` row); env vars `GEMINI_API_KEY` / `OPENAI_API_KEY` are only a fallback when no in-app config is set. Providers supported: `gemini`, `openai`, `anthropic`, `ollama`, `custom`. The CSP in `tauri.conf.json` whitelists exactly the LLM hosts the app talks to — adding a new provider requires updating `connect-src` there too.

Translation prompts include schema context built from the **Schema Knowledge Base** (`schema_kb.rs`): a snapshot of tables/columns/relationships with optional LLM-generated descriptions, regenerated per-connection via `generate_schema_kb` / `refresh_schema_kb`. KB generation emits progress events the UI subscribes to.

### Connection pool & query cache (`connection_pool.rs`)

In-process query result cache (1 min TTL, hashed by `conn_str + query`) plus driver-level connection pools. Cache and pool stats are exposed as commands and surfaced in the Settings UI. Cache invalidation is best-effort — don't rely on it for writes (and the app blocks destructive queries at the LLM layer via `LlmError::DestructiveQuery` rather than relying on the cache).

### Frontend stores & composables

- `stores/connections.ts` — single source of truth for connections, the active connection, and the loaded schema/table list.
- `stores/history.ts`, `stores/snippets.ts` — backed by app-DB tables of the same shape.
- `composables/useQuery.ts`, `useConnection.ts`, `useSchemaKb.ts`, `useTauri.ts`, `useAppUpdater.ts`, `useAppSettings.ts` — thin wrappers over `invoke()` plus reactive state.
- `pages/` is route-based: `index.vue` (query workspace), `schema.vue`, `history.vue`, `settings.vue`.

### Icon bundling (Tauri-specific gotcha)

`nuxt.config.ts` sets `icon.serverBundle: false`, `fallbackToApi: false`, and an explicit `clientBundle.icons` list. Reason: Tauri serves a static bundle with a strict CSP that blocks `api.iconify.design`; without bundling, icons either blank out or trigger uncaught fetch rejections that blank the page. If you reference a new icon dynamically (e.g. from `app/constants/engines.ts`), add it to the explicit list — the scanner only catches statically-resolvable usages.

### Updater

`tauri-plugin-updater` polls the `endpoints` URL in `tauri.conf.json` (currently the GitHub release `latest.json`) and verifies bundles against the embedded minisign `pubkey`. Updates surface through `components/shared/UpdateBanner.vue` + `composables/useAppUpdater.ts`. Linux `.deb` users are not auto-updated; only AppImage is.

## Conventions worth knowing

- Tauri commands live in `lib.rs::run()`'s `invoke_handler!` block — when adding one, register it there or the frontend will get "command not found".
- All `#[tauri::command]` functions return `Result<T, String>` (errors converted with `map_err(|e| e.to_string())`). Match that shape; don't bubble up raw error types to the frontend.
- shadcn components are generated under `app/components/ui/` (config in `components.json`, prefix is empty, components dir overridden in `nuxt.config.ts`). Don't hand-edit unless you also update the generation source.
- Vite is configured with `envPrefix: ['VITE_', 'TAURI_']` and `clearScreen: false` — Tauri-prefixed env vars are visible to the frontend.
