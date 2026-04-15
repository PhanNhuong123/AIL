/// SqliteGraph integration tests (tasks 7.2 and 7.3).
///
/// All tests use `tempfile::NamedTempFile` so that WAL mode is exercised on a
/// real file. In-memory SQLite does not support WAL.
use ail_db::SqliteGraph;
use ail_graph::{
    Contract, ContractKind, EdgeKind, Expression, GraphBackend, Node, NodeId, Pattern,
};
use tempfile::NamedTempFile;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn tmp_path() -> (NamedTempFile, std::path::PathBuf) {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_path_buf();
    // NamedTempFile keeps the path alive; we close the file handle but keep the
    // guard to prevent deletion until end of test.
    (file, path)
}

fn fresh_db() -> (NamedTempFile, SqliteGraph) {
    let (guard, path) = tmp_path();
    // NamedTempFile pre-creates the file; open_or_create handles the pre-existing empty file.
    let db = SqliteGraph::open_or_create(&path).unwrap();
    (guard, db)
}

fn make_do(intent: &str) -> Node {
    Node::new(NodeId::new(), intent, Pattern::Do)
}

fn make_named_do(intent: &str, name: &str) -> Node {
    let mut n = make_do(intent);
    n.metadata.name = Some(name.to_string());
    n
}

fn make_node(intent: &str, pattern: Pattern) -> Node {
    Node::new(NodeId::new(), intent, pattern)
}

// ─── t072_sqlite_create_new_database ─────────────────────────────────────────

#[test]
fn t072_sqlite_create_new_database() {
    let (_guard, path) = tmp_path();
    // The temp file exists from NamedTempFile; create() would fail if it exists.
    // Use open_or_create instead to handle the temp-file pre-existence.
    let db = SqliteGraph::open_or_create(&path).unwrap();
    assert!(path.exists(), "database file must exist after create");
    assert_eq!(db.node_count(), 0);
}

// ─── t072_sqlite_open_existing_database ──────────────────────────────────────

#[test]
fn t072_sqlite_open_existing_database() {
    let (_guard, path) = tmp_path();
    let node_id = {
        let mut db = SqliteGraph::open_or_create(&path).unwrap();
        let id = GraphBackend::add_node(&mut db, make_do("persist me")).unwrap();
        id
    };
    // Re-open the same file and verify the node is still there.
    let db2 = SqliteGraph::open(&path).unwrap();
    let fetched = GraphBackend::get_node(&db2, node_id).unwrap();
    assert!(fetched.is_some(), "node must survive close+reopen");
    assert_eq!(fetched.unwrap().intent, "persist me");
}

// ─── t072_sqlite_add_node_persists ───────────────────────────────────────────

#[test]
fn t072_sqlite_add_node_persists() {
    let (_guard, mut db) = fresh_db();
    let node = make_do("transfer money safely");
    let id = node.id;

    let returned_id = GraphBackend::add_node(&mut db, node).unwrap();
    assert_eq!(returned_id, id);

    let fetched = GraphBackend::get_node(&db, id).unwrap();
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().intent, "transfer money safely");

    // Unknown id returns None, not an error.
    assert!(GraphBackend::get_node(&db, NodeId::new()).unwrap().is_none());
}

// ─── t072_sqlite_get_node_returns_all_fields ─────────────────────────────────

#[test]
fn t072_sqlite_get_node_returns_all_fields() {
    let (_guard, mut db) = fresh_db();

    let mut node = make_named_do("validate wallet", "validate_wallet");
    node.expression = Some(Expression("amount > 0".to_string()));
    node.contracts.push(Contract {
        kind: ContractKind::Before,
        expression: Expression("balance >= 0".to_string()),
    });
    let id = node.id;
    let original_pattern = node.pattern.clone();

    GraphBackend::add_node(&mut db, node).unwrap();

    let fetched = GraphBackend::get_node(&db, id).unwrap().unwrap();
    assert_eq!(fetched.id, id);
    assert_eq!(fetched.intent, "validate wallet");
    assert_eq!(fetched.pattern, original_pattern);
    assert_eq!(fetched.metadata.name, Some("validate_wallet".to_string()));
    assert_eq!(
        fetched.expression,
        Some(Expression("amount > 0".to_string()))
    );
    assert_eq!(fetched.contracts.len(), 1);
    assert_eq!(fetched.contracts[0].kind, ContractKind::Before);
}

// ─── t072_sqlite_update_node_modifies_fields ─────────────────────────────────

#[test]
fn t072_sqlite_update_node_modifies_fields() {
    let (_guard, mut db) = fresh_db();
    let id = GraphBackend::add_node(&mut db, make_do("original intent")).unwrap();

    let mut updated = make_do("updated intent");
    updated.id = id; // preserve the same id
    GraphBackend::update_node(&mut db, id, updated).unwrap();

    let fetched = GraphBackend::get_node(&db, id).unwrap().unwrap();
    assert_eq!(fetched.intent, "updated intent");
    assert_eq!(fetched.id, id, "id must not change after update");
}

