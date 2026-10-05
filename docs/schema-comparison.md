# Schema comparison

Choose **Compare schemas** in the sidebar. Desired source and current target must use the same SQL engine family: PostgreSQL, MySQL/MariaDB, or SQLite. Read-only profiles are supported. Metadata uses saved connection IDs. No migration is executed.

Choose one schema on each side. Tables and columns match by name. Differences are directional: an add is missing on the target, a removal exists only on the target, and a change differs from the desired source. Native type metadata preserves declared length and precision. Nullability, defaults, and available primary/foreign-key flags are compared.

The draft emits ADD COLUMN only for ordinary nullable columns without defaults or key flags and with a supported simple native type. Generated/identity columns, missing tables, removals, default/nullability/type changes become manual review items. No DROP, rebuild, apply, or inferred rename is generated. Review every statement against native definitions, constraints, and data before using it in a migration workflow.

Coverage is column metadata, not full database parity. Indexes, complete/composite constraints and foreign-key destinations, generated expressions, triggers, views, sequences, permissions, and column order require separate review. SQLite generated/hidden column existence is surfaced separately. Source defaults are displayed but never transplanted because they can reference source objects. A no-difference result only refers to compared metadata.

Limits: 256 ordinary tables, 128 columns per table. Existing driver metadata calls are sequential and do not provide an atomic cross-database snapshot; concurrent DDL may require a refresh. Native read-only SQLite and pure comparison/draft tests are included. Live PostgreSQL/MySQL and migration acceptance remain separate environment checks.
