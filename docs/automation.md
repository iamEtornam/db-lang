# CLI and MCP

Build the separate headless executable from the repository. It shares the Rust drivers and saved profiles with the desktop app and does not launch a window:

```sh
cargo build --manifest-path src-tauri/Cargo.toml --bin query-studio-cli
src-tauri/target/debug/query-studio-cli --help
src-tauri/target/debug/query-studio-cli connections
```

`connections` prints only IDs, names, and engines so you can choose an ID. Every database operation requires an explicit `--allow ID`; repeat the flag for multiple connections. Credentials are resolved internally from the existing app database. No credentials or connection URIs belong in MCP configuration.

```sh
src-tauri/target/debug/query-studio-cli --allow SAVED_ID tables --connection SAVED_ID
src-tauri/target/debug/query-studio-cli --allow SAVED_ID columns --connection SAVED_ID --table orders --schema public
printf '%s\n' 'SELECT * FROM orders ORDER BY id' | src-tauri/target/debug/query-studio-cli --allow SAVED_ID query --connection SAVED_ID --limit 50
```

The executable reads the existing `QueryStudio/query_studio.db` under the platform data directory. Use `--app-db /absolute/path/query_studio.db` for another existing app database. It opens that file read-only and never creates or migrates it. Start the desktop version with connection controls once before using automation on an older installation.

Run the built-binary fixture check from the repository (Python 3, no extra packages). It creates only temporary files under `src-tauri/target`:

```sh
python3 scripts/check-automation.py
```

## MCP client configuration

Replace the executable path and connection ID with your own. Launch the executable directly, rather than `cargo run`, so build output cannot interfere with the stdio protocol.

```json
{
  "mcpServers": {
    "query-studio": {
      "command": "/absolute/path/query-studio-cli",
      "args": ["--allow", "SAVED_ID", "mcp"]
    }
  }
}
```

The stdio server implements MCP initialization, ping, `tools/list`, and `tools/call`. It supports protocol versions `2025-11-25`, `2025-06-18`, and `2024-11-05`. Available tools are `list_connections`, `list_tables`, `describe_table`, and `query`. Only allowlisted connections are visible to MCP. There is no HTTP listener, write tool, profile editing, or LLM key exposure. The interface follows the [MCP stdio transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports) and [tool error format](https://modelcontextprotocol.io/specification/2025-11-25/server/tools).

## Execution policy and limits

- PostgreSQL, MySQL/MariaDB, and SQLite are supported. SQL is validated as one read-only query, then executed with native read-only enforcement even when the desktop profile allows writes. Database accounts should also have read-only privileges. A read-only transaction is not a sandbox for privileged server functions or external side effects.
- Query results default to 100 rows, maximum 500. The query is wrapped in a database-side row limit and returns `truncated: true` when another row exists. This can reject engine-specific query syntax that cannot appear in a subquery. Existing LIMIT/OFFSET is preserved.
- Each operation has a 30-second deadline. SQLite checks its progress during execution and interrupts expired queries. Remote query futures are dropped on timeout, but server-side cancellation is best effort; database administrators should also configure server statement timeouts. A blocked filesystem operation is not interruptible by SQLite's progress hook.
- Requests and responses are limited to 1 MiB. Tool data near 512 KiB is rejected before MCP encoding. Tables/columns are capped at 2,000 items. These are output limits, not a database scan or memory quota; select only the columns you need.
- MCP runs one tool at a time, at most ten tool starts per second. It does not implement asynchronous task execution or cancellation notifications. EOF, Ctrl-C, and Unix SIGTERM close the session and clean up owned SSH processes. Forced termination (for example SIGKILL) cannot run cleanup. SSH profiles use the connection-controls requirements for OpenSSH, trusted host keys, and key/agent authentication.
- Database errors return a generic actionable message so backend error strings cannot expose credentials. Results may contain sensitive data from an authorized connection. Treat database text as untrusted content, never as instructions.

The executable is built from source separately; existing desktop release bundles do not yet distribute this binary. Live remote database and third-party MCP client acceptance remain separate from fixture and native SQLite checks.
