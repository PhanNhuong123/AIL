use std::collections::HashMap;

use ail_graph::Node;
use ail_types::parse_constraint_expr;

use crate::errors::EmitError;
use crate::python::expression_parser::{
    parse_assignments, parse_fetch_expression, parse_return_with_expression,
    parse_update_expression,
};
use crate::typescript::constraint::render_constraint_ts;
use crate::typescript::fn_name::to_camel_case_var;
use crate::typescript::import_tracker::TypeKind;
use crate::typescript::type_map::resolve_ts_type;

// ── Let ────────────────────────────────────────────────────────────────────────

/// Emit a `let` binding as a TypeScript `const` declaration.
///
/// When the declared type is a `define` type, the RHS is wrapped with
/// the factory function: `const x: T = createT(expr);`.
pub(crate) fn emit_let_ts(
    node: &Node,
    indent: &str,
    type_registry: &HashMap<String, TypeKind>,
) -> Result<String, EmitError> {
    let expr = node.expression.as_ref().map(|e| e.0.as_str()).unwrap_or("");
    let var_raw = node.metadata.name.as_deref().unwrap_or("_");
    let var_name = to_camel_case_var(var_raw);
    let type_ref = node.metadata.return_type.as_deref().unwrap_or("");

    if type_ref.is_empty() {
        return Ok(format!("{indent}const {var_name} = {expr};"));
    }

    let ts_type = resolve_ts_type(type_ref);
    let rhs = if type_registry.get(type_ref) == Some(&TypeKind::Define) {
        format!("create{type_ref}({expr})")
    } else {
        expr.to_owned()
    };
    Ok(format!("{indent}const {var_name}: {ts_type} = {rhs};"))
}

// ── Check ──────────────────────────────────────────────────────────────────────

/// Emit a `check` guard as `if (!cond) { throw new Error({fields}); }`.
pub(crate) fn emit_check_ts(node: &Node, indent: &str) -> Result<String, EmitError> {
    let raw_expr = node
        .expression
        .as_ref()
        .map(|e| e.0.as_str())
        .unwrap_or("true");

    let ts_cond = match parse_constraint_expr(raw_expr) {
        Ok(ast) => render_constraint_ts(&ast),
        Err(_) => raw_expr.to_owned(),
    };

    let error_type = node.metadata.otherwise_error.as_deref().unwrap_or("Error");

    let inner = format!("{indent}  ");
    let throw_call = if node.metadata.otherwise_assigns.is_empty() {
        format!("{inner}throw new {error_type}({{}});")
    } else {
        let args = node
            .metadata
            .otherwise_assigns
            .iter()
            .map(|(k, v)| format!("{k}: {}", to_camel_case_var(v)))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{inner}throw new {error_type}({{ {args} }});")
    };

    Ok(format!(
        "{indent}if (!({ts_cond})) {{\n{throw_call}\n{indent}}}"
    ))
}

// ── Match ──────────────────────────────────────────────────────────────────────

/// Emit a `match` as a TypeScript `switch` statement.
///
/// Double-quoted arm values are converted to single-quoted TypeScript strings.
/// `"proceed"` arms emit a `break`. `raise` arms emit `throw new`.
pub(crate) fn emit_match_ts(node: &Node, indent: &str) -> Result<String, EmitError> {
    let discriminant = node.metadata.discriminant.as_deref().unwrap_or("_value");

    let arm_indent = format!("{indent}  ");
    let body_indent = format!("{indent}    ");

    let mut lines = vec![format!("{indent}switch ({discriminant}) {{")];

    for (value, then_expr) in &node.metadata.arms {
        let ts_value = dequote_to_single(value);
        lines.push(format!("{arm_indent}case {ts_value}:"));
        emit_arm_body(&body_indent, then_expr, &mut lines);
    }

    if let Some(otherwise) = &node.metadata.otherwise_result {
        lines.push(format!("{arm_indent}default:"));
        emit_arm_body(&body_indent, otherwise, &mut lines);
    } else if !node.metadata.arms.is_empty() {
        lines.push(format!("{arm_indent}default:"));
        lines.push(format!("{body_indent}break;"));
    }

    lines.push(format!("{indent}}}"));
    Ok(lines.join("\n"))
}

