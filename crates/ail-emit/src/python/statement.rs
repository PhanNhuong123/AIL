use ail_graph::Node;
use ail_types::parse_constraint_expr;

use crate::constants::PYTHON_INDENT;
use crate::errors::EmitError;
use crate::python::constraint::render_constraint_python;
use crate::python::expression_parser::{
    parse_assignments, parse_fetch_expression, parse_return_with_expression,
    parse_update_expression,
};
use crate::python::type_map::{register_cross_file_type, resolve_python_type};
use crate::types::{EmitConfig, ImportSet};

/// Emit a `let` binding as a typed Python assignment.
///
/// `let x:T = expr` → `{indent}x: T = expr`
pub(crate) fn emit_let(
    node: &Node,
    indent: &str,
    imports: &mut ImportSet,
) -> Result<String, EmitError> {
    let expr = node.expression.as_ref().map(|e| e.0.as_str()).unwrap_or("");

    // Variable name and type from metadata (populated by ail-text parser).
    let var_name = node.metadata.name.as_deref().unwrap_or("_");
    let type_ref = node.metadata.return_type.as_deref().unwrap_or("");

    if type_ref.is_empty() {
        Ok(format!("{indent}{var_name} = {expr}"))
    } else {
        let py_type = resolve_python_type(type_ref, imports);
        Ok(format!("{indent}{var_name}: {py_type} = {expr}"))
    }
}

/// Emit a `check` guard as `if not (condition): raise Error(fields)`.
///
/// `check cond otherwise raise E carries f=v` →
/// ```python
/// if not (cond):
///     raise E(f=v)
/// ```
pub(crate) fn emit_check(
    node: &Node,
    indent: &str,
    imports: &mut ImportSet,
) -> Result<String, EmitError> {
    let raw_expr = node
        .expression
        .as_ref()
        .map(|e| e.0.as_str())
        .unwrap_or("True");

    // Parse the condition using the constraint AST for correct operator mapping.
    let py_cond = match parse_constraint_expr(raw_expr) {
        Ok(ast) => render_constraint_python(&ast, imports),
        Err(_) => raw_expr.to_owned(), // fall back to raw text if parse fails
    };

    let error_type = node
        .metadata
        .otherwise_error
        .as_deref()
        .unwrap_or("AssertionError");

    // Register user-defined error types for cross-file import.
    // "AssertionError" is a Python builtin and is filtered by register_cross_file_type.
    register_cross_file_type(error_type, imports);

    // Build the raise arguments from otherwise_assigns.
    let args = node
        .metadata
        .otherwise_assigns
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(", ");

    let inner = format!("{indent}{PYTHON_INDENT}");
    let raise_call = if args.is_empty() {
        format!("{inner}raise {error_type}()")
    } else {
        format!("{inner}raise {error_type}({args})")
    };

    Ok(format!("{indent}if not ({py_cond}):\n{raise_call}"))
}

/// Emit a `match` expression as Python 3.10+ structural pattern matching.
///
/// ```python
/// match discriminant:
///     case "V1":
///         expr1
///     case _:
///         otherwise_expr
/// ```
pub(crate) fn emit_match(
    node: &Node,
    indent: &str,
    _imports: &mut ImportSet,
) -> Result<String, EmitError> {
    let discriminant = node
        .metadata
        .discriminant
        .as_deref()
        .unwrap_or("_match_value");

    let arm_indent = format!("{indent}{PYTHON_INDENT}");
    let body_indent = format!("{indent}{PYTHON_INDENT}{PYTHON_INDENT}");

    let mut lines = vec![format!("{indent}match {discriminant}:")];

    for (value, then_expr) in &node.metadata.arms {
        lines.push(format!("{arm_indent}case {value}:"));
        lines.push(format!("{body_indent}{then_expr}"));
    }

    if let Some(otherwise) = &node.metadata.otherwise_result {
        lines.push(format!("{arm_indent}case _:"));
        lines.push(format!("{body_indent}{otherwise}"));
    }

    // Ensure there is at least a default arm if no otherwise provided.
    if node.metadata.otherwise_result.is_none() && !node.metadata.arms.is_empty() {
        lines.push(format!("{arm_indent}case _:"));
        lines.push(format!("{body_indent}pass"));
    }

    Ok(lines.join("\n"))
}

