//! CIC cache — persists and invalidates [`ContextPacket`]s in SQLite.
//!
//! All operations accept a raw `&Connection` so they can be called inside
//! an existing mutex-lock scope without risk of deadlock.

use ail_graph::{errors::GraphError, ContextPacket};
use rusqlite::Connection;

/// Retrieve a **valid** cached packet for `node_id`.
///
/// Returns `None` when no entry exists or the entry is stale (`valid = 0`).
pub(crate) fn get_cached_packet(
    conn: &Connection,
    node_id: &str,
) -> Result<Option<ContextPacket>, GraphError> {
    match conn.query_row(
        "SELECT packet_json FROM cic_cache WHERE node_id = ? AND valid = 1",
        rusqlite::params![node_id],
        |row| row.get::<_, String>(0),
    ) {
        Ok(json) => {
            let packet = serde_json::from_str(&json).map_err(GraphError::Serialization)?;
            Ok(Some(packet))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(GraphError::Storage(e.to_string())),
    }
}

/// Upsert a computed packet into the cache, marking it valid.
pub(crate) fn store_packet(
    conn: &Connection,
    node_id: &str,
    packet: &ContextPacket,
) -> Result<(), GraphError> {
    let json = serde_json::to_string(packet).map_err(GraphError::Serialization)?;
    conn.execute(
        "INSERT INTO cic_cache (node_id, packet_json, valid) VALUES (?1, ?2, 1)
         ON CONFLICT(node_id) DO UPDATE SET
           packet_json = excluded.packet_json,
           valid       = 1,
           computed_at = datetime('now')",
        rusqlite::params![node_id, json],
    )
    .map_err(|e| GraphError::Storage(e.to_string()))?;
    Ok(())
}

/// Compute the set of all node IDs affected when `node_id` changes and mark
/// their `cic_cache` entries stale (`valid = 0`).
///
/// Uses four-rule CIC invalidation via a single recursive SQL CTE:
///
/// - **Rule 1 DOWN** — all descendants of the changed node.
/// - **Rule 2 UP** — all ancestors of the changed node.
/// - **Rule 3 ACROSS** — all next siblings (positional + Eh-edge) and their
///   descendants.
/// - **Rule 4 DIAGONAL** — all nodes that have an incoming `ed` edge to the
///   changed node (i.e. nodes that reference it).
///
/// Returns the number of cache rows marked stale.
pub(crate) fn compute_and_invalidate(
    conn: &Connection,
    node_id: &str,
) -> Result<usize, GraphError> {
    let affected = collect_affected_ids(conn, node_id)?;
    if affected.is_empty() {
        return Ok(0);
    }

    let placeholders = affected.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let update_sql = format!("UPDATE cic_cache SET valid = 0 WHERE node_id IN ({placeholders})");
    let count = conn
        .execute(&update_sql, rusqlite::params_from_iter(affected.iter()))
        .map_err(|e| GraphError::Storage(e.to_string()))?;

    Ok(count)
}

// ─── private helpers ──────────────────────────────────────────────────────────

/// Run the four-rule CTE and return all affected node ID strings.
fn collect_affected_ids(conn: &Connection, node_id: &str) -> Result<Vec<String>, GraphError> {
    const AFFECTED_SQL: &str = "
        WITH RECURSIVE
          -- Rule 1 DOWN: all descendants
          desc(id) AS (
            SELECT id FROM nodes WHERE parent_id = ?1
            UNION ALL
            SELECT n.id FROM nodes n INNER JOIN desc ON n.parent_id = desc.id
          ),
          -- Rule 2 UP: all ancestors via parent_id chain
          ancs(id) AS (
            SELECT parent_id FROM nodes WHERE id = ?1 AND parent_id IS NOT NULL
            UNION ALL
            SELECT n.parent_id FROM nodes n
              INNER JOIN ancs ON n.id = ancs.id
              WHERE n.parent_id IS NOT NULL
          ),
          -- Rule 3 ACROSS (positional siblings): next siblings + their descendants
          node_info(par, pos) AS (
            SELECT parent_id, position FROM nodes WHERE id = ?1
          ),
          pos_next(id) AS (
            SELECT n.id FROM nodes n, node_info
              WHERE node_info.par IS NOT NULL
                AND n.parent_id = node_info.par
                AND n.position > node_info.pos
          ),
          pos_next_desc(id) AS (
            SELECT id FROM pos_next
            UNION ALL
            SELECT n.id FROM nodes n INNER JOIN pos_next_desc ON n.parent_id = pos_next_desc.id
          ),
          -- Rule 3 ACROSS (Eh-edge siblings): next siblings + their descendants
          eh_next(id) AS (
            SELECT target_id FROM edges WHERE source_id = ?1 AND kind = 'eh'
            UNION ALL
            SELECT e.target_id FROM edges e
              INNER JOIN eh_next ON e.source_id = eh_next.id
              WHERE e.kind = 'eh'
          ),
          eh_next_desc(id) AS (
            SELECT id FROM eh_next
            UNION ALL
            SELECT n.id FROM nodes n INNER JOIN eh_next_desc ON n.parent_id = eh_next_desc.id
          ),
          -- Rule 4 DIAGONAL: nodes that Ed-reference this node (incoming)
          diag(id) AS (
            SELECT source_id FROM edges WHERE target_id = ?1 AND kind = 'ed'
          ),
          -- Rule 5 PROMOTION: for Check nodes, walk up through ancestors and
          -- at each ancestor level collect positional siblings-after + their
          -- descendants. This mirrors the CIC read path where
          -- collect_promoted_facts walks the full ancestor chain and recurses
          -- into preceding Do bodies to find nested Check nodes.
          -- The seed row filters on pattern = 'check' so this CTE is empty
          -- (and produces no rows) for non-Check node changes.
          anc_chain(id, par, pos) AS (
            SELECT id, parent_id, position FROM nodes
              WHERE id = ?1 AND pattern = 'check'
            UNION ALL
            SELECT n.id, n.parent_id, n.position
              FROM nodes n INNER JOIN anc_chain ac ON n.id = ac.par
          ),
          promo_next(id) AS (
            SELECT n.id FROM nodes n
              INNER JOIN anc_chain ac
                ON n.parent_id = ac.par AND n.position > ac.pos
              WHERE ac.par IS NOT NULL
          ),
          promo_next_desc(id) AS (
            SELECT id FROM promo_next
            UNION ALL
            SELECT n.id FROM nodes n
              INNER JOIN promo_next_desc ON n.parent_id = promo_next_desc.id
          )
        SELECT ?1 AS id
        UNION SELECT id FROM desc
        UNION SELECT id FROM ancs
        UNION SELECT id FROM pos_next_desc
        UNION SELECT id FROM eh_next_desc
        UNION SELECT id FROM diag
        UNION SELECT id FROM promo_next_desc
    ";

    let mut stmt = conn
        .prepare(AFFECTED_SQL)
        .map_err(|e| GraphError::Storage(e.to_string()))?;

    let ids = stmt
        .query_map(rusqlite::params![node_id], |row| row.get::<_, String>(0))
        .map_err(|e| GraphError::Storage(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| GraphError::Storage(e.to_string()))?;

    Ok(ids)
}
