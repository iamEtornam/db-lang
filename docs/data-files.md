# Data files

Open **Data files** in the sidebar. Files are processed locally in Rust, without an LLM or remote upload. Maximum file size is 8 MiB.

## Import CSV or Excel

Choose **Import into an existing table**, select a CSV or XLSX file, then choose a saved SQL connection and an existing destination table. The first file row supplies unique nonempty column headers. XLSX supports worksheet selection; formulas are not evaluated and their cached values are used. Empty spreadsheet cells are NULL; CSV empty fields remain empty text unless **Convert empty text cells to NULL** is selected.

Preview the first 20 rows, map each source column to a destination column or omit it, then select **Review import**. Type the destination table name to confirm. Nothing is written during the preview. Import reparses the submitted file and validates the mapping against current database column metadata before binding values in a native transaction. Unmapped columns use native defaults. A successful import is disabled from immediate resubmission until its inputs change.

Limits and behavior:

- At most 10,000 rows and 128 file columns. Headers are unique and at most 256 bytes. Expanded XLSX archives are limited to 32 MiB/512 entries, and streamed cell coordinates are bounded before allocating sheet rows.
- PostgreSQL, MySQL/MariaDB, and SQLite are supported. Read-only profiles cannot import. PostgreSQL supports common scalar, date/time, JSON, UUID, and bytea columns; unsupported custom/array types must be omitted. SQLite follows its native affinity and constraints. MySQL/MariaDB requires an InnoDB destination and uses strict value conversion on a dedicated connection.
- Values use bound parameters, never SQL interpolation. PostgreSQL/MySQL inserts are batched in groups of 100; SQLite reuses one prepared statement. Errors abort the transaction. Native database triggers/functions retain their own semantics and may have external effects.
- Imports append rows; they do not create tables, replace data, upsert, or evaluate formulas. SQL schema changes and custom conversions should be reviewed separately.

## Query CSV, JSON, or Parquet locally

Choose **Query a local file**, select the file, then enter a single read-only SQL query using the table name `data`. DuckDB infers file types and loads the selected bytes into an in-memory table. For example:

```sql
SELECT name, COUNT(*) AS occurrences
FROM data
GROUP BY name
ORDER BY occurrences DESC
```

Each run gets a fresh in-memory database. Extension installation is disabled, and external file/network access plus configuration changes are disabled after loading. Temporary selected-file bytes are removed after the run. No saved database is modified. Common PostgreSQL-compatible SELECT syntax is accepted; engine-specific syntax that the validator cannot parse is rejected.

Results are capped at 500 rows, 128 columns, and 2 MiB. DuckDB is configured for 256 MiB memory, two workers, and no disk spill. A 30-second timer interrupts an expired query. One native file query runs at a time. These limits bound normal operation, not every allocation inside native parsers or blocked filesystem I/O.

Validation uses native SQLite and DuckDB fixtures plus a synthetic browser IPC fixture. Live PostgreSQL/MySQL/MariaDB, native file selection on each desktop platform, and platform-specific DuckDB packaging need separate acceptance.
