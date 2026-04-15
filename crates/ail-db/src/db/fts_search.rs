use ail_graph::search::SearchResult;
use rusqlite::Connection;

use crate::errors::DbError;

use super::node_serde::{node_id_from_sql, pattern_from_sql};

/// Run a BM25-ranked FTS5 search over the `search_fts` virtual table.
///
/// Returns up to `limit` results ordered by relevance (best first). FTS5's
/// built-in BM25 `rank` column is negative — the most-relevant result has the
/// most-negative rank. We negate it to produce a positive `score` value.
///
/// **FTS5 query syntax** — `query` is passed directly to `MATCH`. Valid FTS5
/// boolean operators (`AND`, `OR`, `NOT`) and phrase quotes work as intended.
/// A malformed FTS5 expression (e.g. unbalanced quotes) propagates as
/// `Err(DbError::Other(...))` — no panic. An empty or whitespace-only query
/// returns `Ok(vec![])` without touching SQLite.
pub(crate) fn search(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, DbError> {
    if limit == 0 || query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let sql = "
        SELECT n.id, n.intent, n.depth, n.pattern, n.name, search_fts.rank
        FROM search_fts
        JOIN nodes n ON n.rowid = search_fts.rowid
        WHERE search_fts MATCH ?
        ORDER BY rank
        LIMIT ?
    ";

    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| DbError::Other(e.to_string()))?;

    // Clamp limit to i64::MAX — usize::MAX would cast to -1 (SQLite "no limit").
    let limit_i64 = i64::try_from(limit).unwrap_or(i64::MAX);

    let rows: Vec<(String, String, i64, String, Option<String>, f64)> = stmt
        .query_map(rusqlite::params![query, limit_i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, f64>(5)?,
            ))
        })
        .map_err(|e| DbError::Other(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| DbError::Other(e.to_string()))?;

    let mut results = Vec::with_capacity(rows.len());
    for (id_str, intent, depth_i64, pattern_str, name, rank) in rows {
        let node_id = node_id_from_sql(&id_str)?;
        let pattern = pattern_from_sql(&pattern_str)?;
        let depth = depth_i64 as usize;
        // FTS5 rank is negative BM25; negate for a positive relevance score.
        let score = (-rank) as f32;
        let path = build_path(conn, &id_str)?;

        results.push(SearchResult {
            node_id,
            score,
            intent,
            name,
            pattern,
            depth,
            path,
        });
    }

    Ok(results)
}

/// Walk the parent chain from `node_id` up to the root, building a path of
/// labels (node name if set, otherwise the first word of the intent). The
/// path is returned in root-to-node order.
///
/// Uses direct SQL queries on the provided `&Connection` rather than calling
/// through `GraphBackend`, avoiding mutex re-entry (callers already hold the
/// `SqliteGraph` connection lock).
/// Maximum parent-chain depth before `build_path` aborts with an error.
///
/// Guards against infinite loops if a cycle is ever written into `parent_id`
/// (e.g. via direct DB manipulation or a future bug). AIL trees are expected
/// to stay well under this limit in normal use.
const MAX_PATH_DEPTH: usize = 1024;

fn build_path(conn: &Connection, node_id: &str) -> Result<Vec<String>, DbError> {
    let mut path: Vec<String> = Vec::new();
    let mut current_id = node_id.to_string();
    let mut depth_guard = 0usize;

    loop {
        depth_guard += 1;
        if depth_guard > MAX_PATH_DEPTH {
            return Err(DbError::Other(format!(
                "build_path: cycle or excessive depth detected at node {current_id}"
            )));
        }

        let result = conn.query_row(
            "SELECT name, intent, parent_id FROM nodes WHERE id = ?",
            rusqlite::params![current_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        );

        match result {
            Ok((name, intent, parent_opt)) => {
                let label = name.unwrap_or_else(|| first_word(&intent));
                path.push(label);
                match parent_opt {
                    Some(pid) => current_id = pid,
                    None => break,
                }
            }
            Err(e) => return Err(DbError::Other(e.to_string())),
        }
    }

    path.reverse();
    Ok(path)
}

fn first_word(s: &str) -> String {
    s.split_whitespace().next().unwrap_or(s).to_string()
}
