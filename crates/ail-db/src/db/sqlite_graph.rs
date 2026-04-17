use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use ail_graph::{
    compute_context_packet_for_backend,
    errors::GraphError,
    graph::GraphBackend,
    search::SearchResult,
    types::{Contract, EdgeKind, Expression, Node, NodeId},
    AilGraph, ContextPacket,
};
use rusqlite::Connection;
use std::collections::VecDeque;
use uuid::Uuid;

use crate::errors::DbError;

use super::cic_cache;
use super::fts_search;
use super::node_serde::{
    contract_kind_from_sql, contract_kind_to_sql, node_id_from_sql, row_to_node,
};
use super::schema::init_schema;

/// A graph backed by a single SQLite `.ail.db` file.
///
/// `Connection` is wrapped in a `Mutex` so that `SqliteGraph` satisfies the
/// `Send + Sync` bounds required by `GraphBackend`. The mutex is never contested
/// in the current single-threaded pipeline; the overhead is negligible.
///
/// All DB helpers are free functions that accept `&Connection` directly, which
/// avoids re-locking the mutex and the associated deadlock risk.
pub struct SqliteGraph {
    pub(crate) conn: Mutex<Connection>,
    path: PathBuf,
}

// ─── Constructors ─────────────────────────────────────────────────────────────