fn emit_arm_body(body_indent: &str, then_expr: &str, lines: &mut Vec<String>) {
    let trimmed = then_expr.trim();
    if trimmed.is_empty() || trimmed == "proceed" {
        lines.push(format!("{body_indent}break;"));
    } else if let Some(rest) = trimmed.strip_prefix("raise ") {
        let error = rest.trim();
        lines.push(format!("{body_indent}throw new {error}();"));
    } else {
        lines.push(format!("{body_indent}{trimmed}"));
        lines.push(format!("{body_indent}break;"));
    }
}

// ── Fetch / Save / Update / Remove ────────────────────────────────────────────

/// Emit a `fetch` as `const var: T = await source.findT({ key: val });`.
///
/// When `tx_name` is `Some("tx")` (inside a `together` block), the source is
/// replaced with the transaction proxy name.
pub(crate) fn emit_fetch_ts(
    node: &Node,
    indent: &str,
    tx_name: Option<&str>,
) -> Result<String, EmitError> {
    let var_raw = node
        .metadata
        .name
        .as_deref()
        .ok_or(EmitError::TsFetchNodeMissingName { node_id: node.id })?;
    let var_name = to_camel_case_var(var_raw);
    let entity_type = node.metadata.return_type.as_deref().unwrap_or("unknown");
    let ts_type = resolve_ts_type(entity_type);

    let raw_expr = node.expression.as_ref().map(|e| e.0.as_str()).unwrap_or("");
    let (source_opt, conditions) = parse_fetch_expression(raw_expr);
    let source = tx_name
        .map(str::to_owned)
        .unwrap_or_else(|| source_opt.unwrap_or_else(|| "repository".to_owned()));

    let cond_obj = conditions_to_ts_obj(&conditions);
    Ok(format!(
        "{indent}const {var_name}: {ts_type} = await {source}.find{entity_type}({cond_obj});"
    ))
}

/// Emit a `save` as `await source.saveEntity(var);` or with fields object.
pub(crate) fn emit_save_ts(
    node: &Node,
    indent: &str,
    tx_name: Option<&str>,
) -> Result<String, EmitError> {
    let entity_raw = node
        .metadata
        .name
        .as_deref()
        .ok_or(EmitError::TsSaveNodeMissingName { node_id: node.id })?;

    let raw_expr = node.expression.as_ref().map(|e| e.0.as_str()).unwrap_or("");
    let (parsed_source, fields) = parse_save_expression(raw_expr);
    let source = tx_name.map(str::to_owned).unwrap_or(parsed_source);

    let entity_type = node.metadata.return_type.as_deref().unwrap_or(entity_raw);
    let pascal = capitalize_first(entity_type);

    if fields.is_empty() {
        Ok(format!(
            "{indent}await {source}.save{pascal}({entity_raw});"
        ))
    } else {
        let fields_obj = assignments_to_ts_obj(&fields);
        Ok(format!(
            "{indent}await {source}.save{pascal}({fields_obj});"
        ))
    }
}

/// Emit an `update` as `await source.updateT({ where }, { set });`.
pub(crate) fn emit_update_ts(
    node: &Node,
    indent: &str,
    tx_name: Option<&str>,
) -> Result<String, EmitError> {
    let entity_type = node.metadata.return_type.as_deref().unwrap_or("Entity");

    let raw_expr = node.expression.as_ref().map(|e| e.0.as_str()).unwrap_or("");
    let (source_opt, conditions, set_assignments) = parse_update_expression(raw_expr);
    let source = tx_name
        .map(str::to_owned)
        .unwrap_or_else(|| source_opt.unwrap_or_else(|| "repository".to_owned()));

    let where_obj = conditions_to_ts_obj(&conditions);
    let set_obj = assignments_to_ts_obj(&set_assignments);
    Ok(format!(
        "{indent}await {source}.update{entity_type}({where_obj}, {set_obj});"
    ))
}