/// Emit a `fetch` as a repository get call.
///
/// `fetch x:T from src where id is val` →
/// - sync:  `x = repo.get(T, {"id": val})`
/// - async: `x = await repo.get(T, {"id": val})`
pub(crate) fn emit_fetch(
    node: &Node,
    indent: &str,
    config: &EmitConfig,
    imports: &mut ImportSet,
) -> Result<String, EmitError> {
    let var_name = node
        .metadata
        .name
        .as_deref()
        .ok_or(EmitError::FetchNodeMissingName { node_id: node.id })?;

    let entity_type = node.metadata.return_type.as_deref().unwrap_or("object");

    // Register user-defined entity types for cross-file import.
    // The type is passed as a runtime value to repo.get(T, ...).
    register_cross_file_type(entity_type, imports);

    let raw_expr = node.expression.as_ref().map(|e| e.0.as_str()).unwrap_or("");
    let (_source, conditions) = parse_fetch_expression(raw_expr);

    let cond_dict = conditions_to_dict(&conditions);
    let await_kw = if config.async_mode {
        imports.needs_asyncio = true;
        "await "
    } else {
        ""
    };

    Ok(format!(
        "{indent}{var_name} = {await_kw}repo.get({entity_type}, {cond_dict})"
    ))
}

/// Emit a `save` as a repository save call.
///
/// `save x to dst` →
/// - sync:  `repo.save(x)`
/// - async: `await repo.save(x)`
pub(crate) fn emit_save(
    node: &Node,
    indent: &str,
    config: &EmitConfig,
    imports: &mut ImportSet,
) -> Result<String, EmitError> {
    let entity_name = node
        .metadata
        .name
        .as_deref()
        .ok_or(EmitError::SaveNodeMissingName { node_id: node.id })?;

    let await_kw = if config.async_mode {
        imports.needs_asyncio = true;
        "await "
    } else {
        ""
    };

    Ok(format!("{indent}{await_kw}repo.save({entity_name})"))
}

/// Emit an `update` as a repository update call.
///
/// `update T in src where id is val set field = new_val` →
/// - sync:  `repo.update(T, {"id": val}, {"field": new_val})`
/// - async: `await repo.update(T, {"id": val}, {"field": new_val})`
pub(crate) fn emit_update(
    node: &Node,
    indent: &str,
    config: &EmitConfig,
    imports: &mut ImportSet,
) -> Result<String, EmitError> {
    let entity_type = node.metadata.return_type.as_deref().unwrap_or("object");

    // Register user-defined entity types for cross-file import.
    // The type is passed as a runtime value to repo.update(T, ...).
    register_cross_file_type(entity_type, imports);

    let raw_expr = node.expression.as_ref().map(|e| e.0.as_str()).unwrap_or("");
    let (_source, conditions, set_assignments) = parse_update_expression(raw_expr);

    let cond_dict = conditions_to_dict(&conditions);
    let set_dict = assignments_to_dict(&set_assignments);
    let await_kw = if config.async_mode {
        imports.needs_asyncio = true;
        "await "
    } else {
        ""
    };

    Ok(format!(
        "{indent}{await_kw}repo.update({entity_type}, {cond_dict}, {set_dict})"
    ))
}

/// Emit a `remove` as a repository delete call.
///
/// `remove T from src where id is val` →
/// - sync:  `repo.delete(T, {"id": val})`
/// - async: `await repo.delete(T, {"id": val})`
pub(crate) fn emit_remove(
    node: &Node,
    indent: &str,
    config: &EmitConfig,
    imports: &mut ImportSet,
) -> Result<String, EmitError> {
    let entity_type = node.metadata.return_type.as_deref().unwrap_or("object");

    // Register user-defined entity types for cross-file import.
    // The type is passed as a runtime value to repo.delete(T, ...).
    register_cross_file_type(entity_type, imports);

    let raw_expr = node.expression.as_ref().map(|e| e.0.as_str()).unwrap_or("");
    let (_source, conditions) = parse_fetch_expression(raw_expr);

    let cond_dict = conditions_to_dict(&conditions);
    let await_kw = if config.async_mode {
        imports.needs_asyncio = true;
        "await "
    } else {
        ""
    };

    Ok(format!(
        "{indent}{await_kw}repo.delete({entity_type}, {cond_dict})"
    ))
}

