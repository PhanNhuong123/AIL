use ail_graph::{Contract, ContractKind, Expression, Field, NodeMetadata, Param, Pattern};
use pest::iterators::{Pair, Pairs};

use crate::errors::ParseError;
use crate::grammar::Rule;
use crate::types::{ParsedStatement, SourceSpan};

// ── Entry point ──────────────────────────────────────────────────────────────

/// Walk the top-level `file` pairs and produce a flat list of statements.
pub(crate) fn walk_file(pairs: Pairs<'_, Rule>) -> Result<Vec<ParsedStatement>, ParseError> {
    let mut statements = Vec::new();
    for pair in pairs {
        if pair.as_rule() == Rule::file {
            for inner in pair.into_inner() {
                if inner.as_rule() == Rule::statement {
                    statements.push(walk_statement(inner)?);
                }
            }
        }
    }
    Ok(statements)
}

// ── Statement-level dispatch ─────────────────────────────────────────────────

fn walk_statement(pair: Pair<'_, Rule>) -> Result<ParsedStatement, ParseError> {
    let span = extract_source_span(&pair);
    let mut indent = 0;
    let mut body_pair = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::line_indent => indent = inner.as_str().len(),
            Rule::statement_body => body_pair = Some(inner),
            Rule::comment_tail => {} // ignored
            _ => {}
        }
    }

    // The pest implicit WHITESPACE rule (" " | "\t") can consume leading
    // spaces between NEWLINE repetitions in the file rule's NEWLINE+.
    // When this happens, line_indent captures 0 spaces even for indented
    // statements (e.g. after a blank line). Use the statement_body's column
    // position as the authoritative indent — it always reflects the true
    // position on the line regardless of implicit WHITESPACE consumption.
    if let Some(ref body) = body_pair {
        let body_col = body.as_span().start_pos().line_col().1;
        indent = body_col - 1;
    }

    let body = body_pair.ok_or_else(|| ParseError::MissingElement {
        pattern: "statement".into(),
        detail: "no statement_body found".into(),
        span,
    })?;

    let stmt_pair = body
        .into_inner()
        .next()
        .ok_or_else(|| ParseError::MissingElement {
            pattern: "statement_body".into(),
            detail: "empty statement_body".into(),
            span,
        })?;

    let mut parsed = match stmt_pair.as_rule() {
        Rule::define_stmt => walk_define_stmt(stmt_pair)?,
        Rule::describe_stmt => walk_describe_stmt(stmt_pair)?,
        Rule::error_stmt => walk_error_stmt(stmt_pair)?,
        Rule::promise_stmt => walk_promise_stmt(stmt_pair)?,
        Rule::let_stmt => walk_let_stmt(stmt_pair)?,
        Rule::check_stmt => walk_check_stmt(stmt_pair)?,
        Rule::for_each_stmt => walk_for_each_stmt(stmt_pair)?,
        Rule::match_stmt => walk_match_stmt(stmt_pair)?,
        Rule::fetch_stmt => walk_fetch_stmt(stmt_pair)?,
        Rule::save_stmt => walk_save_stmt(stmt_pair)?,
        Rule::update_stmt => walk_update_stmt(stmt_pair)?,
        Rule::remove_stmt => walk_remove_stmt(stmt_pair)?,
        Rule::return_stmt => walk_return_stmt(stmt_pair)?,
        Rule::raise_stmt => walk_raise_stmt(stmt_pair)?,
        Rule::together_stmt => walk_together_stmt(stmt_pair)?,
        Rule::retry_stmt => walk_retry_stmt(stmt_pair)?,
        Rule::do_stmt => walk_do_stmt(stmt_pair)?,
        other => {
            return Err(ParseError::UnknownPattern {
                rule_name: format!("{other:?}"),
                span,
            })
        }
    };

    parsed.indent = indent;
    parsed.span = span;
    Ok(parsed)
}