/// Emit a `remove` as `await source.removeT({ key: val });`.
pub(crate) fn emit_remove_ts(
    node: &Node,
    indent: &str,
    tx_name: Option<&str>,
) -> Result<String, EmitError> {
    let entity_type = node.metadata.return_type.as_deref().unwrap_or("Entity");

    let raw_expr = node.expression.as_ref().map(|e| e.0.as_str()).unwrap_or("");
    let (source_opt, conditions) = parse_fetch_expression(raw_expr);
    let source = tx_name
        .map(str::to_owned)
        .unwrap_or_else(|| source_opt.unwrap_or_else(|| "repository".to_owned()));

    let where_obj = conditions_to_ts_obj(&conditions);
    Ok(format!(
        "{indent}await {source}.remove{entity_type}({where_obj});"
    ))
}

// ── Return / Raise ─────────────────────────────────────────────────────────────

/// Emit a `return with` as `return createT({ field: camelVal, … });`.
pub(crate) fn emit_return_ts(
    node: &Node,
    indent: &str,
    _type_registry: &HashMap<String, TypeKind>,
) -> Result<String, EmitError> {
    let type_name = node
        .metadata
        .name
        .as_deref()
        .ok_or(EmitError::TsReturnNodeMissingName { node_id: node.id })?;

    let raw_expr = node.expression.as_ref().map(|e| e.0.as_str()).unwrap_or("");
    let fields = parse_return_with_expression(raw_expr);

    if fields.is_empty() {
        return Ok(format!("{indent}return create{type_name}({{}});"));
    }

    let args = fields
        .iter()
        .map(|(k, v)| format!("  {k}: {}", to_camel_case_var(v)))
        .collect::<Vec<_>>()
        .join(",\n");
    Ok(format!(
        "{indent}return create{type_name}({{\n{args},\n{indent}}});"
    ))
}

/// Emit a `raise carries` as `throw new T({ field: camelVal });`.
pub(crate) fn emit_raise_ts(node: &Node, indent: &str) -> Result<String, EmitError> {
    let error_type = node
        .metadata
        .name
        .as_deref()
        .ok_or(EmitError::TsRaiseNodeMissingName { node_id: node.id })?;

    let fields: Vec<(String, String)> = if !node.metadata.otherwise_assigns.is_empty() {
        node.metadata.otherwise_assigns.clone()
    } else if let Some(expr) = &node.expression {
        parse_assignments(&expr.0)
    } else {
        vec![]
    };

    if fields.is_empty() {
        return Ok(format!("{indent}throw new {error_type}({{}});"));
    }

    let args = fields
        .iter()
        .map(|(k, v)| format!("{k}: {}", to_camel_case_var(v)))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!("{indent}throw new {error_type}({{ {args} }});"))
}

// ── Private helpers ────────────────────────────────────────────────────────────

/// Convert `[(key, value)]` conditions to `{ key: camelVal }` TS object literal.
pub(crate) fn conditions_to_ts_obj(conditions: &[(String, String)]) -> String {
    if conditions.is_empty() {
        return "{}".to_owned();
    }
    let pairs: Vec<_> = conditions
        .iter()
        .map(|(k, v)| {
            let val = v.strip_prefix("not:").unwrap_or(v);
            format!("{k}: {}", to_camel_case_var(val))
        })
        .collect();
    format!("{{ {} }}", pairs.join(", "))
}

/// Convert `[(field, expr)]` to `{ field: camelExpr }` TS object literal.
pub(crate) fn assignments_to_ts_obj(assignments: &[(String, String)]) -> String {
    if assignments.is_empty() {
        return "{}".to_owned();
    }
    let pairs: Vec<_> = assignments
        .iter()
        .map(|(k, v)| format!("{k}: {}", to_camel_case_var(v)))
        .collect();
    format!("{{ {} }}", pairs.join(", "))
}