impl SqliteGraph {
    /// Create a new database at `path`. Returns an error if the file already exists.
    pub fn create(path: &Path) -> Result<Self, DbError> {
        if path.exists() {
            return Err(DbError::Other(format!(
                "database already exists: {}",
                path.display()
            )));
        }
        let conn = Connection::open(path)?;
        configure_pragmas(&conn)?;
        init_schema(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            path: path.to_path_buf(),
        })
    }

    /// Open an existing database at `path`. Returns an error if not found.
    pub fn open(path: &Path) -> Result<Self, DbError> {
        if !path.exists() {
            return Err(DbError::Other(format!(
                "database not found: {}",
                path.display()
            )));
        }
        let conn = Connection::open(path)?;
        configure_pragmas(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            path: path.to_path_buf(),
        })
    }

    /// Open an existing database or create a new one if the file does not exist.
    pub fn open_or_create(path: &Path) -> Result<Self, DbError> {
        let conn = Connection::open(path)?;
        configure_pragmas(&conn)?;
        // init_schema uses CREATE TABLE IF NOT EXISTS — safe to call on any connection.
        init_schema(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            path: path.to_path_buf(),
        })
    }

    /// Return the filesystem path to the database file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Acquire the connection lock. Panics if the mutex is poisoned.
    pub(crate) fn db(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn
            .lock()
            .expect("SqliteGraph connection mutex poisoned")
    }

    // ─── Diagnostic / test-support helpers ───────────────────────────────────

    /// Return the current SQLite journal mode (e.g. `"wal"`, `"delete"`).
    ///
    /// Used in tests to assert WAL mode is active; useful in diagnostics.
    pub fn journal_mode(&self) -> Result<String, DbError> {
        let db = self.db();
        let mode = db.query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))?;
        Ok(mode)
    }

    /// Count all rows in a named table.
    ///
    /// Allowed tables: `nodes`, `contracts`, `edges`, `project_meta`, `cic_cache`, `search_fts`.
    /// Used in tests to verify cascade deletions and cache state.
    pub fn table_row_count(&self, table: &str) -> Result<i64, DbError> {
        let table = match table {
            "nodes" | "contracts" | "edges" | "project_meta" | "cic_cache" | "search_fts"
            | "embeddings" => table,
            other => return Err(DbError::Other(format!("unknown table: {other}"))),
        };
        let db = self.db();
        let count = db.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get::<_, i64>(0)
        })?;
        Ok(count)
    }

    // ─── CIC cache ───────────────────────────────────────────────────────────

    /// Return the cached [`ContextPacket`] for `node_id`, computing and storing
    /// it on a cache miss.
    ///
    /// The cache is keyed by `node_id`. A stored entry is returned immediately
    /// (cache HIT). When no valid entry exists the packet is computed via the
    /// backend-agnostic CIC algorithm, stored, and returned (cache MISS).
    pub fn get_context_packet(&self, node_id: NodeId) -> Result<ContextPacket, DbError> {
        let id_str = node_id.to_string();

        // Try the cache first (locked scope — drop before compute to avoid deadlock).
        {
            let db = self.db();
            match cic_cache::get_cached_packet(&db, &id_str) {
                Ok(Some(packet)) => return Ok(packet),
                Ok(None) => {}
                Err(e) => return Err(DbError::Other(e.to_string())),
            }
        }

        // Cache miss — compute via the GraphBackend trait (re-acquires lock per call).
        let packet = compute_context_packet_for_backend(self, node_id)
            .map_err(|e| DbError::Other(e.to_string()))?;

        // Persist the result.
        {
            let db = self.db();
            cic_cache::store_packet(&db, &id_str, &packet)
                .map_err(|e| DbError::Other(e.to_string()))?;
        }

        Ok(packet)
    }

    /// Mark the cache entries of `node_id` and all affected nodes stale.
    ///
    /// Returns the number of cache rows marked stale. Callers rarely need to
    /// call this directly — `GraphBackend` write methods call it automatically.
    pub fn invalidate_node(&self, node_id: NodeId) -> Result<usize, DbError> {
        let id_str = node_id.to_string();
        let db = self.db();
        cic_cache::compute_and_invalidate(&db, &id_str).map_err(|e| DbError::Other(e.to_string()))
    }

    // ─── Bulk replace from AilGraph ──────────────────────────────────────────

    /// Replace the current database contents with the state of `graph`.
    ///
    /// Clears the `nodes`, `contracts`, `edges`, `cic_cache`, and `embeddings`
    /// tables, then re-inserts every node and edge in migration order:
    /// nodes first, Ev child edges in BFS order (`node.children`), Eh
    /// next-sibling edges, then Ed diagonal references. The embedding index is
    /// always cleared — callers that need embeddings must re-run `ail reindex`.
    ///
    /// Used by MCP / batch workflows that edit an in-memory `AilGraph` and must
    /// flush the result back to `.ail.db`. Drop-and-reinsert is chosen over
    /// row-level diff for simplicity; write volume is bounded by project size.
    pub fn save_from_graph(&mut self, graph: &AilGraph) -> Result<(), DbError> {
        // Step 1 — clear in FK-safe order inside a single transaction.
        {
            let mut db = self.db();
            let tx = db.transaction()?;
            // `edges`, `contracts`, `cic_cache`, `embeddings` all FK → nodes.id
            // with ON DELETE CASCADE, but deleting them explicitly keeps the
            // behavior independent of trigger configuration.
            tx.execute("DELETE FROM edges", [])?;
            tx.execute("DELETE FROM contracts", [])?;
            tx.execute("DELETE FROM cic_cache", [])?;
            tx.execute("DELETE FROM embeddings", [])?;
            tx.execute("DELETE FROM nodes", [])?;
            tx.commit()?;
        }

        // Step 2 — insert all nodes (parent_id NULL, position 0, depth 0).
        for node in graph.all_nodes() {
            self.add_node(node.clone())
                .map_err(|e| DbError::Other(format!("save_from_graph add_node: {e}")))?;
        }

        // Step 3 — Ev edges in BFS order. Use `GraphBackend::children` so this
        // works for graphs built via `add_edge` (where `node.children` is
        // `None`) as well as parser-built graphs (where it is `Some`).
        // Inserting children in list order guarantees positions 0, 1, 2 …
        // because `add_edge(Ev)` assigns `position = max_child_position + 1`.
        if let Some(root_id) = graph.root_id() {
            let mut queue: VecDeque<NodeId> = VecDeque::new();
            queue.push_back(root_id);
            while let Some(node_id) = queue.pop_front() {
                let children = GraphBackend::children(graph, node_id).map_err(|e| {
                    DbError::Other(format!("save_from_graph children {node_id}: {e}"))
                })?;
                for child_id in children {
                    self.add_edge(node_id, child_id, EdgeKind::Ev)
                        .map_err(|e| {
                            DbError::Other(format!(
                                "save_from_graph add_edge Ev {node_id}->{child_id}: {e}"
                            ))
                        })?;
                    queue.push_back(child_id);
                }
            }
        }

        // Step 4 — Eh next-sibling edges (one directed edge per pair).
        for node in graph.all_nodes() {
            match graph.next_sibling_of(node.id) {
                Ok(Some(next_id)) => {
                    self.add_edge(node.id, next_id, EdgeKind::Eh).map_err(|e| {
                        DbError::Other(format!(
                            "save_from_graph add_edge Eh {}->{next_id}: {e}",
                            node.id
                        ))
                    })?;
                }
                Ok(None) => {}
                Err(e) => {
                    return Err(DbError::Other(format!(
                        "save_from_graph next_sibling_of {}: {e}",
                        node.id
                    )))
                }
            }
        }

        // Step 5 — Ed diagonal edges (outgoing, each directed edge once).
        for node in graph.all_nodes() {
            let targets = graph.outgoing_diagonal_refs_of(node.id).map_err(|e| {
                DbError::Other(format!(
                    "save_from_graph outgoing_diagonal_refs_of {}: {e}",
                    node.id
                ))
            })?;
            for target_id in targets {
                self.add_edge(node.id, target_id, EdgeKind::Ed)
                    .map_err(|e| {
                        DbError::Other(format!(
                            "save_from_graph add_edge Ed {}->{target_id}: {e}",
                            node.id
                        ))
                    })?;
            }
        }

        Ok(())
    }

    // ─── FTS5 search ─────────────────────────────────────────────────────────

    /// Search nodes using the FTS5 `search_fts` virtual table.
    ///
    /// Returns up to `limit` [`SearchResult`] values ranked by BM25 relevance
    /// (best match first). Each result includes the node's `depth` in the Ev
    /// tree and the `path` from root down to the matched node.
    ///
    /// The `query` string is passed directly to FTS5 `MATCH`. Valid FTS5
    /// boolean operators (`AND`, `OR`, `NOT`) work as intended. Malformed FTS5
    /// syntax returns `Err(DbError::Other(...))`. An empty query returns
    /// `Ok(vec![])`.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, DbError> {
        let db = self.db();
        fts_search::search(&db, query, limit)
    }

    /// Return `Some(true)` if the cache entry for `node_id` is valid,
    /// `Some(false)` if stale, `None` if absent.
    ///
    /// Used in tests to inspect cache state without going through the full
    /// `get_context_packet` path.
    pub fn cic_cache_valid(&self, node_id: NodeId) -> Option<bool> {
        let id_str = node_id.to_string();
        let db = self.db();
        match db.query_row(
            "SELECT valid FROM cic_cache WHERE node_id = ?",
            rusqlite::params![id_str],
            |row| row.get::<_, i64>(0),
        ) {
            Ok(v) => Some(v != 0),
            Err(_) => None,
        }
    }
}