// ── Pattern walkers ──────────────────────────────────────────────────────────

fn walk_define_stmt(pair: Pair<'_, Rule>) -> Result<ParsedStatement, ParseError> {
    let span = extract_source_span(&pair);
    let mut name = String::new();
    let mut base = String::new();
    let mut expr = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::type_ref => name = inner.as_str().to_string(),
            Rule::base_type => base = inner.as_str().to_string(),
            Rule::raw_expr => expr = Some(Expression(inner.as_str().trim().to_string())),
            _ => {}
        }
    }

    Ok(ParsedStatement {
        indent: 0,
        pattern: Pattern::Define,
        intent: format!("define {name}"),
        metadata: NodeMetadata {
            name: Some(name),
            base_type: Some(base),
            ..Default::default()
        },
        expression: expr,
        contracts: vec![],
        span,
        inline_children: vec![],
    })
}

fn walk_describe_stmt(pair: Pair<'_, Rule>) -> Result<ParsedStatement, ParseError> {
    let span = extract_source_span(&pair);
    let mut name = String::new();
    let mut fields = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::type_ref => name = inner.as_str().to_string(),
            Rule::field_list => fields = walk_field_list(inner),
            _ => {}
        }
    }

    Ok(ParsedStatement {
        indent: 0,
        pattern: Pattern::Describe,
        intent: format!("describe {name}"),
        metadata: NodeMetadata {
            name: Some(name),
            fields,
            ..Default::default()
        },
        expression: None,
        contracts: vec![],
        span,
        inline_children: vec![],
    })
}

fn walk_error_stmt(pair: Pair<'_, Rule>) -> Result<ParsedStatement, ParseError> {
    let span = extract_source_span(&pair);
    let mut name = String::new();
    let mut carries = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::type_ref => name = inner.as_str().to_string(),
            Rule::field_list => carries = walk_field_list(inner),
            Rule::raw_expr => {} // document-style — ignored for now
            _ => {}
        }
    }

    Ok(ParsedStatement {
        indent: 0,
        pattern: Pattern::Error,
        intent: format!("error {name}"),
        metadata: NodeMetadata {
            name: Some(name),
            carries,
            ..Default::default()
        },
        expression: None,
        contracts: vec![],
        span,
        inline_children: vec![],
    })
}

fn walk_promise_stmt(pair: Pair<'_, Rule>) -> Result<ParsedStatement, ParseError> {
    let span = extract_source_span(&pair);
    let mut kind = ContractKind::Before;
    let mut expr_text = String::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::promise_before => kind = ContractKind::Before,
            Rule::promise_after => kind = ContractKind::After,
            Rule::promise_always => kind = ContractKind::Always,
            Rule::raw_expr => expr_text = inner.as_str().trim().to_string(),
            _ => {}
        }
    }

    Ok(ParsedStatement {
        indent: 0,
        pattern: Pattern::Promise,
        intent: expr_text.clone(),
        metadata: NodeMetadata::default(),
        expression: None,
        contracts: vec![Contract {
            kind,
            expression: Expression(expr_text),
        }],
        span,
        inline_children: vec![],
    })
}

fn walk_let_stmt(pair: Pair<'_, Rule>) -> Result<ParsedStatement, ParseError> {
    let span = extract_source_span(&pair);
    let mut name = String::new();
    let mut type_ref = String::new();
    let mut exprs: Vec<String> = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::typed_param => {
                let (n, t) = walk_typed_param(inner);
                name = n;
                type_ref = t;
            }
            Rule::raw_expr => exprs.push(inner.as_str().trim().to_string()),
            _ => {}
        }
    }

    // For "let x:T = expr" form, name/type_ref are set and exprs has 1 entry.
    // For "calculate expr as expr" form, name is empty and exprs has 2 entries.
    let expression = exprs.into_iter().last().map(Expression);

    Ok(ParsedStatement {
        indent: 0,
        pattern: Pattern::Let,
        intent: if name.is_empty() {
            "let binding".to_string()
        } else {
            format!("let {name}")
        },
        metadata: NodeMetadata {
            name: if name.is_empty() { None } else { Some(name) },
            return_type: if type_ref.is_empty() {
                None
            } else {
                Some(type_ref)
            },
            ..Default::default()
        },
        expression,
        contracts: vec![],
        span,
        inline_children: vec![],
    })
}

