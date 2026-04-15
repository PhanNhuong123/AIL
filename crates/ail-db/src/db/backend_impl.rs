use ail_graph::{
    errors::GraphError,
    graph::GraphBackend,
    types::{Contract, EdgeKind, Node, NodeId, Pattern},
};

use super::cic_cache;
use super::node_serde::{node_id_from_sql, node_to_row, pattern_to_sql};
use super::sqlite_graph::{
    edge_kind_str, fetch_node, get_depth, get_max_child_position, get_parent_and_position,
    insert_contracts, next_contract_position, node_exists,
};
use super::sqlite_graph::SqliteGraph;

impl GraphBackend for SqliteGraph {
    // ═══════════════════════════════════════════════════════════════════════
    // Node Operations
    // ═══════════════════════════════════════════════════════════════════════

    fn add_node(&mut self, node: Node) -> Result<NodeId, GraphError> {
        let id = node.id;
        let row = node_to_row(&node).map_err(GraphError::from)?;
        let db = self.db();

        if node_exists(&db, &row.id)? {
            return Err(GraphError::DuplicateNodeId(id));
        }

        db.execute(
            "INSERT INTO nodes (id, intent, name, pattern, expression, metadata) \
             VALUES (?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                row.id,
                row.intent,
                row.name,
                row.pattern,
                row.expression,
                row.metadata,
            ],
        )
        .map_err(|e| GraphError::Storage(e.to_string()))?;

        insert_contracts(&db, &node.id.to_string(), &node.contracts, 0)?;