/// Parse `"to {source} [with k=v, …]"` from a Save node expression.
fn parse_save_expression(expr: &str) -> (String, Vec<(String, String)>) {
    let s = expr.trim();
    let without_to = match s.strip_prefix("to ") {
        Some(rest) => rest,
        None => return ("repository".to_owned(), vec![]),
    };
    if let Some(idx) = without_to.find(" with ") {
        let source = without_to[..idx].trim().to_owned();
        let fields_str = without_to[idx + " with ".len()..].trim();
        (source, parse_assignments(fields_str))
    } else {
        (without_to.trim().to_owned(), vec![])
    }
}

/// Convert double-quoted string `"value"` to single-quoted `'value'` for TS.
fn dequote_to_single(s: &str) -> String {
    let t = s.trim();
    if t.starts_with('"') && t.ends_with('"') && t.len() >= 2 {
        let inner = &t[1..t.len() - 1];
        return format!("'{inner}'");
    }
    t.to_owned()
}

/// Capitalize the first character of a string.
pub(crate) fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let upper: String = first.to_uppercase().collect();
            upper + chars.as_str()
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ail_graph::{Expression, NodeId, NodeMetadata, Pattern};

    fn make_node(pattern: Pattern, intent: &str) -> Node {
        Node {
            id: NodeId::new(),
            intent: intent.to_owned(),
            pattern,
            children: None,
            expression: None,
            contracts: vec![],
            metadata: NodeMetadata::default(),
        }
    }

    #[test]
    fn let_simple_no_type() {
        let mut node = make_node(Pattern::Let, "compute");
        node.expression = Some(Expression("sender.balance - amount".to_owned()));
        node.metadata.name = Some("new_balance".to_owned());
        let result = emit_let_ts(&node, "  ", &HashMap::new()).unwrap();
        assert_eq!(result, "  const newBalance = sender.balance - amount;");
    }

    #[test]
    fn let_with_type_no_factory() {
        let mut node = make_node(Pattern::Let, "compute");
        node.expression = Some(Expression("sender.balance - amount".to_owned()));
        node.metadata.name = Some("new_balance".to_owned());
        node.metadata.return_type = Some("WalletBalance".to_owned());
        let mut registry = HashMap::new();
        registry.insert("WalletBalance".to_owned(), TypeKind::Describe);
        let result = emit_let_ts(&node, "  ", &registry).unwrap();
        assert_eq!(
            result,
            "  const newBalance: WalletBalance = sender.balance - amount;"
        );
    }

    #[test]
    fn let_with_define_type_wraps_factory() {
        let mut node = make_node(Pattern::Let, "compute");
        node.expression = Some(Expression("sender.balance - amount".to_owned()));
        node.metadata.name = Some("new_balance".to_owned());
        node.metadata.return_type = Some("WalletBalance".to_owned());
        let mut registry = HashMap::new();
        registry.insert("WalletBalance".to_owned(), TypeKind::Define);
        let result = emit_let_ts(&node, "  ", &registry).unwrap();
        assert_eq!(
            result,
            "  const newBalance: WalletBalance = createWalletBalance(sender.balance - amount);"
        );
    }

    #[test]
    fn check_emits_if_throw() {
        let mut node = make_node(Pattern::Check, "check status");
        node.expression = Some(Expression("sender.status is \"active\"".to_owned()));
        node.metadata.otherwise_error = Some("InvalidUserError".to_owned());
        let result = emit_check_ts(&node, "  ").unwrap();
        assert!(result.contains("if (!("));
        assert!(result.contains("throw new InvalidUserError({})"));
    }

    #[test]
    fn check_with_carries_passes_fields() {
        let mut node = make_node(Pattern::Check, "check sender");
        node.expression = Some(Expression("sender.status is \"active\"".to_owned()));
        node.metadata.otherwise_error = Some("InvalidUserError".to_owned());
        node.metadata.otherwise_assigns = vec![("user_id".to_owned(), "sender.id".to_owned())];
        let result = emit_check_ts(&node, "  ").unwrap();
        assert!(result.contains("user_id: sender.id"));
    }

    #[test]
    fn match_emits_switch() {
        let mut node = make_node(Pattern::Match, "check status");
        node.metadata.discriminant = Some("user.status".to_owned());
        node.metadata.arms = vec![
            ("\"active\"".to_owned(), "proceed".to_owned()),
            (
                "\"suspended\"".to_owned(),
                "raise AccountSuspendedError".to_owned(),
            ),
        ];
        node.metadata.otherwise_result = Some("raise UnknownStatusError".to_owned());
        let result = emit_match_ts(&node, "  ").unwrap();
        assert!(result.contains("switch (user.status)"));
        assert!(result.contains("case 'active':"));
        assert!(result.contains("break;"));
        assert!(result.contains("throw new AccountSuspendedError();"));
        assert!(result.contains("default:"));
        assert!(result.contains("throw new UnknownStatusError();"));
    }

    #[test]
    fn fetch_emits_await_find() {
        let mut node = make_node(Pattern::Fetch, "fetch user");
        node.metadata.name = Some("user".to_owned());
        node.metadata.return_type = Some("User".to_owned());
        node.expression = Some(Expression("from database where id is userId".to_owned()));
        let result = emit_fetch_ts(&node, "  ", None).unwrap();
        assert_eq!(
            result,
            "  const user: User = await database.findUser({ id: userId });"
        );
    }

    #[test]
    fn fetch_inside_together_uses_tx() {
        let mut node = make_node(Pattern::Fetch, "fetch user");
        node.metadata.name = Some("user".to_owned());
        node.metadata.return_type = Some("User".to_owned());
        node.expression = Some(Expression("from database where id is userId".to_owned()));
        let result = emit_fetch_ts(&node, "  ", Some("tx")).unwrap();
        assert!(result.contains("tx.findUser("));
    }

    #[test]
    fn save_emits_await_save() {
        let mut node = make_node(Pattern::Save, "save user");
        node.metadata.name = Some("user".to_owned());
        node.metadata.return_type = Some("User".to_owned());
        node.expression = Some(Expression("to database".to_owned()));
        let result = emit_save_ts(&node, "  ", None).unwrap();
        assert_eq!(result, "  await database.saveUser(user);");
    }

    #[test]
    fn update_emits_await_update() {
        let mut node = make_node(Pattern::Update, "update user balance");
        node.metadata.return_type = Some("User".to_owned());
        node.expression = Some(Expression(
            "in database where id is sender.id set balance = newBalance".to_owned(),
        ));
        let result = emit_update_ts(&node, "  ", None).unwrap();
        assert_eq!(
            result,
            "  await database.updateUser({ id: sender.id }, { balance: newBalance });"
        );
    }

    #[test]
    fn remove_emits_await_remove() {
        let mut node = make_node(Pattern::Remove, "remove session");
        node.metadata.return_type = Some("Session".to_owned());
        node.expression = Some(Expression(
            "from store where token is expiredToken".to_owned(),
        ));
        let result = emit_remove_ts(&node, "  ", None).unwrap();
        assert_eq!(
            result,
            "  await store.removeSession({ token: expiredToken });"
        );
    }

    #[test]
    fn return_emits_create_factory() {
        let mut node = make_node(Pattern::Return, "return result");
        node.metadata.name = Some("TransferResult".to_owned());
        node.expression = Some(Expression(
            "with sender = sender, amount = amount".to_owned(),
        ));
        let result = emit_return_ts(&node, "  ", &HashMap::new()).unwrap();
        assert!(result.contains("return createTransferResult({"));
        assert!(result.contains("sender: sender"));
        assert!(result.contains("amount: amount"));
    }

    #[test]
    fn raise_emits_throw_new() {
        let mut node = make_node(Pattern::Raise, "raise error");
        node.metadata.name = Some("InsufficientBalanceError".to_owned());
        node.metadata.otherwise_assigns = vec![
            ("current_balance".to_owned(), "sender.balance".to_owned()),
            ("requested_amount".to_owned(), "amount".to_owned()),
        ];
        let result = emit_raise_ts(&node, "  ").unwrap();
        assert_eq!(
            result,
            "  throw new InsufficientBalanceError({ current_balance: sender.balance, requested_amount: amount });"
        );
    }
}