fn walk_check_stmt(pair: Pair<'_, Rule>) -> Result<ParsedStatement, ParseError> {
    let span = extract_source_span(&pair);
    let mut condition = String::new();
    let mut otherwise_error = None;
    let mut otherwise_assigns = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::check_cond_expr => {
                condition = inner.as_str().trim().to_string();
            }
            Rule::otherwise_clause => {
                for oc_inner in inner.into_inner() {
                    match oc_inner.as_rule() {
                        Rule::type_ref => {
                            otherwise_error = Some(oc_inner.as_str().to_string());
                        }
                        Rule::assignment_list => {
                            otherwise_assigns = walk_assignment_list(oc_inner);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    Ok(ParsedStatement {
        indent: 0,
        pattern: Pattern::Check,
        intent: format!("check {condition}"),
        metadata: NodeMetadata {
            otherwise_error,
            otherwise_assigns,
            ..Default::default()
        },
        expression: Some(Expression(condition)),
        contracts: vec![],
        span,
        inline_children: vec![],
    })
}

fn walk_for_each_stmt(pair: Pair<'_, Rule>) -> Result<ParsedStatement, ParseError> {
    let span = extract_source_span(&pair);
    let mut var_name = String::new();
    let mut var_type = String::new();
    let mut collection = String::new();
    let mut do_intent = String::new();
    let mut seen_param = false;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::typed_param if !seen_param => {
                let (n, t) = walk_typed_param(inner);
                var_name = n;
                var_type = t;
                seen_param = true;
            }
            Rule::ident if !seen_param => {
                var_name = inner.as_str().to_string();
                seen_param = true;
            }
            Rule::ident if seen_param => {
                collection = inner.as_str().to_string();
            }
            Rule::raw_expr => {
                collection = inner.as_str().trim().to_string();
            }
            Rule::intent => {
                do_intent = inner.as_str().to_string();
            }
            _ => {}
        }
    }

    Ok(ParsedStatement {
        indent: 0,
        pattern: Pattern::ForEach,
        intent: format!("for each {var_name} in {collection}"),
        metadata: NodeMetadata {
            name: Some(var_name.clone()),
            params: vec![Param {
                name: var_name,
                type_ref: var_type,
            }],
            ..Default::default()
        },
        expression: if do_intent.is_empty() {
            if collection.is_empty() {
                None
            } else {
                Some(Expression(collection))
            }
        } else {
            Some(Expression(do_intent))
        },
        contracts: vec![],
        span,
        inline_children: vec![],
    })
}

fn walk_match_stmt(pair: Pair<'_, Rule>) -> Result<ParsedStatement, ParseError> {
    let span = extract_source_span(&pair);
    let mut discriminant = String::new();
    let mut when_clauses = Vec::new();
    let mut otherwise_expr = None;
    let mut last_was_otherwise = false;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::raw_expr => {
                let text = inner.as_str().trim().to_string();
                if last_was_otherwise {
                    otherwise_expr = Some(text);
                    last_was_otherwise = false;
                } else if discriminant.is_empty() {
                    discriminant = text;
                } else {
                    // Otherwise default: "otherwise: <raw_expr>"
                    otherwise_expr = Some(text);
                }
            }
            Rule::when_clause => {
                let mut val = String::new();
                let mut then = String::new();
                for wc in inner.into_inner() {
                    match wc.as_rule() {
                        Rule::when_val_expr => val = wc.as_str().trim().to_string(),
                        Rule::raw_expr => then = wc.as_str().trim().to_string(),
                        _ => {}
                    }
                }
                when_clauses.push((val, then));
            }
            _ => {
                // Track "otherwise" keyword for the following raw_expr
                let s = inner.as_str();
                if s == "otherwise" {
                    last_was_otherwise = true;
                }
            }
        }
    }

    // Encode match as expression text: "discriminant | val1: then1 | val2: then2 [| otherwise: default]"
    let mut expr_parts = vec![discriminant.clone()];
    for (val, then) in &when_clauses {
        expr_parts.push(format!("{val}: {then}"));
    }
    if let Some(ref ow) = otherwise_expr {
        expr_parts.push(format!("otherwise: {ow}"));
    }

    Ok(ParsedStatement {
        indent: 0,
        pattern: Pattern::Match,
        intent: format!("match {discriminant}"),
        metadata: NodeMetadata::default(),
        expression: Some(Expression(expr_parts.join(" | "))),
        contracts: vec![],
        span,
        inline_children: vec![],
    })
}