        Ok(id)
    }

    fn get_node(&self, id: NodeId) -> Result<Option<Node>, GraphError> {
        let db = self.db();
        fetch_node(&db, id)
    }

    fn update_node(&mut self, id: NodeId, node: Node) -> Result<(), GraphError> {
        let id_str = id.to_string();
        let row = node_to_row(&node).map_err(GraphError::from)?;
        let db = self.db();

        if !node_exists(&db, &id_str)? {
            return Err(GraphError::NodeNotFound(id));
        }

        // Update content columns; preserve parent_id, position, depth.
        db.execute(
            "UPDATE nodes SET intent=?, name=?, pattern=?, expression=?, metadata=?, \
             updated_at=datetime('now') WHERE id=?",
            rusqlite::params![
                row.intent,
                row.name,
                row.pattern,
                row.expression,
                row.metadata,
                id_str,
            ],
        )
        .map_err(|e| GraphError::Storage(e.to_string()))?;

        // Replace contracts: delete then re-insert.
        db.execute(
            "DELETE FROM contracts WHERE node_id = ?",
            rusqlite::params![id_str],
        )
        .map_err(|e| GraphError::Storage(e.to_string()))?;

        insert_contracts(&db, &id_str, &node.contracts, 0)?;

        // Invalidate: this node + descendants + ancestors + next-siblings.
        cic_cache::compute_and_invalidate(&db, &id_str)?;

        Ok(())
    }

    fn remove_node(&mut self, id: NodeId) -> Result<(), GraphError> {
        let id_str = id.to_string();
        let db = self.db();

        if !node_exists(&db, &id_str)? {
            return Err(GraphError::NodeNotFound(id));
        }

        // Invalidate BEFORE delete — the CTE must walk the graph while it still
        // exists. CASCADE will remove the cic_cache row for this node itself.
        cic_cache::compute_and_invalidate(&db, &id_str)?;

        // CASCADE handles children nodes, contracts, edges, and cic_cache rows.
        db.execute("DELETE FROM nodes WHERE id = ?", rusqlite::params![id_str])
            .map_err(|e| GraphError::Storage(e.to_string()))?;

        Ok(())
    }

    fn all_node_ids(&self) -> Result<Vec<NodeId>, GraphError> {
        let db = self.db();
        let mut stmt = db
            .prepare("SELECT id FROM nodes")
            .map_err(|e| GraphError::Storage(e.to_string()))?;

        let ids = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| GraphError::Storage(e.to_string()))?
            .map(|res| {
                res.map_err(|e| GraphError::Storage(e.to_string()))
                    .and_then(|s| node_id_from_sql(&s).map_err(GraphError::from))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ids)
    }

    fn node_count(&self) -> usize {
        let db = self.db();
        db.query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get::<_, i64>(0))
            .unwrap_or(0) as usize
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Edge Operations
    // ═══════════════════════════════════════════════════════════════════════

    fn add_edge(&mut self, from: NodeId, to: NodeId, kind: EdgeKind) -> Result<(), GraphError> {
        let from_str = from.to_string();
        let to_str = to.to_string();
        let db = self.db();

        match kind {
            EdgeKind::Ev => {
                let parent_depth = get_depth(&db, &from_str)?;
                let max_pos = get_max_child_position(&db, &from_str)?;

                let rows_changed = db
                    .execute(
                        "UPDATE nodes SET parent_id=?, depth=?, position=? WHERE id=?",
                        rusqlite::params![from_str, parent_depth + 1, max_pos + 1, to_str],
                    )
                    .map_err(|e| GraphError::Storage(e.to_string()))?;

                if rows_changed == 0 {
                    return Err(GraphError::NodeNotFound(to));
                }

                // Invalidate: new child's ancestor chain (incl. `from`) goes stale
                // via Rule 2 UP. The child itself has no cache entry yet.
                cic_cache::compute_and_invalidate(&db, &to_str)?;
            }
            EdgeKind::Eh => {
                db.execute(
                    "INSERT OR REPLACE INTO edges (source_id, target_id, kind) VALUES (?, ?, ?)",
                    rusqlite::params![from_str, to_str, edge_kind_str(&kind)],
                )
                .map_err(|e| GraphError::Storage(e.to_string()))?;

                // Invalidate: `from`'s next-siblings (including `to`) via Rule 3 ACROSS.
                cic_cache::compute_and_invalidate(&db, &from_str)?;
            }
            EdgeKind::Ed => {
                db.execute(
                    "INSERT OR REPLACE INTO edges (source_id, target_id, kind) VALUES (?, ?, ?)",
                    rusqlite::params![from_str, to_str, edge_kind_str(&kind)],
                )
                .map_err(|e| GraphError::Storage(e.to_string()))?;

                // Invalidate both endpoints: `from`'s call_contracts change,
                // and `to` now has a new incoming diagonal reference.
                cic_cache::compute_and_invalidate(&db, &from_str)?;
                cic_cache::compute_and_invalidate(&db, &to_str)?;
            }
        }

        Ok(())
    }

    fn remove_edge_by_kind(
        &mut self,
        from: NodeId,
        to: NodeId,
        kind: EdgeKind,
    ) -> Result<(), GraphError> {
        let from_str = from.to_string();
        let to_str = to.to_string();
        let db = self.db();

        match kind {
            EdgeKind::Ev => {
                // Invalidate BEFORE removing — CTE walks the current parent chain.
                cic_cache::compute_and_invalidate(&db, &to_str)?;

                let rows_changed = db
                    .execute(
                        "UPDATE nodes SET parent_id=NULL, depth=0, position=0 \
                         WHERE id=? AND parent_id=?",
                        rusqlite::params![to_str, from_str],
                    )
                    .map_err(|e| GraphError::Storage(e.to_string()))?;

                if rows_changed == 0 {
                    return Err(GraphError::EdgeKindNotFound { from, to, kind });
                }
            }
            EdgeKind::Eh => {
                // Invalidate BEFORE removing — `from`'s next-siblings include `to`.
                cic_cache::compute_and_invalidate(&db, &from_str)?;

                let rows_changed = db
                    .execute(
                        "DELETE FROM edges WHERE source_id=? AND target_id=? AND kind=?",
                        rusqlite::params![from_str, to_str, edge_kind_str(&kind)],
                    )
                    .map_err(|e| GraphError::Storage(e.to_string()))?;

                if rows_changed == 0 {
                    return Err(GraphError::EdgeKindNotFound { from, to, kind });
                }
            }
            EdgeKind::Ed => {
                // Invalidate both endpoints BEFORE removing.
                cic_cache::compute_and_invalidate(&db, &from_str)?;
                cic_cache::compute_and_invalidate(&db, &to_str)?;

                let rows_changed = db
                    .execute(
                        "DELETE FROM edges WHERE source_id=? AND target_id=? AND kind=?",
                        rusqlite::params![from_str, to_str, edge_kind_str(&kind)],
                    )
                    .map_err(|e| GraphError::Storage(e.to_string()))?;

                if rows_changed == 0 {
                    return Err(GraphError::EdgeKindNotFound { from, to, kind });
                }
            }
        }

        Ok(())
    }

    fn children(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError> {
        let db = self.db();
        let id_str = id.to_string();

        let mut stmt = db
            .prepare("SELECT id FROM nodes WHERE parent_id = ? ORDER BY position ASC")
            .map_err(|e| GraphError::Storage(e.to_string()))?;

        let ids = stmt
            .query_map(rusqlite::params![id_str], |row| row.get::<_, String>(0))
            .map_err(|e| GraphError::Storage(e.to_string()))?
            .map(|res| {
                res.map_err(|e| GraphError::Storage(e.to_string()))
                    .and_then(|s| node_id_from_sql(&s).map_err(GraphError::from))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ids)
    }

    fn siblings_before(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError> {
        let db = self.db();
        let id_str = id.to_string();
        let (parent_id_opt, pos) = get_parent_and_position(&db, &id_str)?;

        if let Some(parent_id) = parent_id_opt {
            let mut stmt = db
                .prepare(
                    "SELECT id FROM nodes WHERE parent_id = ? AND position < ? \
                     ORDER BY position ASC",
                )
                .map_err(|e| GraphError::Storage(e.to_string()))?;

            let ids: Result<Vec<NodeId>, GraphError> = stmt
                .query_map(rusqlite::params![parent_id, pos], |row| {
                    row.get::<_, String>(0)
                })
                .map_err(|e| GraphError::Storage(e.to_string()))?
                .map(|res| {
                    res.map_err(|e| GraphError::Storage(e.to_string()))
                        .and_then(|s| node_id_from_sql(&s).map_err(GraphError::from))
                })
                .collect();
            ids
        } else {
            // No parent: walk Eh edges backward. (A, B, 'eh') means A precedes B.
            let mut result = Vec::new();
            let mut current = id_str;

            loop {
                match db.query_row(
                    "SELECT source_id FROM edges WHERE target_id = ? AND kind = 'eh' LIMIT 1",
                    rusqlite::params![current],
                    |row| row.get::<_, String>(0),
                ) {
                    Ok(prev) => {
                        result.push(node_id_from_sql(&prev).map_err(GraphError::from)?);
                        current = prev;
                    }
                    Err(rusqlite::Error::QueryReturnedNoRows) => break,
                    Err(e) => return Err(GraphError::Storage(e.to_string())),
                }
            }

            result.reverse(); // walk was newest-first; reverse for earliest-first
            Ok(result)
        }
    }

    fn siblings_after(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError> {
        let db = self.db();
        let id_str = id.to_string();
        let (parent_id_opt, pos) = get_parent_and_position(&db, &id_str)?;

        if let Some(parent_id) = parent_id_opt {
            let mut stmt = db
                .prepare(
                    "SELECT id FROM nodes WHERE parent_id = ? AND position > ? \
                     ORDER BY position ASC",
                )
                .map_err(|e| GraphError::Storage(e.to_string()))?;

            let ids: Result<Vec<NodeId>, GraphError> = stmt
                .query_map(rusqlite::params![parent_id, pos], |row| {
                    row.get::<_, String>(0)
                })
                .map_err(|e| GraphError::Storage(e.to_string()))?
                .map(|res| {
                    res.map_err(|e| GraphError::Storage(e.to_string()))
                        .and_then(|s| node_id_from_sql(&s).map_err(GraphError::from))
                })
                .collect();
            ids
        } else {
            // No parent: walk Eh edges forward.
            let mut result = Vec::new();
            let mut current = id_str;

            loop {
                match db.query_row(
                    "SELECT target_id FROM edges WHERE source_id = ? AND kind = 'eh' LIMIT 1",
                    rusqlite::params![current],
                    |row| row.get::<_, String>(0),
                ) {
                    Ok(next) => {
                        result.push(node_id_from_sql(&next).map_err(GraphError::from)?);
                        current = next;
                    }
                    Err(rusqlite::Error::QueryReturnedNoRows) => break,
                    Err(e) => return Err(GraphError::Storage(e.to_string())),
                }
            }

            Ok(result)
        }
    }

    fn parent(&self, id: NodeId) -> Result<Option<NodeId>, GraphError> {
        let db = self.db();
        let (parent_opt, _) = get_parent_and_position(&db, &id.to_string())?;
        match parent_opt {
            Some(p) => Ok(Some(node_id_from_sql(&p).map_err(GraphError::from)?)),
            None => Ok(None),
        }
    }

    fn diagonal_refs(&self, id: NodeId) -> Result<Vec<(NodeId, EdgeKind)>, GraphError> {
        let db = self.db();
        let id_str = id.to_string();

        let mut stmt = db
            .prepare(
                "SELECT source_id, target_id FROM edges \
                 WHERE kind = 'ed' AND (source_id = ? OR target_id = ?)",
            )
            .map_err(|e| GraphError::Storage(e.to_string()))?;

        let refs = stmt
            .query_map(rusqlite::params![id_str, id_str], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| GraphError::Storage(e.to_string()))?
            .map(|res| {
                res.map_err(|e| GraphError::Storage(e.to_string()))
                    .and_then(|(src, tgt)| {
                        let other = if src == id_str { &tgt } else { &src };
                        let other_id = node_id_from_sql(other).map_err(GraphError::from)?;
                        Ok((other_id, EdgeKind::Ed))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(refs)
    }

    fn outgoing_diagonal_refs(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError> {
        let db = self.db();
        let id_str = id.to_string();

        let mut stmt = db
            .prepare("SELECT target_id FROM edges WHERE source_id = ? AND kind = 'ed'")
            .map_err(|e| GraphError::Storage(e.to_string()))?;

        let ids = stmt
            .query_map(rusqlite::params![id_str], |row| row.get::<_, String>(0))
            .map_err(|e| GraphError::Storage(e.to_string()))?
            .map(|res| {
                res.map_err(|e| GraphError::Storage(e.to_string()))
                    .and_then(|s| node_id_from_sql(&s).map_err(GraphError::from))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ids)
    }

    fn ancestors(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError> {
        let db = self.db();
        let mut result = Vec::new();
        let mut current = id.to_string();

        loop {
            match db.query_row(
                "SELECT parent_id FROM nodes WHERE id = ?",
                rusqlite::params![current],
                |row| row.get::<_, Option<String>>(0),
            ) {
                Ok(Some(p)) => {
                    result.push(node_id_from_sql(&p).map_err(GraphError::from)?);
                    current = p;
                }
                Ok(None) => break, // root node
                Err(rusqlite::Error::QueryReturnedNoRows) => break,
                Err(e) => return Err(GraphError::Storage(e.to_string())),
            }
        }

        Ok(result)
    }

    fn all_descendants(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError> {
        let db = self.db();
        let id_str = id.to_string();

        let mut stmt = db
            .prepare(
                "WITH RECURSIVE r(id) AS (
                     SELECT id FROM nodes WHERE parent_id = ?
                     UNION ALL
                     SELECT n.id FROM nodes n JOIN r ON n.parent_id = r.id
                 )
                 SELECT id FROM r",
            )
            .map_err(|e| GraphError::Storage(e.to_string()))?;

        let ids = stmt
            .query_map(rusqlite::params![id_str], |row| row.get::<_, String>(0))
            .map_err(|e| GraphError::Storage(e.to_string()))?
            .map(|res| {
                res.map_err(|e| GraphError::Storage(e.to_string()))
                    .and_then(|s| node_id_from_sql(&s).map_err(GraphError::from))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ids)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Query Operations
    // ═══════════════════════════════════════════════════════════════════════

    fn find_by_pattern(&self, pattern: Pattern) -> Result<Vec<NodeId>, GraphError> {
        let db = self.db();
        let pattern_str = pattern_to_sql(&pattern).map_err(GraphError::from)?;

        let mut stmt = db
            .prepare("SELECT id FROM nodes WHERE pattern = ?")
            .map_err(|e| GraphError::Storage(e.to_string()))?;

        let ids = stmt
            .query_map(rusqlite::params![pattern_str], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|e| GraphError::Storage(e.to_string()))?
            .map(|res| {
                res.map_err(|e| GraphError::Storage(e.to_string()))
                    .and_then(|s| node_id_from_sql(&s).map_err(GraphError::from))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ids)
    }

    fn find_by_name(&self, name: &str) -> Result<Vec<NodeId>, GraphError> {
        let db = self.db();
        let mut stmt = db
            .prepare("SELECT id FROM nodes WHERE name = ?")
            .map_err(|e| GraphError::Storage(e.to_string()))?;

        let ids = stmt
            .query_map(rusqlite::params![name], |row| row.get::<_, String>(0))
            .map_err(|e| GraphError::Storage(e.to_string()))?
            .map(|res| {
                res.map_err(|e| GraphError::Storage(e.to_string()))
                    .and_then(|s| node_id_from_sql(&s).map_err(GraphError::from))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ids)
    }

    fn root_nodes(&self) -> Result<Vec<NodeId>, GraphError> {
        let db = self.db();
        let mut stmt = db
            .prepare("SELECT id FROM nodes WHERE parent_id IS NULL")
            .map_err(|e| GraphError::Storage(e.to_string()))?;

        let ids = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| GraphError::Storage(e.to_string()))?
            .map(|res| {
                res.map_err(|e| GraphError::Storage(e.to_string()))
                    .and_then(|s| node_id_from_sql(&s).map_err(GraphError::from))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ids)
    }

    fn depth(&self, id: NodeId) -> Result<usize, GraphError> {
        let db = self.db();
        let d = get_depth(&db, &id.to_string())?;
        Ok(d as usize)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Contract Operations
    // ═══════════════════════════════════════════════════════════════════════

    fn contracts(&self, id: NodeId) -> Result<Vec<Contract>, GraphError> {
        let db = self.db();
        let id_str = id.to_string();
        if !node_exists(&db, &id_str)? {
            return Err(GraphError::NodeNotFound(id));
        }
        super::sqlite_graph::load_contracts(&db, &id_str)
    }

    fn add_contract(&mut self, id: NodeId, contract: Contract) -> Result<(), GraphError> {
        let id_str = id.to_string();
        let db = self.db();

        if !node_exists(&db, &id_str)? {
            return Err(GraphError::NodeNotFound(id));
        }

        let pos = next_contract_position(&db, &id_str)?;
        insert_contracts(&db, &id_str, std::slice::from_ref(&contract), pos)?;

        // Invalidate: descendants inherit this node's contracts (Rule 1 DOWN).
        cic_cache::compute_and_invalidate(&db, &id_str)?;

        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Transaction Operations
    // ═══════════════════════════════════════════════════════════════════════

    fn begin_transaction(&mut self) -> Result<(), GraphError> {
        self.db()
            .execute_batch("BEGIN")
            .map_err(|e| GraphError::Storage(e.to_string()))
    }

    fn commit_transaction(&mut self) -> Result<(), GraphError> {
        self.db()
            .execute_batch("COMMIT")
            .map_err(|e| GraphError::Storage(e.to_string()))
    }

    fn rollback_transaction(&mut self) -> Result<(), GraphError> {
        self.db()
            .execute_batch("ROLLBACK")
            .map_err(|e| GraphError::Storage(e.to_string()))
    }
}
