use rusqlite::Connection;

use crate::errors::DbError;

/// SQL executed once when creating a new `ail-db` database.
///
/// WAL mode, foreign keys, and synchronous pragmas are applied on every
/// connection open (see `SqliteGraph::configure_pragmas`), not here.
const SCHEMA_SQL: &str = "
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

/// Create all tables and indexes on a fresh connection.
pub(crate) fn init_schema(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(SCHEMA_SQL)?;
    Ok(())
}