fn walk_fetch_stmt(pair: Pair<'_, Rule>) -> Result<ParsedStatement, ParseError> {
    let span = extract_source_span(&pair);
    let mut var_name = String::new();
    let mut var_type = String::new();
    let mut source = String::new();
    let mut where_expr = None;
    let mut seen_first = false;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::typed_param if !seen_first => {
                let (n, t) = walk_typed_param(inner);
                var_name = n;
                var_type = t;
                seen_first = true;
            }
            Rule::ident if !seen_first => {
                var_name = inner.as_str().to_string();
                seen_first = true;
            }
            Rule::ident if seen_first => {
                source = inner.as_str().to_string();
            }
            Rule::raw_expr => {
                where_expr = Some(inner.as_str().trim().to_string());
            }
            _ => {}
        }
    }

    let expr = match where_expr {
        Some(w) => Some(Expression(format!("from {source} where {w}"))),
        None => Some(Expression(format!("from {source}"))),
    };

    Ok(ParsedStatement {
        indent: 0,
        pattern: Pattern::Fetch,
        intent: format!("fetch {var_name}"),
        metadata: NodeMetadata {
            name: Some(var_name),
            return_type: if var_type.is_empty() {
                None
            } else {
                Some(var_type)
            },
            ..Default::default()
        },
        expression: expr,
        contracts: vec![],
        span,
        inline_children: vec![],
    })
}

fn walk_save_stmt(pair: Pair<'_, Rule>) -> Result<ParsedStatement, ParseError> {
    let span = extract_source_span(&pair);
    let mut entity = String::new();
    let mut dest = String::new();
    let mut assigns = Vec::new();
    let mut seen_entity = false;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::ident if !seen_entity => {
                entity = inner.as_str().to_string();
                seen_entity = true;
            }
            Rule::ident if seen_entity => {
                dest = inner.as_str().to_string();
            }
            Rule::assignment_list => {
                assigns = walk_assignment_list(inner);
            }
            _ => {}
        }
    }

    let expr_text = if assigns.is_empty() {
        format!("to {dest}")
    } else {
        let a: Vec<String> = assigns.iter().map(|(k, v)| format!("{k} = {v}")).collect();
        format!("to {dest} with {}", a.join(", "))
    };

    Ok(ParsedStatement {
        indent: 0,
        pattern: Pattern::Save,
        intent: format!("save {entity}"),
        metadata: NodeMetadata {
            name: Some(entity),
            ..Default::default()
        },
        expression: Some(Expression(expr_text)),
        contracts: vec![],
        span,
        inline_children: vec![],
    })
}