// ─── Free helpers — take &Connection directly (no re-lock risk) ──────────────

pub(crate) fn configure_pragmas(conn: &Connection) -> Result<(), DbError> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(())
}

pub(crate) fn load_contracts(
    conn: &Connection,
    node_id: &str,
) -> Result<Vec<Contract>, GraphError> {
    let mut stmt = conn
        .prepare("SELECT kind, expression FROM contracts WHERE node_id = ? ORDER BY position")
        .map_err(|e| GraphError::Storage(e.to_string()))?;

    let contracts = stmt
        .query_map(rusqlite::params![node_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| GraphError::Storage(e.to_string()))?
        .map(|res| {
            res.map_err(|e| GraphError::Storage(e.to_string()))
                .and_then(|(kind_s, expr_s)| {
                    let kind = contract_kind_from_sql(&kind_s).map_err(GraphError::from)?;
                    Ok(Contract {
                        kind,
                        expression: Expression(expr_s),
                    })
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(contracts)
}

pub(crate) fn node_exists(conn: &Connection, id: &str) -> Result<bool, GraphError> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM nodes WHERE id = ?",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .map_err(|e| GraphError::Storage(e.to_string()))?;
    Ok(count > 0)
}

pub(crate) fn get_depth(conn: &Connection, id: &str) -> Result<i64, GraphError> {
    match conn.query_row(
        "SELECT depth FROM nodes WHERE id = ?",
        rusqlite::params![id],
        |row| row.get::<_, i64>(0),
    ) {
        Ok(d) => Ok(d),
        Err(rusqlite::Error::QueryReturnedNoRows) => Err(GraphError::NodeNotFound(
            node_id_from_sql(id).unwrap_or_else(|_| NodeId::new()),
        )),
        Err(e) => Err(GraphError::Storage(e.to_string())),
    }
}

pub(crate) fn get_max_child_position(
    conn: &Connection,
    parent_id: &str,
) -> Result<i64, GraphError> {
    let max: Option<i64> = conn
        .query_row(
            "SELECT MAX(position) FROM nodes WHERE parent_id = ?",
            rusqlite::params![parent_id],
            |row| row.get(0),
        )
        .map_err(|e| GraphError::Storage(e.to_string()))?;
    Ok(max.unwrap_or(-1))
}

pub(crate) fn get_parent_and_position(
    conn: &Connection,
    id: &str,
) -> Result<(Option<String>, i64), GraphError> {
    match conn.query_row(
        "SELECT parent_id, position FROM nodes WHERE id = ?",
        rusqlite::params![id],
        |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, i64>(1)?)),
    ) {
        Ok(pair) => Ok(pair),
        Err(rusqlite::Error::QueryReturnedNoRows) => Err(GraphError::NodeNotFound(
            node_id_from_sql(id).unwrap_or_else(|_| NodeId::new()),
        )),
        Err(e) => Err(GraphError::Storage(e.to_string())),
    }
}

pub(crate) fn next_contract_position(conn: &Connection, node_id: &str) -> Result<i64, GraphError> {
    let max: Option<i64> = conn
        .query_row(
            "SELECT MAX(position) FROM contracts WHERE node_id = ?",
            rusqlite::params![node_id],
            |row| row.get(0),
        )
        .map_err(|e| GraphError::Storage(e.to_string()))?;
    Ok(max.unwrap_or(-1) + 1)
}

pub(crate) fn insert_contracts(
    conn: &Connection,
    node_id: &str,
    contracts: &[Contract],
    start_position: i64,
) -> Result<(), GraphError> {
    for (i, contract) in contracts.iter().enumerate() {
        let kind_str = contract_kind_to_sql(&contract.kind).map_err(GraphError::from)?;
        conn.execute(
            "INSERT OR IGNORE INTO contracts (id, node_id, kind, expression, position) \
             VALUES (?, ?, ?, ?, ?)",
            rusqlite::params![
                Uuid::new_v4().to_string(),
                node_id,
                kind_str,
                contract.expression.0,
                start_position + i as i64,
            ],
        )
        .map_err(|e| GraphError::Storage(e.to_string()))?;
    }
    Ok(())
}

pub(crate) fn fetch_node(conn: &Connection, id: NodeId) -> Result<Option<Node>, GraphError> {
    let id_str = id.to_string();
    match conn.query_row(
        "SELECT id, intent, pattern, expression, metadata FROM nodes WHERE id = ?",
        rusqlite::params![id_str],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
            ))
        },
    ) {
        Ok((id_s, intent, pattern_s, expression, metadata_s)) => {
            let contracts = load_contracts(conn, &id_s)?;
            let node = row_to_node(id_s, intent, pattern_s, expression, metadata_s, contracts)
                .map_err(GraphError::from)?;
            Ok(Some(node))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(GraphError::Storage(e.to_string())),
    }
}

pub(crate) fn edge_kind_str(kind: &EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Ev => "ev",
        EdgeKind::Eh => "eh",
        EdgeKind::Ed => "ed",
    }
}
