use rusqlite::Connection;

use crate::errors::DbError;

/// SQL for the core relational tables and indexes.
///
/// WAL mode, foreign keys, and synchronous pragmas are applied on every
/// connection open (see `SqliteGraph::configure_pragmas`), not here.
const CORE_SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS nodes (
    id         TEXT PRIMARY KEY,
    intent     TEXT NOT NULL,
    name       TEXT,
    pattern    TEXT NOT NULL,
    expression TEXT,
    parent_id  TEXT REFERENCES nodes(id) ON DELETE CASCADE,
    position   INTEGER NOT NULL DEFAULT 0,
    depth      INTEGER NOT NULL DEFAULT 0,
    metadata   TEXT NOT NULL DEFAULT '{}',
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_nodes_parent  ON nodes(parent_id);
CREATE INDEX IF NOT EXISTS idx_nodes_pattern ON nodes(pattern);
CREATE INDEX IF NOT EXISTS idx_nodes_name    ON nodes(name);
CREATE INDEX IF NOT EXISTS idx_nodes_depth   ON nodes(depth);

CREATE TABLE IF NOT EXISTS contracts (
    id         TEXT PRIMARY KEY,
    node_id    TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    kind       TEXT NOT NULL,
    expression TEXT NOT NULL,
    position   INTEGER NOT NULL DEFAULT 0,
    UNIQUE(node_id, kind, expression)
);
CREATE INDEX IF NOT EXISTS idx_contracts_node ON contracts(node_id);

CREATE TABLE IF NOT EXISTS edges (
    source_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    target_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    kind      TEXT NOT NULL,
    label     TEXT,
    metadata  TEXT DEFAULT '{}',
    PRIMARY KEY (source_id, target_id, kind)
);
CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source_id);
CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target_id);
CREATE INDEX IF NOT EXISTS idx_edges_kind   ON edges(kind);

CREATE TABLE IF NOT EXISTS project_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS cic_cache (
    node_id     TEXT PRIMARY KEY REFERENCES nodes(id) ON DELETE CASCADE,
    packet_json TEXT NOT NULL,
    valid       INTEGER NOT NULL DEFAULT 1,
    computed_at TEXT DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_cic_cache_valid ON cic_cache(valid);
";

// ─── FTS5 ─────────────────────────────────────────────────────────────────────
//
// FTS5 full-text search over node content fields.
//
// Standalone FTS5 table (no content= option): FTS5 stores a copy of the indexed
// text alongside the index. The triggers below keep both the FTS5 index and the
// stored text in sync. The search query JOINs `nodes` for depth/pattern/id anyway,
// so the duplicate storage is the only trade-off vs. a content table.
//
// content_rowid is not needed for standalone tables — rowid is provided explicitly
// in every trigger INSERT (matching nodes.rowid), so the search JOIN works correctly.
//
// NULL name/expression are indexed as empty string by FTS5 (safe, no special handling).
// metadata (JSON blob) is deliberately excluded — unstructured JSON is not useful to tokenize.

const FTS5_CREATE: &str = "
CREATE VIRTUAL TABLE IF NOT EXISTS search_fts USING fts5(
    intent,
    name,
    expression,
    pattern,
    tokenize='porter unicode61'
);
";

// Trigger: INSERT — add new node to FTS5 index.
const FTS5_TRIGGER_AI: &str = "
CREATE TRIGGER IF NOT EXISTS nodes_ai AFTER INSERT ON nodes BEGIN
    INSERT INTO search_fts(rowid, intent, name, expression, pattern)
    VALUES (new.rowid, new.intent, new.name, new.expression, new.pattern);
END;
";

// Trigger: DELETE — remove deleted node from FTS5 index.
//
// For standalone FTS5 tables (no content= option) use a regular DELETE statement.
// The FTS5 'delete' admin command only applies to content tables.
const FTS5_TRIGGER_AD: &str = "
CREATE TRIGGER IF NOT EXISTS nodes_ad AFTER DELETE ON nodes BEGIN
    DELETE FROM search_fts WHERE rowid = old.rowid;
END;
";

// Trigger: UPDATE — replace old FTS5 entry with updated content.
//
// Delete the old entry by rowid, then re-insert with the new content.
const FTS5_TRIGGER_AU: &str = "
CREATE TRIGGER IF NOT EXISTS nodes_au AFTER UPDATE ON nodes BEGIN
    DELETE FROM search_fts WHERE rowid = old.rowid;
    INSERT INTO search_fts(rowid, intent, name, expression, pattern)
    VALUES (new.rowid, new.intent, new.name, new.expression, new.pattern);
END;
";

/// Create all tables, indexes, FTS5 virtual table, and sync triggers.
pub(crate) fn init_schema(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(CORE_SCHEMA_SQL)?;
    conn.execute_batch(FTS5_CREATE)?;
    conn.execute_batch(FTS5_TRIGGER_AI)?;
    conn.execute_batch(FTS5_TRIGGER_AD)?;
    conn.execute_batch(FTS5_TRIGGER_AU)?;
    Ok(())
}