fn walk_update_stmt(pair: Pair<'_, Rule>) -> Result<ParsedStatement, ParseError> {
    let span = extract_source_span(&pair);
    let mut type_name = String::new();
    let mut source = String::new();
    let mut where_expr = None;
    let mut set_assigns = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::type_ref => type_name = inner.as_str().to_string(),
            Rule::ident => source = inner.as_str().to_string(),
            Rule::where_expr_update => {
                where_expr = Some(inner.as_str().trim().to_string());
            }
            Rule::assignment_list => {
                set_assigns = walk_assignment_list(inner);
            }
            _ => {}
        }
    }

    let mut parts = vec![format!("in {source}")];
    if let Some(ref w) = where_expr {
        parts.push(format!("where {w}"));
    }
    if !set_assigns.is_empty() {
        let a: Vec<String> = set_assigns
            .iter()
            .map(|(k, v)| format!("{k} = {v}"))
            .collect();
        parts.push(format!("set {}", a.join(", ")));
    }

    Ok(ParsedStatement {
        indent: 0,
        pattern: Pattern::Update,
        intent: format!("update {type_name}"),
        metadata: NodeMetadata {
            name: Some(type_name),
            ..Default::default()
        },
        expression: Some(Expression(parts.join(" "))),
        contracts: vec![],
        span,
        inline_children: vec![],
    })
}

fn walk_remove_stmt(pair: Pair<'_, Rule>) -> Result<ParsedStatement, ParseError> {
    let span = extract_source_span(&pair);
    let mut type_name = String::new();
    let mut source = String::new();
    let mut where_expr = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::type_ref => type_name = inner.as_str().to_string(),
            Rule::ident => source = inner.as_str().to_string(),
            Rule::raw_expr => where_expr = Some(inner.as_str().trim().to_string()),
            _ => {}
        }
    }

    let expr = match where_expr {
        Some(w) => format!("from {source} where {w}"),
        None => format!("from {source}"),
    };

    Ok(ParsedStatement {
        indent: 0,
        pattern: Pattern::Remove,
        intent: format!("remove {type_name}"),
        metadata: NodeMetadata {
            name: Some(type_name),
            ..Default::default()
        },
        expression: Some(Expression(expr)),
        contracts: vec![],
        span,
        inline_children: vec![],
    })
}

fn walk_return_stmt(pair: Pair<'_, Rule>) -> Result<ParsedStatement, ParseError> {
    let span = extract_source_span(&pair);
    let mut type_name = String::new();
    let mut assigns = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::type_ref => type_name = inner.as_str().to_string(),
            Rule::raw_expr => {
                // "respond with" document form
                type_name = inner.as_str().trim().to_string();
            }
            Rule::assignment_list => assigns = walk_assignment_list(inner),
            _ => {}
        }
    }

    let expr = if assigns.is_empty() {
        None
    } else {
        let a: Vec<String> = assigns.iter().map(|(k, v)| format!("{k} = {v}")).collect();
        Some(Expression(format!("with {}", a.join(", "))))
    };

    Ok(ParsedStatement {
        indent: 0,
        pattern: Pattern::Return,
        intent: format!("return {type_name}"),
        metadata: NodeMetadata {
            name: Some(type_name.clone()),
            return_type: Some(type_name),
            ..Default::default()
        },
        expression: expr,
        contracts: vec![],
        span,
        inline_children: vec![],
    })
}

fn walk_raise_stmt(pair: Pair<'_, Rule>) -> Result<ParsedStatement, ParseError> {
    let span = extract_source_span(&pair);
    let mut type_name = String::new();
    let mut assigns = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::type_ref => type_name = inner.as_str().to_string(),
            Rule::assignment_list => assigns = walk_assignment_list(inner),
            _ => {}
        }
    }

    let carries: Vec<Field> = assigns
        .iter()
        .map(|(k, v)| Field {
            name: k.clone(),
            type_ref: v.clone(),
        })
        .collect();

    Ok(ParsedStatement {
        indent: 0,
        pattern: Pattern::Raise,
        intent: format!("raise {type_name}"),
        metadata: NodeMetadata {
            name: Some(type_name),
            carries,
            ..Default::default()
        },
        expression: None,
        contracts: vec![],
        span,
        inline_children: vec![],
    })
}

