# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Firebase Firestore** engine: connect with a Service Account JSON, browse collections as tables, sample documents to infer schema, and run queries in the compact `collectionName` or `collectionName.{...structuredQuery JSON...}` format. Backed by the Firestore REST API.
- **Firebase Realtime Database** engine: connect with a Service Account JSON + database URL, browse top-level paths, and run REST-style path queries (`users` or `messages?orderBy="$key"&limitToFirst=20`).
- **Live RTDB streaming** via a new "Watch" tab on the query results panel. Powered by Server-Sent Events; surfaces `put` / `patch` events through Tauri's event bus to a reactive snapshot view + event log. New Tauri commands `rtdb_subscribe` and `rtdb_unsubscribe`.
- New Tauri command `build_firebase_conn_str` for building the Firebase connection blob server-side from a service-account JSON, so the dialog's Test button works for unsaved Firebase connections.
- New `auth_json` column on the `connections` table (idempotent migration) for storing service-account credentials.
- Service-account warning banner in the Connection dialog reminding users that admin credentials bypass Firebase security rules.
- New Vue component `RtdbWatchPanel.vue` for live-watching RTDB paths.
- Engine-aware AI query translation: the `QueryDialect` enum drives separate prompts for SQL, MongoDB, Redis, Firestore, and Realtime DB. The Firestore and RTDB prompts explicitly forbid SQL output.
- 19 unit tests across `gemini`, `firestore`, and `firebase_auth` covering query-dialect dispatch, prompt content, blob round-tripping, and Firestore input validation.

### Fixed

- Firestore connections no longer generate SQL like `SELECT * FROM "Profiles"` for natural-language queries. The translator now dispatches to a Firestore-specific prompt.
- Firestore driver now rejects SQL-looking input with a clear error (`Firestore does not support SQL...`) instead of silently returning zero rows.
- Firestore driver rejects collection names containing whitespace or other non-identifier characters.
- Connection dialog's "Test" button no longer fails with `Expected firebase:// prefix` for Firebase engines. The dialog now asks the backend to build the proper base64 connection blob before testing.

### Changed

- `DbConnectionRecord`, `CreateConnectionRequest`, and `UpdateConnectionRequest` now carry an `auth_json` field (defaults to `""` for non-Firebase engines).
- `translate_with_schema` and `translate_to_query_with_kb` in `gemini.rs` now branch on engine type rather than assuming SQL.
- README "Multi-Database Support" line updated to list all 7 engines (PostgreSQL, MySQL, SQLite, MongoDB, Redis, Firestore, RTDB).
