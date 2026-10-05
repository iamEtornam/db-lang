# Credential storage and connection backups

Passwords, service-account JSON, connection hosts (including URI credentials), AI API keys, and AI endpoint URLs are encrypted before SQLite writes. AES-256-GCM uses random 96-bit nonces and authenticates the table, record ID, and field name. Empty values stay empty. The 256-bit key lives in macOS Keychain, Windows Credential Manager, or Linux Secret Service, under service `dev.etornam.QueryStudio.credentials` and a per-installation UUID. Linux needs a running, unlocked Secret Service; there is no plaintext or local-key-file fallback.

## Upgrade

Startup migrates legacy plaintext fields in one SQLite transaction. It verifies that a new OS key can be retrieved before committing encrypted records. A durable cleanup marker remains until WAL checkpoints and VACUUM reclaim legacy pages. Cleanup failure blocks startup and retries next time. All encrypted fields must authenticate before the app database becomes available. Migration is automatic; no plaintext backup copy is created.

Do not run two application instances during migration. Close the other instance and retry if the database or WAL is busy. OS credential errors appear in the app with a Retry control. Missing keys on an already encrypted installation never cause a new key to be generated. The encrypted records are retained.

SQLite cleanup cannot erase historical filesystem snapshots, external backups, or storage-device remnants. These need their own retention policy.

## Saved credential editing

The webview receives empty password/service-account/API-key fields and presence flags. URI authentication and query parameters are hidden from saved endpoint views. A blank credential field preserves the original only for the same engine or AI provider. A nonempty value replaces it. The password/API-key removal checkboxes explicitly clear those fields. Firebase requires a valid retained or replacement service account.

An unchanged displayed Mongo URI preserves its hidden credentials and options. To replace the full URI, select **Replace the entire saved URI** and enter the complete desired URI; this also permits deliberately removing embedded authentication. Removing the separate password field does not remove a password embedded in the URI. Saved connection testing and model discovery resolve credentials in Rust. Newly entered secrets necessarily pass from the form to Rust, but saved secrets are never returned by these commands.

## Portable backup

Settings > Connection backup exports selected connections to an encrypted JSON file. A separate passphrase of at least 12 characters derives a 256-bit key using Argon2id (v19, 19 MiB memory, 2 iterations, 1 lane) and a random 16-byte salt. The payload uses AES-256-GCM. Format version 1 fixes these parameters. Keep the passphrase separately; it cannot be recovered by Query Studio.

Backups include connection credentials and options. They exclude AI keys, history, snippets, and the device encryption key. Files are limited to 10 MiB; export plaintext is limited to 5 MiB; an import contains 1–1000 supported connection profiles. Authentication, JSON shape, and engine/name/host validation occur before insertion. Wrong passwords and corrupt files fail without importing anything. Imports use one transaction, new IDs, new timestamps, and the receiving device's encryption key. Existing profiles are never overwritten. Imported duplicates can be renamed in the normal connection editor.

## Recovery

A raw `query_studio.db` copy cannot be opened on another device without the original OS credential key. Prefer portable encrypted export/import. Keep backups while the source installation is still unlocked.

If the original key is unavailable, restore it from the original keychain/credential-manager backup. If only a portable backup remains, import it into a fresh installation with its own key. Preserve the old encrypted database separately before manually creating that fresh installation. The app does not reset or delete a locked installation. AI keys must be re-entered after recovery from a connection-only backup.

## Verification boundary

Rust tests cover authenticated encryption, migration rollback/key-loss behavior, SQLite ciphertext, portable-backup integrity, atomic imports, and redacted frontend responses. Browser checks use synthetic profiles and mocked Tauri responses. Native OS credential-store permissions and actual remote database connectivity still require desktop acceptance on macOS, Windows, and Linux.