fn walk_together_stmt(pair: Pair<'_, Rule>) -> Result<ParsedStatement, ParseError> {
    let span = extract_source_span(&pair);
    let mut children = Vec::new();

    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::statement_body {
            let child = walk_inline_statement_body(inner)?;
            children.push(child);
        }
    }

    Ok(ParsedStatement {
        indent: 0,
        pattern: Pattern::Together,
        intent: "together".to_string(),
        metadata: NodeMetadata::default(),
        expression: None,
        contracts: vec![],
        span,
        inline_children: children,
    })
}

fn walk_retry_stmt(pair: Pair<'_, Rule>) -> Result<ParsedStatement, ParseError> {
    let span = extract_source_span(&pair);
    let mut count = 0u32;
    let mut delay_value = None;
    let mut delay_unit = None;
    let mut children = Vec::new();
    let mut seen_count = false;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::integer_lit => {
                let val: u32 = inner.as_str().parse().unwrap_or(0);
                if !seen_count {
                    count = val;
                    seen_count = true;
                } else {
                    delay_value = Some(val);
                }
            }
            Rule::time_unit => {
                delay_unit = Some(inner.as_str().to_string());
            }
            Rule::statement_body => {
                let child = walk_inline_statement_body(inner)?;
                children.push(child);
            }
            _ => {}
        }
    }

    let expr = match (delay_value, delay_unit) {
        (Some(v), Some(u)) => Some(Expression(format!("{count} times with delay {v} {u}"))),
        _ => Some(Expression(format!("{count} times"))),
    };

    Ok(ParsedStatement {
        indent: 0,
        pattern: Pattern::Retry,
        intent: format!("retry {count} times"),
        metadata: NodeMetadata::default(),
        expression: expr,
        contracts: vec![],
        span,
        inline_children: children,
    })
}

