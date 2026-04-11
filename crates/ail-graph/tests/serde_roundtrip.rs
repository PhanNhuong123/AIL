use ail_graph::{
    Contract, ContractKind, EdgeKind, Expression, Field, Node, NodeId, NodeMetadata, Param,
    Pattern,
};

fn roundtrip<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json = serde_json::to_string(value).expect("serialize failed");
    let restored: T = serde_json::from_str(&json).expect("deserialize failed");
    assert_eq!(*value, restored, "roundtrip mismatch\njson: {json}");
    restored
}

#[test]
fn roundtrip_node_leaf_with_expression() {
    let node = Node {
        id: NodeId::new(),
        intent: "deduct amount from sender balance".to_string(),
        pattern: Pattern::Let,
        children: None,
        expression: Some(Expression("new_balance: WalletBalance = sender.balance - amount".to_string())),
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    roundtrip(&node);
}

#[test]
fn roundtrip_node_structural_with_children() {
    let child_a = NodeId::new();
    let child_b = NodeId::new();
    let node = Node {
        id: NodeId::new(),
        intent: "transfer money between wallets".to_string(),
        pattern: Pattern::Do,
        children: Some(vec![child_a, child_b]),
        expression: None,
        contracts: vec![
            Contract {
                kind: ContractKind::Before,
                expression: Expression("sender.status is \"active\"".to_string()),
            },
            Contract {
                kind: ContractKind::After,
                expression: Expression(
                    "result.sender.balance is old(sender.balance) - amount".to_string(),
                ),
            },
        ],
        metadata: NodeMetadata {
            name: Some("transfer_money".to_string()),
            params: vec![
                Param { name: "sender".to_string(), type_ref: "User".to_string() },
                Param { name: "amount".to_string(), type_ref: "PositiveAmount".to_string() },
            ],
            return_type: Some("TransferResult".to_string()),
            ..Default::default()
        },
    };
    roundtrip(&node);
}

#[test]
fn roundtrip_node_with_all_contract_kinds() {
    let node = Node {
        id: NodeId::new(),
        intent: "validate wallet invariant".to_string(),
        pattern: Pattern::Do,
        children: Some(vec![]),
        expression: None,
        contracts: vec![
            Contract {
                kind: ContractKind::Before,
                expression: Expression("balance >= 0".to_string()),
            },
            Contract {
                kind: ContractKind::After,
                expression: Expression("result is not None".to_string()),
            },
            Contract {
                kind: ContractKind::Always,
                expression: Expression("account.balance >= 0".to_string()),
            },
        ],
        metadata: NodeMetadata::default(),
    };
    roundtrip(&node);
}

#[test]
fn roundtrip_all_pattern_variants() {
    let patterns = vec![
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
    assert_eq!(patterns.len(), 17, "expected 17 pattern variants");
    roundtrip(&patterns);
}

#[test]
fn roundtrip_all_edge_kinds() {
    let kinds = vec![EdgeKind::Ev, EdgeKind::Eh, EdgeKind::Ed];
    roundtrip(&kinds);
}

#[test]
fn roundtrip_node_metadata_all_fields() {
    let metadata = NodeMetadata {
        name: Some("transfer_error".to_string()),
        params: vec![
            Param { name: "sender_id".to_string(), type_ref: "UserId".to_string() },
        ],
        return_type: Some("TransferResult".to_string()),
        base_type: Some("number".to_string()),
        fields: vec![
            Field { name: "amount".to_string(), type_ref: "PositiveAmount".to_string() },
            Field { name: "timestamp".to_string(), type_ref: "timestamp".to_string() },
        ],
        carries: vec![
            Field { name: "reason".to_string(), type_ref: "text".to_string() },
        ],
    };
    roundtrip(&metadata);
}