// ─── t072_sqlite_remove_node_cascades_children ───────────────────────────────

#[test]
fn t072_sqlite_remove_node_cascades_children() {
    let (_guard, mut db) = fresh_db();
    let parent_id = GraphBackend::add_node(&mut db, make_do("parent")).unwrap();
    let child_id = GraphBackend::add_node(&mut db, make_do("child")).unwrap();
    GraphBackend::add_edge(&mut db, parent_id, child_id, EdgeKind::Ev).unwrap();

    // Remove parent → child should also be gone via CASCADE.
    GraphBackend::remove_node(&mut db, parent_id).unwrap();

    assert_eq!(GraphBackend::node_count(&db), 0);
    assert!(GraphBackend::get_node(&db, child_id).unwrap().is_none());
}

// ─── t072_sqlite_remove_node_cascades_contracts ──────────────────────────────

#[test]
fn t072_sqlite_remove_node_cascades_contracts() {
    let (_guard, mut db) = fresh_db();
    let id = GraphBackend::add_node(&mut db, make_do("node with contract")).unwrap();
    GraphBackend::add_contract(
        &mut db,
        id,
        Contract {
            kind: ContractKind::Before,
            expression: Expression("x > 0".to_string()),
        },
    )
    .unwrap();

    GraphBackend::remove_node(&mut db, id).unwrap();

    // Contracts table must be empty — no orphaned rows.
    let count = db.table_row_count("contracts").unwrap();
    assert_eq!(count, 0, "contract rows must be cascaded on node delete");
}

// ─── t072_sqlite_remove_node_cascades_edges ──────────────────────────────────

#[test]
fn t072_sqlite_remove_node_cascades_edges() {
    let (_guard, mut db) = fresh_db();
    let func_id = GraphBackend::add_node(&mut db, make_do("transfer money")).unwrap();
    let ty_id =
        GraphBackend::add_node(&mut db, make_node("wallet balance", Pattern::Define)).unwrap();
    GraphBackend::add_edge(&mut db, func_id, ty_id, EdgeKind::Ed).unwrap();

    GraphBackend::remove_node(&mut db, func_id).unwrap();

    // edges table must be empty.
    let count = db.table_row_count("edges").unwrap();
    assert_eq!(count, 0, "edge rows must be cascaded on node delete");
}

// ─── t072_sqlite_children_returns_position_ordered ───────────────────────────