fn walk_do_stmt(pair: Pair<'_, Rule>) -> Result<ParsedStatement, ParseError> {
    let span = extract_source_span(&pair);
    let mut intent_text = String::new();
    let mut params = Vec::new();
    let mut return_types = Vec::new();
    let mut or_error_types = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::intent => {
                intent_text = inner.as_str().to_string();
            }
            Rule::param_list => {
                params = walk_param_list(inner);
            }
            Rule::return_clause => {
                for rc_inner in inner.into_inner() {
                    if rc_inner.as_rule() == Rule::ret_type_list {
                        for t in rc_inner.into_inner() {
                            if t.as_rule() == Rule::type_ref {
                                return_types.push(t.as_str().to_string());
                            }
                        }
                    }
                }
            }
            Rule::type_ref => {
                // "or ErrorType" continuation lines
                or_error_types.push(inner.as_str().to_string());
            }
            _ => {}
        }
    }

    let primary_return = return_types.first().cloned();
    // Combine all error types from return_clause "or" and standalone "or" lines
    let mut all_error_types: Vec<String> = return_types.into_iter().skip(1).collect();
    all_error_types.extend(or_error_types);

    let return_type = match (primary_return.as_ref(), all_error_types.is_empty()) {
        (Some(rt), true) => Some(rt.clone()),
        (Some(rt), false) => Some(format!("{rt} or {}", all_error_types.join(" or "))),
        _ => None,
    };

    Ok(ParsedStatement {
        indent: 0,
        pattern: Pattern::Do,
        intent: intent_text.clone(),
        metadata: NodeMetadata {
            name: Some(intent_text.replace(' ', "_")),
            params,
            return_type,
            ..Default::default()
        },
        expression: None,
        contracts: vec![],
        span,
        inline_children: vec![],
    })
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Walk an inline `statement_body` (used by together/retry for their children).
fn walk_inline_statement_body(pair: Pair<'_, Rule>) -> Result<ParsedStatement, ParseError> {
    let span = extract_source_span(&pair);
    let stmt_pair = pair
        .into_inner()
        .next()
        .ok_or_else(|| ParseError::MissingElement {
            pattern: "inline_statement_body".into(),
            detail: "empty statement_body".into(),
            span,
        })?;

    let mut parsed = match stmt_pair.as_rule() {
        Rule::define_stmt => walk_define_stmt(stmt_pair)?,
        Rule::describe_stmt => walk_describe_stmt(stmt_pair)?,
        Rule::error_stmt => walk_error_stmt(stmt_pair)?,
        Rule::promise_stmt => walk_promise_stmt(stmt_pair)?,
        Rule::let_stmt => walk_let_stmt(stmt_pair)?,
        Rule::check_stmt => walk_check_stmt(stmt_pair)?,
        Rule::for_each_stmt => walk_for_each_stmt(stmt_pair)?,
        Rule::match_stmt => walk_match_stmt(stmt_pair)?,
        Rule::fetch_stmt => walk_fetch_stmt(stmt_pair)?,
        Rule::save_stmt => walk_save_stmt(stmt_pair)?,
        Rule::update_stmt => walk_update_stmt(stmt_pair)?,
        Rule::remove_stmt => walk_remove_stmt(stmt_pair)?,
        Rule::return_stmt => walk_return_stmt(stmt_pair)?,
        Rule::raise_stmt => walk_raise_stmt(stmt_pair)?,
        Rule::together_stmt => walk_together_stmt(stmt_pair)?,
        Rule::retry_stmt => walk_retry_stmt(stmt_pair)?,
        Rule::do_stmt => walk_do_stmt(stmt_pair)?,
        other => {
            return Err(ParseError::UnknownPattern {
                rule_name: format!("{other:?}"),
                span,
            })
        }
    };
    parsed.span = span;
    Ok(parsed)
}

fn extract_source_span(pair: &Pair<'_, Rule>) -> SourceSpan {
    SourceSpan::from_pest_span(pair.as_span())
}

fn walk_typed_param(pair: Pair<'_, Rule>) -> (String, String) {
    let mut name = String::new();
    let mut type_ref = String::new();
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::ident => name = inner.as_str().to_string(),
            Rule::type_ref => type_ref = inner.as_str().to_string(),
            _ => {}
        }
    }
    (name, type_ref)
}

fn walk_field_list(pair: Pair<'_, Rule>) -> Vec<Field> {
    pair.into_inner()
        .filter(|p| p.as_rule() == Rule::typed_field)
        .map(|tf| {
            let mut name = String::new();
            let mut type_ref = String::new();
            for inner in tf.into_inner() {
                match inner.as_rule() {
                    Rule::ident => name = inner.as_str().to_string(),
                    Rule::type_ref => type_ref = inner.as_str().to_string(),
                    _ => {}
                }
            }
            Field { name, type_ref }
        })
        .collect()
}

fn walk_param_list(pair: Pair<'_, Rule>) -> Vec<Param> {
    pair.into_inner()
        .filter(|p| p.as_rule() == Rule::typed_param)
        .map(|tp| {
            let (name, type_ref) = walk_typed_param(tp);
            Param { name, type_ref }
        })
        .collect()
}

fn walk_assignment_list(pair: Pair<'_, Rule>) -> Vec<(String, String)> {
    pair.into_inner()
        .filter(|p| p.as_rule() == Rule::assignment)
        .map(|a| {
            let mut name = String::new();
            let mut value = String::new();
            for inner in a.into_inner() {
                match inner.as_rule() {
                    Rule::ident => name = inner.as_str().to_string(),
                    Rule::value_expr => value = inner.as_str().trim().to_string(),
                    _ => {}
                }
            }
            (name, value)
        })
        .collect()
}