/// Emit a `return with` as a type constructor call.
///
/// `return TransferResult with sender = s, receiver = r` →
/// `return TransferResult(sender=s, receiver=r)`
///
/// Uses the constructor form `T(field=val, ...)` for frozen dataclass construction.
pub(crate) fn emit_return(
    node: &Node,
    indent: &str,
    imports: &mut ImportSet,
) -> Result<String, EmitError> {
    let type_name = node
        .metadata
        .name
        .as_deref()
        .ok_or(EmitError::ReturnNodeMissingName { node_id: node.id })?;

    // Register the return type for cross-file import.
    // `return with TransferResult(...)` is a runtime constructor call, so the
    // class must be imported — `from __future__ import annotations` does not help here.
    register_cross_file_type(type_name, imports);

    let raw_expr = node.expression.as_ref().map(|e| e.0.as_str()).unwrap_or("");
    let fields = parse_return_with_expression(raw_expr);

    if fields.is_empty() {
        Ok(format!("{indent}return {type_name}()"))
    } else {
        let args = fields
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(", ");
        Ok(format!("{indent}return {type_name}({args})"))
    }
}

/// Emit a `raise carries` as a Python exception raise.
///
/// `raise InsufficientBalanceError carries current_balance = bal` →
/// `raise InsufficientBalanceError(current_balance=bal)`
pub(crate) fn emit_raise(
    node: &Node,
    indent: &str,
    imports: &mut ImportSet,
) -> Result<String, EmitError> {
    let error_type = node
        .metadata
        .name
        .as_deref()
        .ok_or(EmitError::RaiseNodeMissingName { node_id: node.id })?;

    // Register the error type for cross-file import.
    // `raise InsufficientBalanceError(...)` is a runtime constructor call.
    register_cross_file_type(error_type, imports);

    // carries fields come from metadata.carries (populated by ail-text parser).
    // The values should be in node.expression or otherwise_assigns.
    // In practice, Raise nodes store the carried values as metadata.otherwise_assigns
    // (same structure as Check's carries clause).
    let fields = if !node.metadata.otherwise_assigns.is_empty() {
        node.metadata.otherwise_assigns.clone()
    } else if let Some(expr) = &node.expression {
        // Fallback: parse expression if it looks like "field = val, ..."
        parse_assignments(&expr.0)
    } else {
        vec![]
    };

    if fields.is_empty() {
        Ok(format!("{indent}raise {error_type}()"))
    } else {
        let args = fields
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(", ");
        Ok(format!("{indent}raise {error_type}({args})"))
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Convert `[(key, value)]` to a Python dict literal `{"key": value}`.
///
/// Values are unquoted (treated as Python expressions, not string literals).
///
/// **Known limitation**: `"is not"` conditions are stored with a `"not:"` prefix by
/// `parse_key_val_condition`, but standard repository `where` dicts only support equality.
/// The `"not:"` prefix is stripped here — the negation semantics are lost. `"is not"` in
/// a `where` clause is uncommon in AIL (it appears in `check` guards, not `fetch`/`remove`
/// expressions), so this is acceptable for Phase 5a. Phase 5b should add `NotEqual`
/// support to the repository call convention if needed.
fn conditions_to_dict(conditions: &[(String, String)]) -> String {
    if conditions.is_empty() {
        return "{}".to_owned();
    }
    let pairs: Vec<_> = conditions
        .iter()
        .map(|(k, v)| {
            // Strip "not:" prefix — see known limitation above.
            let val = v.strip_prefix("not:").unwrap_or(v.as_str());
            format!("\"{k}\": {val}")
        })
        .collect();
    format!("{{{}}}", pairs.join(", "))
}

/// Convert `[(field, expr)]` to a Python dict literal `{"field": expr}`.
fn assignments_to_dict(assignments: &[(String, String)]) -> String {
    if assignments.is_empty() {
        return "{}".to_owned();
    }
    let pairs: Vec<_> = assignments
        .iter()
        .map(|(k, v)| format!("\"{k}\": {v}"))
        .collect();
    format!("{{{}}}", pairs.join(", "))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ail_graph::*;

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

    fn sync_config() -> EmitConfig {
        EmitConfig { async_mode: false, ..Default::default() }
    }
    fn async_config() -> EmitConfig {
        EmitConfig { async_mode: true, ..Default::default() }
    }

    // ── Let ───────────────────────────────────────────────────────────────────

    #[test]
    fn emit_let_simple_binding() {
        let mut node = make_node(Pattern::Let, "compute balance");
        node.expression = Some(Expression("sender.balance - amount".to_owned()));
        node.metadata.name = Some("new_balance".to_owned());
        let mut imports = ImportSet::new();
        let result = emit_let(&node, "    ", &mut imports).unwrap();
        assert_eq!(result, "    new_balance = sender.balance - amount");
    }

    #[test]
    fn emit_let_with_type_annotation() {
        let mut node = make_node(Pattern::Let, "compute sender balance");
        node.expression = Some(Expression("sender.balance - amount".to_owned()));
        node.metadata.name = Some("new_sender_balance".to_owned());
        node.metadata.return_type = Some("WalletBalance".to_owned());
        let mut imports = ImportSet::new();
        let result = emit_let(&node, "    ", &mut imports).unwrap();
        assert_eq!(
            result,
            "    new_sender_balance: WalletBalance = sender.balance - amount"
        );
    }

    // ── Check ─────────────────────────────────────────────────────────────────

    #[test]
    fn emit_check_raises_with_carries() {
        let mut node = make_node(Pattern::Check, "validate sender");
        node.expression = Some(Expression("sender_id is not receiver_id".to_owned()));
        node.metadata.otherwise_error = Some("InvalidTransferError".to_owned());
        node.metadata.otherwise_assigns = vec![("user_id".to_owned(), "sender_id".to_owned())];
        let mut imports = ImportSet::new();
        let result = emit_check(&node, "    ", &mut imports).unwrap();
        assert!(result.contains("if not ("));
        assert!(result.contains("raise InvalidTransferError(user_id=sender_id)"));
    }

    #[test]
    fn emit_check_no_assigns() {
        let mut node = make_node(Pattern::Check, "check balance");
        node.expression = Some(Expression("amount > 0".to_owned()));
        node.metadata.otherwise_error = Some("NegativeAmountError".to_owned());
        let mut imports = ImportSet::new();
        let result = emit_check(&node, "    ", &mut imports).unwrap();
        assert!(result.contains("raise NegativeAmountError()"));
    }

    // ── Return ────────────────────────────────────────────────────────────────

    #[test]
    fn emit_return_with_fields_uses_constructor() {
        let mut node = make_node(Pattern::Return, "return result");
        node.metadata.name = Some("TransferResult".to_owned());
        node.expression = Some(Expression(
            "with sender = sender, receiver = receiver".to_owned(),
        ));
        let mut imports = ImportSet::new();
        let result = emit_return(&node, "    ", &mut imports).unwrap();
        assert_eq!(
            result,
            "    return TransferResult(sender=sender, receiver=receiver)"
        );
    }

    #[test]
    fn emit_return_no_fields_just_type() {
        let mut node = make_node(Pattern::Return, "return result");
        node.metadata.name = Some("VoidResult".to_owned());
        let mut imports = ImportSet::new();
        let result = emit_return(&node, "    ", &mut imports).unwrap();
        assert_eq!(result, "    return VoidResult()");
    }

    #[test]
    fn emit_return_missing_name_returns_error() {
        let node = make_node(Pattern::Return, "return result");
        let mut imports = ImportSet::new();
        let err = emit_return(&node, "    ", &mut imports).unwrap_err();
        assert!(matches!(err, EmitError::ReturnNodeMissingName { .. }));
    }

    // ── Raise ─────────────────────────────────────────────────────────────────

    #[test]
    fn emit_raise_with_carries_fields() {
        let mut node = make_node(Pattern::Raise, "raise error");
        node.metadata.name = Some("InsufficientBalanceError".to_owned());
        node.metadata.otherwise_assigns = vec![
            ("current_balance".to_owned(), "sender.balance".to_owned()),
            ("requested_amount".to_owned(), "amount".to_owned()),
        ];
        let mut imports = ImportSet::new();
        let result = emit_raise(&node, "    ", &mut imports).unwrap();
        assert_eq!(
            result,
            "    raise InsufficientBalanceError(current_balance=sender.balance, requested_amount=amount)"
        );
    }

    #[test]
    fn emit_raise_missing_name_returns_error() {
        let node = make_node(Pattern::Raise, "raise error");
        let mut imports = ImportSet::new();
        let err = emit_raise(&node, "    ", &mut imports).unwrap_err();
        assert!(matches!(err, EmitError::RaiseNodeMissingName { .. }));
    }

    // ── Cross-file import registration ───────────────────────────────────────

    #[test]
    fn emit_return_registers_cross_file_type() {
        let mut node = make_node(Pattern::Return, "return result");
        node.metadata.name = Some("TransferResult".to_owned());
        let mut imports = ImportSet::new();
        emit_return(&node, "    ", &mut imports).unwrap();
        assert!(imports.cross_file_types.contains("TransferResult"));
    }

    #[test]
    fn emit_raise_registers_cross_file_type() {
        let mut node = make_node(Pattern::Raise, "raise error");
        node.metadata.name = Some("InsufficientBalanceError".to_owned());
        let mut imports = ImportSet::new();
        emit_raise(&node, "    ", &mut imports).unwrap();
        assert!(imports.cross_file_types.contains("InsufficientBalanceError"));
    }

    #[test]
    fn emit_check_registers_error_type() {
        let mut node = make_node(Pattern::Check, "validate amount");
        node.expression = Some(Expression("amount > 0".to_owned()));
        node.metadata.otherwise_error = Some("NegativeAmountError".to_owned());
        let mut imports = ImportSet::new();
        emit_check(&node, "    ", &mut imports).unwrap();
        assert!(imports.cross_file_types.contains("NegativeAmountError"));
    }

    #[test]
    fn emit_check_builtin_error_not_registered() {
        let mut node = make_node(Pattern::Check, "guard");
        node.expression = Some(Expression("x > 0".to_owned()));
        // Default error is AssertionError — a Python builtin, must NOT be cross-imported.
        let mut imports = ImportSet::new();
        emit_check(&node, "    ", &mut imports).unwrap();
        assert!(imports.cross_file_types.is_empty());
    }

    #[test]
    fn emit_fetch_registers_entity_type() {
        let mut node = make_node(Pattern::Fetch, "fetch user");
        node.metadata.name = Some("user".to_owned());
        node.metadata.return_type = Some("User".to_owned());
        node.expression = Some(Expression("from db where id is user_id".to_owned()));
        let config = EmitConfig { async_mode: false, ..Default::default() };
        let mut imports = ImportSet::new();
        emit_fetch(&node, "    ", &config, &mut imports).unwrap();
        assert!(imports.cross_file_types.contains("User"));
    }

    #[test]
    fn emit_update_registers_entity_type() {
        let mut node = make_node(Pattern::Update, "update account");
        node.metadata.return_type = Some("Account".to_owned());
        node.expression = Some(Expression(
            "in db where id is account.id set balance = new_balance".to_owned(),
        ));
        let config = EmitConfig { async_mode: false, ..Default::default() };
        let mut imports = ImportSet::new();
        emit_update(&node, "    ", &config, &mut imports).unwrap();
        assert!(imports.cross_file_types.contains("Account"));
    }

    #[test]
    fn emit_remove_registers_entity_type() {
        let mut node = make_node(Pattern::Remove, "remove session");
        node.metadata.return_type = Some("Session".to_owned());
        node.expression = Some(Expression("from store where token is expired".to_owned()));
        let config = EmitConfig { async_mode: false, ..Default::default() };
        let mut imports = ImportSet::new();
        emit_remove(&node, "    ", &config, &mut imports).unwrap();
        assert!(imports.cross_file_types.contains("Session"));
    }

    // ── Fetch ─────────────────────────────────────────────────────────────────

    #[test]
    fn emit_fetch_sync_repo_get() {
        let mut node = make_node(Pattern::Fetch, "fetch sender");
        node.metadata.name = Some("sender".to_owned());
        node.metadata.return_type = Some("User".to_owned());
        node.expression = Some(Expression("from database where id is sender_id".to_owned()));
        let mut imports = ImportSet::new();
        let result = emit_fetch(&node, "    ", &sync_config(), &mut imports).unwrap();
        assert_eq!(result, "    sender = repo.get(User, {\"id\": sender_id})");
        assert!(!imports.needs_asyncio);
    }

    #[test]
    fn emit_fetch_async_adds_await() {
        let mut node = make_node(Pattern::Fetch, "fetch sender");
        node.metadata.name = Some("sender".to_owned());
        node.metadata.return_type = Some("User".to_owned());
        node.expression = Some(Expression("from database where id is sender_id".to_owned()));
        let mut imports = ImportSet::new();
        let result = emit_fetch(&node, "    ", &async_config(), &mut imports).unwrap();
        assert!(result.contains("await repo.get("));
        assert!(imports.needs_asyncio);
    }

    #[test]
    fn emit_fetch_missing_name_returns_error() {
        let node = make_node(Pattern::Fetch, "fetch data");
        let mut imports = ImportSet::new();
        let err = emit_fetch(&node, "    ", &sync_config(), &mut imports).unwrap_err();
        assert!(matches!(err, EmitError::FetchNodeMissingName { .. }));
    }

    // ── Save ──────────────────────────────────────────────────────────────────

    #[test]
    fn emit_save_sync() {
        let mut node = make_node(Pattern::Save, "save user");
        node.metadata.name = Some("user".to_owned());
        node.expression = Some(Expression("to database".to_owned()));
        let mut imports = ImportSet::new();
        let result = emit_save(&node, "    ", &sync_config(), &mut imports).unwrap();
        assert_eq!(result, "    repo.save(user)");
    }

    // ── Update ────────────────────────────────────────────────────────────────

    #[test]
    fn emit_update_sync_with_set() {
        let mut node = make_node(Pattern::Update, "update sender balance");
        node.metadata.return_type = Some("User".to_owned());
        node.expression = Some(Expression(
            "in database where id is sender.id set balance = new_sender_balance".to_owned(),
        ));
        let mut imports = ImportSet::new();
        let result = emit_update(&node, "    ", &sync_config(), &mut imports).unwrap();
        assert_eq!(
            result,
            "    repo.update(User, {\"id\": sender.id}, {\"balance\": new_sender_balance})"
        );
    }

    // ── Remove ────────────────────────────────────────────────────────────────

    #[test]
    fn emit_remove_with_condition() {
        let mut node = make_node(Pattern::Remove, "remove session");
        node.metadata.return_type = Some("Session".to_owned());
        node.expression = Some(Expression(
            "from store where token is expired_token".to_owned(),
        ));
        let mut imports = ImportSet::new();
        let result = emit_remove(&node, "    ", &sync_config(), &mut imports).unwrap();
        assert_eq!(
            result,
            "    repo.delete(Session, {\"token\": expired_token})"
        );
    }

    // ── Match ─────────────────────────────────────────────────────────────────

    #[test]
    fn emit_match_with_arms_and_otherwise() {
        let mut node = make_node(Pattern::Match, "match user status");
        node.metadata.discriminant = Some("user.status".to_owned());
        node.metadata.arms = vec![
            ("\"active\"".to_owned(), "pass".to_owned()),
            ("\"pending\"".to_owned(), "wait()".to_owned()),
        ];
        node.metadata.otherwise_result = Some("raise UnknownStatusError()".to_owned());
        let mut imports = ImportSet::new();
        let result = emit_match(&node, "    ", &mut imports).unwrap();
        assert!(result.contains("match user.status:"));
        assert!(result.contains("case \"active\":"));
        assert!(result.contains("case \"pending\":"));
        assert!(result.contains("case _:"));
        assert!(result.contains("raise UnknownStatusError()"));
    }

    #[test]
    fn emit_match_no_otherwise_adds_pass() {
        let mut node = make_node(Pattern::Match, "match status");
        node.metadata.discriminant = Some("status".to_owned());
        node.metadata.arms = vec![("\"active\"".to_owned(), "proceed()".to_owned())];
        let mut imports = ImportSet::new();
        let result = emit_match(&node, "    ", &mut imports).unwrap();
        // Without otherwise, should emit a default `case _: pass`.
        assert!(result.contains("case _:"));
        assert!(result.contains("pass"));
    }
}