#[test]
fn t072_sqlite_children_returns_position_ordered() {
    let (_guard, mut db) = fresh_db();
    let root_id = GraphBackend::add_node(&mut db, make_do("root")).unwrap();
    let c1 = GraphBackend::add_node(&mut db, make_do("step-1")).unwrap();
    let c2 = GraphBackend::add_node(&mut db, make_do("step-2")).unwrap();
    let c3 = GraphBackend::add_node(&mut db, make_do("step-3")).unwrap();

    // Add in order: positions will be 0, 1, 2 (max_child_pos + 1 each time).
    GraphBackend::add_edge(&mut db, root_id, c1, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(&mut db, root_id, c2, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(&mut db, root_id, c3, EdgeKind::Ev).unwrap();

    let children = GraphBackend::children(&db, root_id).unwrap();
    assert_eq!(children, vec![c1, c2, c3], "children must be in insertion order");
}

// ─── t072_sqlite_siblings_before_correct_order ───────────────────────────────

#[test]
fn t072_sqlite_siblings_before_correct_order() {
    let (_guard, mut db) = fresh_db();
    let s1 = GraphBackend::add_node(&mut db, make_do("step-1")).unwrap();
    let s2 = GraphBackend::add_node(&mut db, make_do("step-2")).unwrap();
    let s3 = GraphBackend::add_node(&mut db, make_do("step-3")).unwrap();

    GraphBackend::add_edge(&mut db, s1, s2, EdgeKind::Eh).unwrap();
    GraphBackend::add_edge(&mut db, s2, s3, EdgeKind::Eh).unwrap();

    // From s3: [s1, s2] before
    let before = GraphBackend::siblings_before(&db, s3).unwrap();
    assert_eq!(before, vec![s1, s2], "siblings before s3 must be [s1, s2] in order");

    // From s1: nothing before
    assert!(GraphBackend::siblings_before(&db, s1).unwrap().is_empty());
}

// ─── t072_sqlite_siblings_after_correct_order ────────────────────────────────

#[test]
fn t072_sqlite_siblings_after_correct_order() {
    let (_guard, mut db) = fresh_db();
    let s1 = GraphBackend::add_node(&mut db, make_do("step-1")).unwrap();
    let s2 = GraphBackend::add_node(&mut db, make_do("step-2")).unwrap();
    let s3 = GraphBackend::add_node(&mut db, make_do("step-3")).unwrap();

    GraphBackend::add_edge(&mut db, s1, s2, EdgeKind::Eh).unwrap();
    GraphBackend::add_edge(&mut db, s2, s3, EdgeKind::Eh).unwrap();

    // From s1: [s2, s3] after
    let after = GraphBackend::siblings_after(&db, s1).unwrap();
    assert_eq!(after, vec![s2, s3]);

    // From s3: nothing after
    assert!(GraphBackend::siblings_after(&db, s3).unwrap().is_empty());
}

// ─── t072_sqlite_parent_returns_correct_parent ───────────────────────────────

#[test]
fn t072_sqlite_parent_returns_correct_parent() {
    let (_guard, mut db) = fresh_db();
    let parent_id = GraphBackend::add_node(&mut db, make_do("parent")).unwrap();
    let child_id = GraphBackend::add_node(&mut db, make_do("child")).unwrap();
    GraphBackend::add_edge(&mut db, parent_id, child_id, EdgeKind::Ev).unwrap();

    assert_eq!(GraphBackend::parent(&db, parent_id).unwrap(), None);
    assert_eq!(
        GraphBackend::parent(&db, child_id).unwrap(),
        Some(parent_id)
    );
}

// ─── t072_sqlite_ancestors_walks_to_root ─────────────────────────────────────

#[test]
fn t072_sqlite_ancestors_walks_to_root() {
    let (_guard, mut db) = fresh_db();
    let root = GraphBackend::add_node(&mut db, make_do("root")).unwrap();
    let mid = GraphBackend::add_node(&mut db, make_do("mid")).unwrap();
    let leaf = GraphBackend::add_node(&mut db, make_do("leaf")).unwrap();

    GraphBackend::add_edge(&mut db, root, mid, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(&mut db, mid, leaf, EdgeKind::Ev).unwrap();

    let ancestors = GraphBackend::ancestors(&db, leaf).unwrap();
    assert_eq!(ancestors, vec![mid, root], "direct parent first, grandparent second");

    assert_eq!(GraphBackend::ancestors(&db, mid).unwrap(), vec![root]);
    assert!(GraphBackend::ancestors(&db, root).unwrap().is_empty());
}

// ─── t072_sqlite_diagonal_refs_returns_ed_edges ──────────────────────────────

#[test]
fn t072_sqlite_diagonal_refs_returns_ed_edges() {
    let (_guard, mut db) = fresh_db();
    let func_id = GraphBackend::add_node(&mut db, make_do("transfer money")).unwrap();
    let ty_id =
        GraphBackend::add_node(&mut db, make_node("wallet balance", Pattern::Define)).unwrap();

    GraphBackend::add_edge(&mut db, func_id, ty_id, EdgeKind::Ed).unwrap();

    // Outgoing side
    let refs = GraphBackend::diagonal_refs(&db, func_id).unwrap();
    assert!(refs.contains(&(ty_id, EdgeKind::Ed)));

    // Incoming side (back-reference)
    let back = GraphBackend::diagonal_refs(&db, ty_id).unwrap();
    assert!(back.contains(&(func_id, EdgeKind::Ed)));
}

// ─── t072_sqlite_find_by_pattern_do_nodes ────────────────────────────────────

#[test]
fn t072_sqlite_find_by_pattern_do_nodes() {
    let (_guard, mut db) = fresh_db();
    let do1 = GraphBackend::add_node(&mut db, make_do("function one")).unwrap();
    let do2 = GraphBackend::add_node(&mut db, make_do("function two")).unwrap();
    let _def = GraphBackend::add_node(&mut db, make_node("a type", Pattern::Define)).unwrap();

    let mut found = GraphBackend::find_by_pattern(&db, Pattern::Do).unwrap();
    found.sort_by_key(|id| id.to_string());
    let mut expected = vec![do1, do2];
    expected.sort_by_key(|id| id.to_string());

    assert_eq!(found, expected);
}

// ─── t072_sqlite_find_by_pattern_define_nodes ────────────────────────────────

#[test]
fn t072_sqlite_find_by_pattern_define_nodes() {
    let (_guard, mut db) = fresh_db();
    let _do1 = GraphBackend::add_node(&mut db, make_do("a function")).unwrap();
    let def1 = GraphBackend::add_node(&mut db, make_node("wallet balance", Pattern::Define)).unwrap();
    let def2 = GraphBackend::add_node(&mut db, make_node("positive amount", Pattern::Define)).unwrap();

    let mut found = GraphBackend::find_by_pattern(&db, Pattern::Define).unwrap();
    found.sort_by_key(|id| id.to_string());
    let mut expected = vec![def1, def2];
    expected.sort_by_key(|id| id.to_string());
    assert_eq!(found, expected);

    assert!(GraphBackend::find_by_pattern(&db, Pattern::Describe).unwrap().is_empty());
}

// ─── t072_sqlite_find_by_name_exact_match ────────────────────────────────────

#[test]
fn t072_sqlite_find_by_name_exact_match() {
    let (_guard, mut db) = fresh_db();
    let id1 = GraphBackend::add_node(&mut db, make_named_do("transfer money", "transfer_money"))
        .unwrap();
    let _id2 = GraphBackend::add_node(&mut db, make_do("validate input")).unwrap();
    let id3 = GraphBackend::add_node(&mut db, make_named_do("transfer again", "transfer_money"))
        .unwrap();

    let mut found = GraphBackend::find_by_name(&db, "transfer_money").unwrap();
    found.sort_by_key(|id| id.to_string());
    let mut expected = vec![id1, id3];
    expected.sort_by_key(|id| id.to_string());
    assert_eq!(found, expected);
}

// ─── t072_sqlite_find_by_name_no_match ───────────────────────────────────────

#[test]
fn t072_sqlite_find_by_name_no_match() {
    let (_guard, mut db) = fresh_db();
    GraphBackend::add_node(&mut db, make_do("some function")).unwrap();

    let found = GraphBackend::find_by_name(&db, "nonexistent_name").unwrap();
    assert!(found.is_empty());
}

// ─── t072_sqlite_root_nodes_returns_depth_zero_only ──────────────────────────

#[test]
fn t072_sqlite_root_nodes_returns_depth_zero_only() {
    let (_guard, mut db) = fresh_db();
    let root1 = GraphBackend::add_node(&mut db, make_do("root-1")).unwrap();
    let root2 = GraphBackend::add_node(&mut db, make_do("root-2")).unwrap();
    let child = GraphBackend::add_node(&mut db, make_do("child")).unwrap();
    GraphBackend::add_edge(&mut db, root1, child, EdgeKind::Ev).unwrap();

    let mut roots = GraphBackend::root_nodes(&db).unwrap();
    roots.sort_by_key(|id| id.to_string());
    let mut expected = vec![root1, root2];
    expected.sort_by_key(|id| id.to_string());

    assert_eq!(roots, expected);
    assert!(!roots.contains(&child));
}

// ─── t072_sqlite_depth_consistent_with_parent_chain ─────────────────────────

#[test]
fn t072_sqlite_depth_consistent_with_parent_chain() {
    let (_guard, mut db) = fresh_db();
    let root = GraphBackend::add_node(&mut db, make_do("root")).unwrap();
    let level1 = GraphBackend::add_node(&mut db, make_do("level-1")).unwrap();
    let level2 = GraphBackend::add_node(&mut db, make_do("level-2")).unwrap();

    GraphBackend::add_edge(&mut db, root, level1, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(&mut db, level1, level2, EdgeKind::Ev).unwrap();

    assert_eq!(GraphBackend::depth(&db, root).unwrap(), 0);
    assert_eq!(GraphBackend::depth(&db, level1).unwrap(), 1);
    assert_eq!(GraphBackend::depth(&db, level2).unwrap(), 2);
}

// ─── t072_sqlite_add_contract_persists ───────────────────────────────────────

#[test]
fn t072_sqlite_add_contract_persists() {
    let (_guard, mut db) = fresh_db();
    let id = GraphBackend::add_node(&mut db, make_do("validate wallet")).unwrap();

    GraphBackend::add_contract(
        &mut db,
        id,
        Contract {
            kind: ContractKind::Before,
            expression: Expression("balance >= 0".to_string()),
        },
    )
    .unwrap();

    let contracts = GraphBackend::contracts(&db, id).unwrap();
    assert_eq!(contracts.len(), 1);
    assert_eq!(contracts[0].kind, ContractKind::Before);
    assert_eq!(contracts[0].expression.0, "balance >= 0");
}

// ─── t072_sqlite_contracts_returns_all_for_node ──────────────────────────────

#[test]
fn t072_sqlite_contracts_returns_all_for_node() {
    let (_guard, mut db) = fresh_db();
    let id = GraphBackend::add_node(&mut db, make_do("transfer money")).unwrap();

    let contracts_to_add = vec![
        Contract {
            kind: ContractKind::Before,
            expression: Expression("amount > 0".to_string()),
        },
        Contract {
            kind: ContractKind::After,
            expression: Expression("sender.balance >= 0".to_string()),
        },
        Contract {
            kind: ContractKind::Always,
            expression: Expression("amount <= 10000".to_string()),
        },
    ];
    for c in &contracts_to_add {
        GraphBackend::add_contract(&mut db, id, c.clone()).unwrap();
    }

    let contracts = GraphBackend::contracts(&db, id).unwrap();
    assert_eq!(contracts.len(), 3);
    // Verify each kind is present.
    assert!(contracts.iter().any(|c| c.kind == ContractKind::Before));
    assert!(contracts.iter().any(|c| c.kind == ContractKind::After));
    assert!(contracts.iter().any(|c| c.kind == ContractKind::Always));
}

// ─── t072_sqlite_transaction_commits ─────────────────────────────────────────

#[test]
fn t072_sqlite_transaction_commits() {
    let (_guard, mut db) = fresh_db();

    GraphBackend::begin_transaction(&mut db).unwrap();
    let id = GraphBackend::add_node(&mut db, make_do("inside transaction")).unwrap();
    GraphBackend::commit_transaction(&mut db).unwrap();

    let fetched = GraphBackend::get_node(&db, id).unwrap();
    assert!(
        fetched.is_some(),
        "node inserted during transaction must be visible after commit"
    );
}

// ─── t072_sqlite_transaction_rollback_on_error ───────────────────────────────

#[test]
fn t072_sqlite_transaction_rollback_on_error() {
    let (_guard, mut db) = fresh_db();

    GraphBackend::begin_transaction(&mut db).unwrap();
    let id = GraphBackend::add_node(&mut db, make_do("tentative node")).unwrap();
    GraphBackend::rollback_transaction(&mut db).unwrap();

    // After rollback, the node must NOT exist (unlike AilGraph which is a no-op).
    let fetched = GraphBackend::get_node(&db, id).unwrap();
    assert!(
        fetched.is_none(),
        "node must be gone after SQLite rollback"
    );
    assert_eq!(GraphBackend::node_count(&db), 0);
}

// ─── t072_sqlite_wal_mode_enabled ────────────────────────────────────────────

#[test]
fn t072_sqlite_wal_mode_enabled() {
    let (_guard, db) = fresh_db();
    let mode = db.journal_mode().unwrap();
    assert_eq!(mode, "wal", "WAL mode must be active on every SqliteGraph connection");
}

// ─── t072_sqlite_all_pattern_variants_roundtrip ──────────────────────────────

#[test]
fn t072_sqlite_all_pattern_variants_roundtrip() {
    let (_guard, mut db) = fresh_db();

    let all_patterns = vec![
        Pattern::Define,
        Pattern::Describe,
        Pattern::Error,
        Pattern::Do,
        Pattern::Promise,
        Pattern::Let,
        Pattern::Check,
        Pattern::ForEach,
        Pattern::Match,
        Pattern::Fetch,
        Pattern::Save,
        Pattern::Update,
        Pattern::Remove,
        Pattern::Return,
        Pattern::Raise,
        Pattern::Together,
        Pattern::Retry,
    ];
    assert_eq!(all_patterns.len(), 17, "must test all 17 Pattern variants");

    let mut ids = Vec::new();
    for pattern in &all_patterns {
        let node = Node::new(NodeId::new(), format!("intent for {pattern:?}"), pattern.clone());
        let id = GraphBackend::add_node(&mut db, node).unwrap();
        ids.push((id, pattern.clone()));
    }

    for (id, expected_pattern) in ids {
        let fetched = GraphBackend::get_node(&db, id).unwrap().unwrap();
        assert_eq!(
            fetched.pattern, expected_pattern,
            "Pattern::{expected_pattern:?} must roundtrip through SQLite"
        );
    }
}

// ─── t072_sqlite_concurrent_read ─────────────────────────────────────────────

#[test]
fn t072_sqlite_concurrent_read() {
    let (_guard, path) = tmp_path();

    // Populate via one connection.
    {
        let mut db1 = SqliteGraph::open_or_create(&path).unwrap();
        GraphBackend::add_node(&mut db1, make_do("shared node")).unwrap();
    }

    // Open two read-only connections simultaneously.
    let db_a = SqliteGraph::open(&path).unwrap();
    let db_b = SqliteGraph::open(&path).unwrap();

    // Both must see the same data without error.
    let count_a = GraphBackend::node_count(&db_a);
    let count_b = GraphBackend::node_count(&db_b);
    assert_eq!(count_a, 1);
    assert_eq!(count_b, 1);

    let roots_a = GraphBackend::root_nodes(&db_a).unwrap();
    let roots_b = GraphBackend::root_nodes(&db_b).unwrap();
    assert_eq!(roots_a, roots_b, "both readers must see identical root list");
}

// ─── all_descendants ─────────────────────────────────────────────────────────

#[test]
fn t072_sqlite_all_descendants_returns_subtree() {
    let (_guard, mut db) = fresh_db();
    let root = GraphBackend::add_node(&mut db, make_do("root")).unwrap();
    let c1 = GraphBackend::add_node(&mut db, make_do("child-1")).unwrap();
    let c2 = GraphBackend::add_node(&mut db, make_do("child-2")).unwrap();
    let gc = GraphBackend::add_node(&mut db, make_do("grandchild")).unwrap();

    GraphBackend::add_edge(&mut db, root, c1, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(&mut db, root, c2, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(&mut db, c1, gc, EdgeKind::Ev).unwrap();

    let mut desc = GraphBackend::all_descendants(&db, root).unwrap();
    desc.sort_by_key(|id| id.to_string());
    let mut expected = vec![c1, c2, gc];
    expected.sort_by_key(|id| id.to_string());
    assert_eq!(desc, expected);

    assert!(GraphBackend::all_descendants(&db, gc).unwrap().is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// Task 7.3 — CIC Cache + Incremental Invalidation
// ═══════════════════════════════════════════════════════════════════════════

// ─── t073_empty_cache_on_new_database ────────────────────────────────────────

#[test]
fn t073_empty_cache_on_new_database() {
    let (_guard, db) = fresh_db();
    let count = db.table_row_count("cic_cache").unwrap();
    assert_eq!(count, 0, "cic_cache must be empty on a new database");
}

// ─── t073_cache_miss_computes_and_stores ─────────────────────────────────────

#[test]
fn t073_cache_miss_computes_and_stores() {
    let (_guard, mut db) = fresh_db();
    let id = GraphBackend::add_node(&mut db, make_do("validate input")).unwrap();

    // No cache entry yet.
    assert_eq!(db.cic_cache_valid(id), None, "no cache entry before first access");

    // First call: cache MISS → compute and store.
    let packet = db.get_context_packet(id).unwrap();
    assert_eq!(packet.node_id, id);
    assert!(!packet.intent_chain.is_empty(), "intent_chain must include the node itself");

    // Cache entry now exists and is valid.
    assert_eq!(db.cic_cache_valid(id), Some(true), "cache must be valid after first access");
    assert_eq!(db.table_row_count("cic_cache").unwrap(), 1);
}

// ─── t073_cache_hit_returns_stored_packet ────────────────────────────────────

#[test]
fn t073_cache_hit_returns_stored_packet() {
    let (_guard, mut db) = fresh_db();
    let id = GraphBackend::add_node(&mut db, make_do("transfer funds")).unwrap();

    let first = db.get_context_packet(id).unwrap();
    let second = db.get_context_packet(id).unwrap();

    // Both calls must return the same packet (second is a cache HIT).
    assert_eq!(first, second, "cache hit must return identical packet");

    // Only one cache row — no duplicate entries.
    assert_eq!(db.table_row_count("cic_cache").unwrap(), 1);
}

// ─── t073_invalidate_on_node_change ──────────────────────────────────────────

#[test]
fn t073_invalidate_on_node_change() {
    let (_guard, mut db) = fresh_db();
    let id = GraphBackend::add_node(&mut db, make_do("original intent")).unwrap();

    // Warm the cache.
    db.get_context_packet(id).unwrap();
    assert_eq!(db.cic_cache_valid(id), Some(true));

    // update_node must invalidate the cache entry.
    let mut updated = make_do("updated intent");
    updated.id = id;
    GraphBackend::update_node(&mut db, id, updated).unwrap();

    assert_eq!(
        db.cic_cache_valid(id),
        Some(false),
        "cache must be stale after update_node"
    );
}

// ─── t073_invalidate_cascades_to_descendants ─────────────────────────────────

#[test]
fn t073_invalidate_cascades_to_descendants() {
    let (_guard, mut db) = fresh_db();
    let root = GraphBackend::add_node(&mut db, make_do("root")).unwrap();
    let child = GraphBackend::add_node(&mut db, make_do("child")).unwrap();
    let grandchild = GraphBackend::add_node(&mut db, make_do("grandchild")).unwrap();
    GraphBackend::add_edge(&mut db, root, child, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(&mut db, child, grandchild, EdgeKind::Ev).unwrap();

    // Warm cache for all three nodes.
    db.get_context_packet(root).unwrap();
    db.get_context_packet(child).unwrap();
    db.get_context_packet(grandchild).unwrap();

    // Updating root must cascade to child and grandchild via Rule 1 DOWN.
    let mut updated = make_do("root updated");
    updated.id = root;
    GraphBackend::update_node(&mut db, root, updated).unwrap();

    assert_eq!(db.cic_cache_valid(child), Some(false), "child must be stale");
    assert_eq!(db.cic_cache_valid(grandchild), Some(false), "grandchild must be stale");
}

// ─── t073_invalidate_cascades_to_ancestors ───────────────────────────────────

#[test]
fn t073_invalidate_cascades_to_ancestors() {
    let (_guard, mut db) = fresh_db();
    let root = GraphBackend::add_node(&mut db, make_do("root")).unwrap();
    let child = GraphBackend::add_node(&mut db, make_do("child")).unwrap();
    let grandchild = GraphBackend::add_node(&mut db, make_do("grandchild")).unwrap();
    GraphBackend::add_edge(&mut db, root, child, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(&mut db, child, grandchild, EdgeKind::Ev).unwrap();

    // Warm cache for all three nodes.
    db.get_context_packet(root).unwrap();
    db.get_context_packet(child).unwrap();
    db.get_context_packet(grandchild).unwrap();

    // Updating grandchild must cascade to child and root via Rule 2 UP.
    let mut updated = make_do("grandchild updated");
    updated.id = grandchild;
    GraphBackend::update_node(&mut db, grandchild, updated).unwrap();

    assert_eq!(db.cic_cache_valid(child), Some(false), "child must be stale");
    assert_eq!(db.cic_cache_valid(root), Some(false), "root must be stale");
}

// ─── t073_invalidate_cascades_to_next_siblings ───────────────────────────────

#[test]
fn t073_invalidate_cascades_to_next_siblings() {
    let (_guard, mut db) = fresh_db();
    let parent = GraphBackend::add_node(&mut db, make_do("parent")).unwrap();
    let s1 = GraphBackend::add_node(&mut db, make_do("step-1")).unwrap();
    let s2 = GraphBackend::add_node(&mut db, make_do("step-2")).unwrap();
    let s3 = GraphBackend::add_node(&mut db, make_do("step-3")).unwrap();
    GraphBackend::add_edge(&mut db, parent, s1, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(&mut db, parent, s2, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(&mut db, parent, s3, EdgeKind::Ev).unwrap();

    // Warm cache for siblings.
    db.get_context_packet(s1).unwrap();
    db.get_context_packet(s2).unwrap();
    db.get_context_packet(s3).unwrap();

    // Updating s1 must cascade to s2 and s3 via Rule 3 ACROSS.
    let mut updated = make_do("step-1 updated");
    updated.id = s1;
    GraphBackend::update_node(&mut db, s1, updated).unwrap();

    assert_eq!(db.cic_cache_valid(s2), Some(false), "s2 must be stale");
    assert_eq!(db.cic_cache_valid(s3), Some(false), "s3 must be stale");
}

// ─── t073_invalidate_diagonal_type_change ────────────────────────────────────

#[test]
fn t073_invalidate_diagonal_type_change() {
    let (_guard, mut db) = fresh_db();
    let type_id = GraphBackend::add_node(
        &mut db,
        make_node("wallet balance", Pattern::Define),
    )
    .unwrap();
    let func_id = GraphBackend::add_node(&mut db, make_do("transfer money")).unwrap();

    // func references type via Ed edge.
    GraphBackend::add_edge(&mut db, func_id, type_id, EdgeKind::Ed).unwrap();

    // Warm cache for func.
    db.get_context_packet(func_id).unwrap();
    assert_eq!(db.cic_cache_valid(func_id), Some(true));

    // Update the type — Rule 4 DIAGONAL: func references type → func goes stale.
    let mut updated = make_node("wallet balance updated", Pattern::Define);
    updated.id = type_id;
    GraphBackend::update_node(&mut db, type_id, updated).unwrap();

    assert_eq!(
        db.cic_cache_valid(func_id),
        Some(false),
        "func cache must be stale after referenced type changes"
    );
}

// ─── t073_invalidate_diagonal_function_change ────────────────────────────────

#[test]
fn t073_invalidate_diagonal_function_change() {
    let (_guard, mut db) = fresh_db();
    let callee = GraphBackend::add_node(&mut db, make_do("validate wallet")).unwrap();
    let caller = GraphBackend::add_node(&mut db, make_do("transfer money")).unwrap();

    // caller calls callee via Ed edge.
    GraphBackend::add_edge(&mut db, caller, callee, EdgeKind::Ed).unwrap();

    // Warm cache for caller.
    db.get_context_packet(caller).unwrap();
    assert_eq!(db.cic_cache_valid(caller), Some(true));

    // Update callee — Rule 4 DIAGONAL: caller has incoming Ed to callee → caller stale.
    let mut updated = make_do("validate wallet updated");
    updated.id = callee;
    GraphBackend::update_node(&mut db, callee, updated).unwrap();

    assert_eq!(
        db.cic_cache_valid(caller),
        Some(false),
        "caller cache must be stale after called function changes"
    );
}

// ─── t073_recompute_after_invalidation ───────────────────────────────────────

#[test]
fn t073_recompute_after_invalidation() {
    let (_guard, mut db) = fresh_db();
    let id = GraphBackend::add_node(&mut db, make_do("compute me")).unwrap();

    // First compute.
    let first = db.get_context_packet(id).unwrap();
    assert_eq!(db.cic_cache_valid(id), Some(true));

    // Manually invalidate.
    db.invalidate_node(id).unwrap();
    assert_eq!(db.cic_cache_valid(id), Some(false));

    // Second get: cache miss → recompute → valid again.
    let second = db.get_context_packet(id).unwrap();
    assert_eq!(db.cic_cache_valid(id), Some(true));
    assert_eq!(
        first.node_id, second.node_id,
        "recomputed packet must have the same node_id"
    );
}

// ─── t073_bulk_invalidation_efficiency ───────────────────────────────────────

#[test]
fn t073_bulk_invalidation_efficiency() {
    let (_guard, mut db) = fresh_db();

    // Build a linear chain: root → n1 → n2 → … → n9
    let root = GraphBackend::add_node(&mut db, make_do("root")).unwrap();
    let mut prev = root;
    let mut ids = vec![root];
    for i in 1..=9 {
        let node = GraphBackend::add_node(&mut db, make_do(&format!("step-{i}"))).unwrap();
        GraphBackend::add_edge(&mut db, prev, node, EdgeKind::Ev).unwrap();
        ids.push(node);
        prev = node;
    }

    // Warm cache for all 10 nodes.
    for &id in &ids {
        db.get_context_packet(id).unwrap();
    }
    assert_eq!(db.table_row_count("cic_cache").unwrap(), 10);

    // Update the middle node (ids[5]). Rule 1 DOWN (4 descendants) and
    // Rule 2 UP (5 ancestors) must all be marked stale.
    let mid = ids[5];
    let mut updated = make_do("mid updated");
    updated.id = mid;
    GraphBackend::update_node(&mut db, mid, updated).unwrap();

    // Verify that root and the leaf are both stale.
    assert_eq!(db.cic_cache_valid(root), Some(false), "root must be stale");
    assert_eq!(db.cic_cache_valid(ids[9]), Some(false), "leaf must be stale");
    // Mid itself must be stale.
    assert_eq!(db.cic_cache_valid(mid), Some(false), "mid must be stale");
}

// ─── t073_add_node_invalidates_parent ────────────────────────────────────────

#[test]
fn t073_add_node_invalidates_parent() {
    let (_guard, mut db) = fresh_db();
    let parent = GraphBackend::add_node(&mut db, make_do("parent")).unwrap();

    // Warm parent's cache.
    db.get_context_packet(parent).unwrap();
    assert_eq!(db.cic_cache_valid(parent), Some(true));

    // Adding a child via add_edge must invalidate the parent (Rule 2 UP from child).
    let child = GraphBackend::add_node(&mut db, make_do("new child")).unwrap();
    GraphBackend::add_edge(&mut db, parent, child, EdgeKind::Ev).unwrap();

    assert_eq!(
        db.cic_cache_valid(parent),
        Some(false),
        "parent cache must be stale after new child added"
    );
}

// ─── t073_remove_node_invalidates_siblings ───────────────────────────────────

#[test]
fn t073_remove_node_invalidates_siblings() {
    let (_guard, mut db) = fresh_db();
    let parent = GraphBackend::add_node(&mut db, make_do("parent")).unwrap();
    let s1 = GraphBackend::add_node(&mut db, make_do("step-1")).unwrap();
    let s2 = GraphBackend::add_node(&mut db, make_do("step-2")).unwrap();
    let s3 = GraphBackend::add_node(&mut db, make_do("step-3")).unwrap();
    GraphBackend::add_edge(&mut db, parent, s1, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(&mut db, parent, s2, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(&mut db, parent, s3, EdgeKind::Ev).unwrap();

    // Warm cache for s2 and s3.
    db.get_context_packet(s2).unwrap();
    db.get_context_packet(s3).unwrap();
    assert_eq!(db.cic_cache_valid(s2), Some(true));
    assert_eq!(db.cic_cache_valid(s3), Some(true));

    // Removing s1 must invalidate its next siblings (s2, s3) via Rule 3 ACROSS.
    GraphBackend::remove_node(&mut db, s1).unwrap();

    assert_eq!(db.cic_cache_valid(s2), Some(false), "s2 must be stale after s1 removed");
    assert_eq!(db.cic_cache_valid(s3), Some(false), "s3 must be stale after s1 removed");
}

// ─── t073_move_node_invalidates_old_and_new_parent ───────────────────────────

#[test]
fn t073_move_node_invalidates_old_and_new_parent() {
    let (_guard, mut db) = fresh_db();
    let old_parent = GraphBackend::add_node(&mut db, make_do("old parent")).unwrap();
    let new_parent = GraphBackend::add_node(&mut db, make_do("new parent")).unwrap();
    let child = GraphBackend::add_node(&mut db, make_do("child")).unwrap();
    GraphBackend::add_edge(&mut db, old_parent, child, EdgeKind::Ev).unwrap();

    // Warm cache for both parents.
    db.get_context_packet(old_parent).unwrap();
    db.get_context_packet(new_parent).unwrap();

    // Detach child from old parent → invalidates old_parent (Rule 2 UP from child).
    GraphBackend::remove_edge_by_kind(&mut db, old_parent, child, EdgeKind::Ev).unwrap();
    assert_eq!(
        db.cic_cache_valid(old_parent),
        Some(false),
        "old parent must be stale after child detached"
    );

    // Re-warm new_parent and re-attach child.
    db.get_context_packet(new_parent).unwrap();
    GraphBackend::add_edge(&mut db, new_parent, child, EdgeKind::Ev).unwrap();
    assert_eq!(
        db.cic_cache_valid(new_parent),
        Some(false),
        "new parent must be stale after child attached"
    );
}

// ─── t073_update_contract_invalidates_subtree ────────────────────────────────

#[test]
fn t073_update_contract_invalidates_subtree() {
    let (_guard, mut db) = fresh_db();
    let parent = GraphBackend::add_node(&mut db, make_do("validate payment")).unwrap();
    let child = GraphBackend::add_node(&mut db, make_do("deduct balance")).unwrap();
    GraphBackend::add_edge(&mut db, parent, child, EdgeKind::Ev).unwrap();

    // Warm cache for child (its inherited_constraints come from parent's contracts).
    db.get_context_packet(child).unwrap();
    assert_eq!(db.cic_cache_valid(child), Some(true));

    // Adding a contract to parent must invalidate child (Rule 1 DOWN: child
    // inherits parent's contracts).
    GraphBackend::add_contract(
        &mut db,
        parent,
        Contract {
            kind: ContractKind::Before,
            expression: Expression("amount > 0".to_string()),
        },
    )
    .unwrap();

    assert_eq!(
        db.cic_cache_valid(child),
        Some(false),
        "child cache must be stale after parent contract added"
    );
}
