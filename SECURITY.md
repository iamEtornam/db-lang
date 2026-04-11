# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in Query Studio, please report it responsibly by emailing **security@etornam.dev** (or opening a private security advisory on GitHub).

Please do **not** file a public issue for security vulnerabilities.

We will acknowledge receipt within 48 hours and aim to release a fix within 7 days for critical issues.

## Threat Model

Query Studio is a **desktop application** that connects directly to user-owned databases. The key trust boundaries are:

| Boundary | Trust level |
|----------|-------------|
| Tauri webview ↔ Rust backend | Trusted (same-process IPC) |
| Application ↔ User databases | User-controlled credentials |
| Application ↔ LLM APIs | User-provided API keys sent over HTTPS |
| Local SQLite (app data) | Stored in the OS user data directory |

### What we protect against

- **SQL injection** in schema introspection queries (parameterized queries and escaped identifiers).
- **API key leakage** in error messages, logs, and URLs (keys sent via HTTP headers, errors sanitized).
- **Prompt injection** in AI-generated queries (allow-list filter: only `SELECT`/`SHOW`/`DESCRIBE`/`EXPLAIN`/`WITH` statements).
- **Broad network access** from the webview (CSP `connect-src` is restricted to known LLM API domains).

### Known limitations

- **Credentials at rest**: Database passwords and LLM API keys are stored in plaintext in a local SQLite database (`query_studio.db`) under the OS user data directory. This is comparable to how most database GUI tools store credentials. A future improvement would use the OS keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service).

- **Arbitrary SQL execution**: Users can manually type and execute any SQL query (including destructive ones like `DROP TABLE`). This is by design for a database client. Only AI-generated queries are restricted to read-only statements.

- **Local app database not encrypted**: The app's internal SQLite database is not encrypted at rest. Anyone with file-system access to the user's data directory can read stored connection details.

## Supported Versions

| Version | Supported |
|---------|-----------|
| latest  | Yes       |

## Security Best Practices for Users

1. **Use dedicated database credentials** with minimal privileges (read-only when possible) for Query Studio connections.
2. **Do not share your app data directory** — it contains saved connection passwords.
3. **Use environment variables** for LLM API keys instead of saving them in the app settings when possible.
4. **Keep Query Studio updated** to receive security fixes.
