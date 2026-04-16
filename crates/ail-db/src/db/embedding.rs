use std::collections::HashMap;

use ail_graph::types::NodeId;
use rusqlite::params;

use crate::errors::DbError;

use super::node_serde::node_id_from_sql;
use super::sqlite_graph::SqliteGraph;

// ─── EmbeddingModelStatus ─────────────────────────────────────────────────────

/// Compatibility state between the stored embedding model and the requested one.
///
/// Returned by [`SqliteGraph::check_embedding_model`]. Callers use this to
/// decide whether to reuse the persisted index or trigger a rebuild.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbeddingModelStatus {
    /// No vectors are stored yet (embedding_count == 0).
    Empty,
    /// Stored model matches `current_model` — index is usable.
    Compatible,
    /// Stored model differs from `current_model` — full reindex required.
    ///
    /// The `stored` field names the model that produced the existing vectors
    /// so callers can log a clear message before wiping.
    Changed { stored: String },
}

// ─── BLOB helpers ─────────────────────────────────────────────────────────────

/// Encode a `Vec<f32>` to a little-endian byte buffer.
///
/// Explicit LE for cross-arch portability; same-machine assumption documented
/// in review issue [10.3-A]. The decode path uses `f32::from_le_bytes`
/// symmetrically.
fn encode_vector(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Decode a little-endian byte buffer back to `Vec<f32>`.
fn decode_vector(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

// ─── impl SqliteGraph — project_meta accessors ────────────────────────────────

impl SqliteGraph {
    /// Read a value from `project_meta` by key. Returns `None` if absent.
    pub fn get_meta(&self, key: &str) -> Result<Option<String>, DbError> {
        let db = self.db();
        match db.query_row(
            "SELECT value FROM project_meta WHERE key = ?",
            params![key],
            |row| row.get::<_, String>(0),
        ) {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DbError::from(e)),
        }
    }

    /// Insert or replace a key-value pair in `project_meta`.
    pub fn set_meta(&self, key: &str, value: &str) -> Result<(), DbError> {
        let db = self.db();
        db.execute(
            "INSERT OR REPLACE INTO project_meta (key, value) VALUES (?, ?)",
            params![key, value],
        )?;
        Ok(())
    }
}

// ─── impl SqliteGraph — embedding persistence ─────────────────────────────────

impl SqliteGraph {
    /// Return the number of persisted embedding vectors.
    pub fn embedding_count(&self) -> Result<usize, DbError> {
        let db = self.db();
        let count: i64 = db.query_row("SELECT COUNT(*) FROM embeddings", [], |row| row.get(0))?;
        Ok(usize::try_from(count).unwrap_or(0))
    }

    /// Compare the stored `embedding_model` meta key against `current_model`.
    ///
    /// Decision table:
    /// - `embedding_count == 0` → [`EmbeddingModelStatus::Empty`] (ignore meta — rows may have
    ///   been wiped).
    /// - `count > 0`, meta absent → [`EmbeddingModelStatus::Compatible`] (old DB without meta,
    ///   treated as compatible to avoid spurious reindexes).
    /// - `count > 0`, meta present, matches → [`EmbeddingModelStatus::Compatible`].
    /// - `count > 0`, meta present, differs → [`EmbeddingModelStatus::Changed`].
    pub fn check_embedding_model(
        &self,
        current_model: &str,
    ) -> Result<EmbeddingModelStatus, DbError> {
        if self.embedding_count()? == 0 {
            return Ok(EmbeddingModelStatus::Empty);
        }
        match self.get_meta("embedding_model")? {
            None => Ok(EmbeddingModelStatus::Compatible),
            Some(stored) if stored == current_model => Ok(EmbeddingModelStatus::Compatible),
            Some(stored) => Ok(EmbeddingModelStatus::Changed { stored }),
        }
    }

    /// Persist a single node's embedding vector.
    ///
    /// Uses `INSERT OR REPLACE` so calling this twice for the same node updates
    /// the stored vector (incremental update path).
    ///
    /// # Errors
    ///
    /// Returns `DbError::Other` if `model_name` differs from the stored
    /// `embedding_model` meta key. This prevents silently mixing vectors from
    /// different models in the same index. Call [`clear_embeddings`] first when
    /// switching models.
    ///
    /// [`clear_embeddings`]: SqliteGraph::clear_embeddings
    pub fn save_embedding(
        &self,
        node_id: &NodeId,
        vector: &[f32],
        model_name: &str,
    ) -> Result<(), DbError> {
        // Mixed-index guard: reject if a different model is already recorded.
        if let Some(stored) = self.get_meta("embedding_model")? {
            if stored != model_name {
                return Err(DbError::Other(format!(
                    "model mismatch: index built with '{stored}', \
                     cannot add vector from '{model_name}'; call clear_embeddings() first"
                )));
            }
        }

        let blob = encode_vector(vector);
        let db = self.db();
        db.execute(
            "INSERT OR REPLACE INTO embeddings (node_id, vector, model_name) \
             VALUES (?, ?, ?)",
            params![node_id.to_string(), blob, model_name],
        )?;
        Ok(())
    }

    /// Persist multiple embedding vectors in a single transaction.
    ///
    /// This is the preferred path for initial indexing. Wrapping all inserts in
    /// one `BEGIN`/`COMMIT` avoids per-row fsyncs (~10 s for 1000 nodes with
    /// autocommit vs ~50 ms in a transaction).
    ///
    /// Sets the `embedding_model` meta key once at the end of the transaction.
    ///
    /// # Errors
    ///
    /// Returns `DbError::Other` if the stored meta model differs from
    /// `model_name` (same mixed-index guard as `save_embedding`). Call
    /// [`clear_embeddings`] first when switching models.
    ///
    /// [`clear_embeddings`]: SqliteGraph::clear_embeddings
    pub fn save_embeddings_bulk(
        &self,
        items: &[(NodeId, Vec<f32>)],
        model_name: &str,
    ) -> Result<(), DbError> {
        // Mixed-index guard.
        if let Some(stored) = self.get_meta("embedding_model")? {
            if stored != model_name {
                return Err(DbError::Other(format!(
                    "model mismatch: index built with '{stored}', \
                     cannot bulk-save from '{model_name}'; call clear_embeddings() first"
                )));
            }
        }

        let db = self.db();

        // Single transaction — one fsync for the whole batch.
        db.execute_batch("BEGIN")?;
        for (node_id, vector) in items {
            let blob = encode_vector(vector);
            db.execute(
                "INSERT OR REPLACE INTO embeddings (node_id, vector, model_name) \
                 VALUES (?, ?, ?)",
                params![node_id.to_string(), blob, model_name],
            )?;
        }
        // Write model key once at the end.
        db.execute(
            "INSERT OR REPLACE INTO project_meta (key, value) VALUES (?, ?)",
            params!["embedding_model", model_name],
        )?;
        db.execute_batch("COMMIT")?;
        Ok(())
    }

    /// Load the embedding vector for a single node. Returns `None` if absent.
    pub fn load_embedding(&self, node_id: &NodeId) -> Result<Option<Vec<f32>>, DbError> {
        let db = self.db();
        match db.query_row(
            "SELECT vector FROM embeddings WHERE node_id = ?",
            params![node_id.to_string()],
            |row| row.get::<_, Vec<u8>>(0),
        ) {
            Ok(blob) => Ok(Some(decode_vector(&blob))),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DbError::from(e)),
        }
    }

    /// Bulk-load all persisted embedding vectors.
    ///
    /// The returned map can be passed directly to
    /// `ail_search::EmbeddingIndex::from_vectors` to reconstruct the in-memory
    /// search index without recomputing embeddings.
    pub fn load_all_embeddings(&self) -> Result<HashMap<NodeId, Vec<f32>>, DbError> {
        let db = self.db();
        let mut stmt = db
            .prepare("SELECT node_id, vector FROM embeddings")
            .map_err(DbError::from)?;

        let pairs = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
            })
            .map_err(DbError::from)?;

        let mut map = HashMap::new();
        for res in pairs {
            let (id_str, blob) = res.map_err(DbError::from)?;
            let node_id = node_id_from_sql(&id_str)?;
            map.insert(node_id, decode_vector(&blob));
        }
        Ok(map)
    }

    /// Wipe all embedding vectors and the `embedding_model` meta key in a
    /// single transaction.
    ///
    /// After this call [`check_embedding_model`] returns [`EmbeddingModelStatus::Empty`]
    /// and [`save_embeddings_bulk`] accepts any model name.
    ///
    /// [`check_embedding_model`]: SqliteGraph::check_embedding_model
    /// [`save_embeddings_bulk`]: SqliteGraph::save_embeddings_bulk
    pub fn clear_embeddings(&self) -> Result<(), DbError> {
        let db = self.db();
        db.execute_batch("BEGIN")?;
        db.execute("DELETE FROM embeddings", [])?;
        db.execute("DELETE FROM project_meta WHERE key = 'embedding_model'", [])?;
        db.execute_batch("COMMIT")?;
        Ok(())
    }
}
