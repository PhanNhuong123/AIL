//! Coverage cache — persists and invalidates [`CoverageInfo`] rows in SQLite.
//!
//! All operations accept a raw `&Connection` so they can be called inside
//! an existing mutex-lock scope without risk of deadlock.
//!
//! Column mapping
//! - `score`                     — `Option<f32>` (NULL = leaf / Guard D)
//! - `status`                    — [`CoverageStatus`] label string
//! - `child_contributions`       — JSON array of [`ChildContributionInfo`]
//! - `missing_aspects`           — JSON array of [`MissingAspectInfo`]
//! - `empty_parent`              — bool stored as 0/1 integer
//! - `degenerate_basis_fallback` — bool stored as 0/1 integer
//! - `computed_at`               — unix epoch seconds (i64)
//! - `config_hash`               — 16-char hex string from [`CoverageConfig::config_hash`]
//! - `valid`                     — 1 = usable, 0 = stale

use ail_graph::cic::{ChildContributionInfo, CoverageInfo, CoverageStatus, MissingAspectInfo};
use rusqlite::Connection;

use crate::errors::DbError;

// ─── Save ─────────────────────────────────────────────────────────────────────

/// Upsert a coverage result for `node_id`, marking it valid.
///
/// An existing row (valid or stale) is atomically replaced so repeated calls
/// for the same node always reflect the freshest computation.
pub(crate) fn save_coverage(
    conn: &Connection,
    node_id: &str,
    info: &CoverageInfo,
) -> Result<(), DbError> {
    let contributions_json = serde_json::to_string(&info.child_contributions)?;
    let missing_json = serde_json::to_string(&info.missing_aspects)?;
    let status_label = info.status.label();
    let empty_parent = i64::from(info.empty_parent);
    let degenerate = i64::from(info.degenerate_basis_fallback);

    conn.execute(
        "INSERT OR REPLACE INTO coverage_cache \
         (node_id, score, status, child_contributions, missing_aspects, \
          empty_parent, degenerate_basis_fallback, computed_at, config_hash, valid) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1)",
        rusqlite::params![
            node_id,
            info.score,
            status_label,
            contributions_json,
            missing_json,
            empty_parent,
            degenerate,
            info.computed_at,
            info.config_hash,
        ],
    )?;
    Ok(())
}

// ─── Load single ──────────────────────────────────────────────────────────────

/// Return the valid cached [`CoverageInfo`] for `node_id` if the stored
/// `config_hash` matches `current_config_hash`.
///
/// Returns `Ok(None)` when:
/// - no row exists for `node_id`,
/// - the row is stale (`valid = 0`), or
/// - the stored `config_hash` differs from `current_config_hash`.
pub(crate) fn load_coverage(
    conn: &Connection,
    node_id: &str,
    current_config_hash: &str,
) -> Result<Option<CoverageInfo>, DbError> {
    match conn.query_row(
        "SELECT score, status, child_contributions, missing_aspects, \
                empty_parent, degenerate_basis_fallback, computed_at, config_hash \
         FROM coverage_cache \
         WHERE node_id = ?1 AND valid = 1 AND config_hash = ?2",
        rusqlite::params![node_id, current_config_hash],
        |row| {
            Ok((
                row.get::<_, Option<f64>>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, String>(7)?,
            ))
        },
    ) {
        Ok((
            score,
            status_label,
            contributions_json,
            missing_json,
            empty,
            degenerate,
            computed_at,
            config_hash,
        )) => {
            let score = score.map(|f| f as f32);
            let status = parse_coverage_status(&status_label)?;
            let child_contributions: Vec<ChildContributionInfo> =
                serde_json::from_str(&contributions_json)?;
            let missing_aspects: Vec<MissingAspectInfo> = serde_json::from_str(&missing_json)?;
            Ok(Some(CoverageInfo {
                score,
                status,
                child_contributions,
                missing_aspects,
                empty_parent: empty != 0,
                degenerate_basis_fallback: degenerate != 0,
                computed_at,
                config_hash,
            }))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(DbError::Sqlite(e)),
    }
}

// ─── Load all valid ───────────────────────────────────────────────────────────

/// Return all valid, config-matching [`CoverageInfo`] rows as `(node_id, info)` pairs.
///
/// Used by CLI `ail coverage --all` to display project-wide coverage state
/// without recomputing.
pub(crate) fn load_all_valid_coverage(
    conn: &Connection,
    current_config_hash: &str,
) -> Result<Vec<(String, CoverageInfo)>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT node_id, score, status, child_contributions, missing_aspects, \
                empty_parent, degenerate_basis_fallback, computed_at, config_hash \
         FROM coverage_cache \
         WHERE valid = 1 AND config_hash = ?1",
    )?;

    let rows = stmt
        .query_map(rusqlite::params![current_config_hash], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<f64>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, String>(8)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut result = Vec::with_capacity(rows.len());
    for (
        node_id,
        score,
        status_label,
        contributions_json,
        missing_json,
        empty,
        degenerate,
        computed_at,
        config_hash,
    ) in rows
    {
        let score = score.map(|f| f as f32);
        let status = parse_coverage_status(&status_label)?;
        let child_contributions: Vec<ChildContributionInfo> =
            serde_json::from_str(&contributions_json)?;
        let missing_aspects: Vec<MissingAspectInfo> = serde_json::from_str(&missing_json)?;
        result.push((
            node_id,
            CoverageInfo {
                score,
                status,
                child_contributions,
                missing_aspects,
                empty_parent: empty != 0,
                degenerate_basis_fallback: degenerate != 0,
                computed_at,
                config_hash,
            },
        ));
    }
    Ok(result)
}

// ─── Invalidation ─────────────────────────────────────────────────────────────

/// Mark coverage rows stale for a pre-computed list of node IDs.
///
/// Callers supply the ancestor list (including the mutated node itself).
/// Returns the number of rows updated. Returns `Ok(0)` immediately when the
/// list is empty (avoids emitting a SQL statement with zero placeholders).
pub(crate) fn invalidate_coverage_for_ancestors(
    conn: &Connection,
    node_ids: &[String],
) -> Result<usize, DbError> {
    if node_ids.is_empty() {
        return Ok(0);
    }
    let placeholders = node_ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let sql = format!("UPDATE coverage_cache SET valid = 0 WHERE node_id IN ({placeholders})");
    let count = conn.execute(&sql, rusqlite::params_from_iter(node_ids.iter()))?;
    Ok(count)
}

// ─── Clear ────────────────────────────────────────────────────────────────────

/// Delete all rows from `coverage_cache`.
pub(crate) fn clear_coverage(conn: &Connection) -> Result<(), DbError> {
    conn.execute("DELETE FROM coverage_cache", [])?;
    Ok(())
}

// ─── Count ────────────────────────────────────────────────────────────────────

/// Return the total number of rows in `coverage_cache` (valid + stale).
pub(crate) fn coverage_count(conn: &Connection) -> Result<usize, DbError> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM coverage_cache", [], |row| row.get(0))?;
    Ok(usize::try_from(count).unwrap_or(0))
}

// ─── Private helpers ──────────────────────────────────────────────────────────

/// Deserialize a stored status label back to [`CoverageStatus`].
///
/// Uses [`CoverageStatus`]'s `Deserialize` impl by wrapping the label in a
/// JSON string value and calling `serde_json::from_value`. This avoids a
/// manual match block and stays consistent with the canonical serde mapping.
fn parse_coverage_status(label: &str) -> Result<CoverageStatus, DbError> {
    let json_string = serde_json::Value::String(label.to_owned());
    serde_json::from_value(json_string).map_err(DbError::Serialization)
}
