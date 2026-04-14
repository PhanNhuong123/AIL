//! Parse raw expression strings from AIL node metadata into structured parts.
//!
//! All inputs are strings produced by the ail-text parser and stored verbatim
//! in `Node::expression`. No external dependencies — all parsing is string-based.

/// Parse `"from {source} where {key is val, ...}"` from a Fetch or Remove node.
///
/// Returns `(source, conditions)` where `conditions` is a vec of `(key, value)` pairs.
/// Both source and conditions may be empty if the expression is missing parts.
pub(crate) fn parse_fetch_expression(expr: &str) -> (Option<String>, Vec<(String, String)>) {
    let s = expr.trim();
    let without_from = match s.strip_prefix("from ") {
        Some(rest) => rest,
        None => return (None, vec![]),
    };

    if let Some(idx) = without_from.find(" where ") {
        let source = without_from[..idx].trim().to_owned();
        let cond_str = without_from[idx + " where ".len()..].trim();
        let conditions = parse_key_val_condition(cond_str);
        (Some(source), conditions)
    } else {
        (Some(without_from.trim().to_owned()), vec![])
    }
}

/// Parse `"in {source} where {cond} set {k=v,...}"` from an Update node.
///
/// Returns `(source, where_conditions, set_assignments)`.
#[allow(clippy::type_complexity)]
pub(crate) fn parse_update_expression(
    expr: &str,
) -> (Option<String>, Vec<(String, String)>, Vec<(String, String)>) {
    let s = expr.trim();
    let without_in = match s.strip_prefix("in ") {
        Some(rest) => rest,
        None => return (None, vec![], vec![]),
    };

    // Split on " where " first.
    let (source, rest_after_source) = if let Some(idx) = without_in.find(" where ") {
        let src = without_in[..idx].trim().to_owned();
        let rest = &without_in[idx + " where ".len()..];
        (Some(src), rest)
    } else if let Some(idx) = without_in.find(" set ") {
        let src = without_in[..idx].trim().to_owned();
        let rest = &without_in[idx..]; // keep " set ..." for next split
        (Some(src), rest)
    } else {
        return (Some(without_in.trim().to_owned()), vec![], vec![]);
    };

    // Split on " set ".
    let (where_str, set_str) = if let Some(idx) = rest_after_source.find(" set ") {
        let w = &rest_after_source[..idx];
        let s = &rest_after_source[idx + " set ".len()..];
        (w.trim(), s.trim())
    } else {
        (rest_after_source.trim(), "")
    };

    let where_conditions = if where_str.is_empty() {
        vec![]
    } else {
        parse_key_val_condition(where_str)
    };
    let set_assignments = if set_str.is_empty() {
        vec![]
    } else {
        parse_assignments(set_str)
    };

    (source, where_conditions, set_assignments)
}

/// Parse `"with {field = expr, ...}"` from a Return node.
///
/// Returns a vec of `(field_name, expression)` pairs.
/// Returns empty vec if expression is None or does not start with "with ".
pub(crate) fn parse_return_with_expression(expr: &str) -> Vec<(String, String)> {
    let s = expr.trim();
    let without_with = match s.strip_prefix("with ") {
        Some(rest) => rest,
        None => return vec![],
    };
    parse_assignments(without_with)
}

/// Parse `"for each {var}:{type} in {collection} [do {intent}]"` from a ForEach node.
///
/// Returns `(var_name, type_ref, collection, body_intent)`.
/// Falls back to empty strings for missing parts.
pub(crate) fn parse_foreach_expression(expr: &str) -> (String, String, String, Option<String>) {
    let s = expr.trim();
    let rest = match s.strip_prefix("for each ") {
        Some(r) => r,
        None => return (String::new(), String::new(), String::new(), None),
    };

    // Extract "var:type" before " in ".
    let (var_type_str, after_in) = if let Some(idx) = rest.find(" in ") {
        (&rest[..idx], &rest[idx + " in ".len()..])
    } else {
        return (rest.to_owned(), String::new(), String::new(), None);
    };

    // Split var:type on ':'.
    let (var_name, type_ref) = if let Some(idx) = var_type_str.find(':') {
        (
            var_type_str[..idx].trim().to_owned(),
            var_type_str[idx + 1..].trim().to_owned(),
        )
    } else {
        (var_type_str.trim().to_owned(), String::new())
    };

    // Check for " do {intent}" suffix.
    let (collection, body_intent) = if let Some(idx) = after_in.find(" do ") {
        let col = after_in[..idx].trim().to_owned();
        let intent = after_in[idx + " do ".len()..].trim().to_owned();
        (col, Some(intent))
    } else {
        (after_in.trim().to_owned(), None)
    };

    (var_name, type_ref, collection, body_intent)
}

/// Parse `"N times [with delay M unit]"` from a Retry node.
///
/// Returns `(count, Some((delay_value, unit)))` or `(count, None)` if no delay.
pub(crate) fn parse_retry_expression(expr: &str) -> (u32, Option<(f64, String)>) {
    let s = expr.trim();

    // Split on " with delay " to separate count from delay spec.
    let (count_str, delay_part) = if let Some(idx) = s.find(" with delay ") {
        (&s[..idx], Some(&s[idx + " with delay ".len()..]))
    } else {
        (s, None)
    };

    // Parse count: "N times" → N.
    let count = count_str
        .trim()
        .trim_end_matches(" times")
        .trim()
        .parse::<u32>()
        .unwrap_or(1);

    // Parse delay: "M unit" → (M, "unit").
    let delay = delay_part.and_then(|d| {
        let parts: Vec<&str> = d.trim().splitn(2, ' ').collect();
        if parts.len() == 2 {
            let val = parts[0].parse::<f64>().ok()?;
            // Normalize unit to singular canonical form. Use an explicit map so
            // short abbreviations like "ms" are not corrupted by trimming 's'.
            let unit = match parts[1] {
                "seconds" | "second" => "second",
                "minutes" | "minute" | "mins" | "min" => "minute",
                "milliseconds" | "millisecond" | "ms" => "millisecond",
                other => other,
            }
            .to_owned();
            Some((val, unit))
        } else if parts.len() == 1 {
            let val = parts[0].parse::<f64>().ok()?;
            Some((val, "second".to_owned()))
        } else {
            None
        }
    });

    (count, delay)
}

/// Convert `"key is value"` or `"key is not value"` into `[(key, value)]`.
///
/// Handles multiple conditions separated by " and ".
/// Unknown operators are treated as equality.
pub(crate) fn parse_key_val_condition(s: &str) -> Vec<(String, String)> {
    s.split(" and ")
        .filter_map(|clause| {
            let clause = clause.trim();
            if let Some(idx) = clause.find(" is not ") {
                // "is not" → equality check negated; keep raw for now (emitter handles semantics)
                let key = clause[..idx].trim().to_owned();
                let val = clause[idx + " is not ".len()..].trim().to_owned();
                Some((key, format!("not:{val}")))
            } else if let Some(idx) = clause.find(" is ") {
                let key = clause[..idx].trim().to_owned();
                let val = clause[idx + " is ".len()..].trim().to_owned();
                Some((key, val))
            } else {
                None
            }
        })
        .collect()
}

/// Parse `"k = v, k2 = v2"` into `[("k", "v"), ("k2", "v2")]`.
///
/// Splits on `", "` first; falls back to `,` with trimming.
pub(crate) fn parse_assignments(s: &str) -> Vec<(String, String)> {
    s.split(',')
        .filter_map(|pair| {
            let pair = pair.trim();
            let idx = pair.find('=')?;
            let key = pair[..idx].trim().to_owned();
            let val = pair[idx + 1..].trim().to_owned();
            if key.is_empty() {
                None
            } else {
                Some((key, val))
            }
        })
        .collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fetch_no_where_clause() {
        let (src, conds) = parse_fetch_expression("from database");
        assert_eq!(src, Some("database".to_owned()));
        assert!(conds.is_empty());
    }

    #[test]
    fn parse_fetch_with_where_clause() {
        let (src, conds) = parse_fetch_expression("from database where id is sender_id");
        assert_eq!(src, Some("database".to_owned()));
        assert_eq!(conds, vec![("id".to_owned(), "sender_id".to_owned())]);
    }

    #[test]
    fn parse_fetch_missing_from_prefix() {
        let (src, conds) = parse_fetch_expression("database where id is sender_id");
        assert!(src.is_none());
        assert!(conds.is_empty());
    }

    #[test]
    fn parse_foreach_basic_form() {
        let (var, ty, col, intent) =
            parse_foreach_expression("for each item:OrderItem in order.items");
        assert_eq!(var, "item");
        assert_eq!(ty, "OrderItem");
        assert_eq!(col, "order.items");
        assert!(intent.is_none());
    }

    #[test]
    fn parse_foreach_with_do_suffix() {
        let (var, ty, col, intent) =
            parse_foreach_expression("for each item:OrderItem in order.items do calculate total");
        assert_eq!(var, "item");
        assert_eq!(ty, "OrderItem");
        assert_eq!(col, "order.items");
        assert_eq!(intent, Some("calculate total".to_owned()));
    }

    #[test]
    fn parse_update_extracts_source_condition_set() {
        let (src, conds, sets) = parse_update_expression(
            "in database where id is sender.id set balance = new_sender_balance",
        );
        assert_eq!(src, Some("database".to_owned()));
        assert_eq!(conds, vec![("id".to_owned(), "sender.id".to_owned())]);
        assert_eq!(
            sets,
            vec![("balance".to_owned(), "new_sender_balance".to_owned())]
        );
    }

    #[test]
    fn parse_return_with_multiple_fields() {
        let fields = parse_return_with_expression(
            "with sender = sender, receiver = receiver, amount = amount",
        );
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0], ("sender".to_owned(), "sender".to_owned()));
        assert_eq!(fields[1], ("receiver".to_owned(), "receiver".to_owned()));
        assert_eq!(fields[2], ("amount".to_owned(), "amount".to_owned()));
    }

    #[test]
    fn parse_return_with_empty_expr() {
        let fields = parse_return_with_expression("");
        assert!(fields.is_empty());
    }

    #[test]
    fn parse_retry_count_only() {
        let (count, delay) = parse_retry_expression("3 times");
        assert_eq!(count, 3);
        assert!(delay.is_none());
    }

    #[test]
    fn parse_retry_count_with_delay() {
        let (count, delay) = parse_retry_expression("3 times with delay 1 second");
        assert_eq!(count, 3);
        assert!(delay.is_some());
        let (val, unit) = delay.unwrap();
        assert!((val - 1.0).abs() < f64::EPSILON);
        assert_eq!(unit, "second");
    }

    #[test]
    fn parse_retry_plural_unit_normalized() {
        let (count, delay) = parse_retry_expression("5 times with delay 2 seconds");
        assert_eq!(count, 5);
        let (val, unit) = delay.unwrap();
        assert!((val - 2.0).abs() < f64::EPSILON);
        assert_eq!(unit, "second");
    }

    #[test]
    fn parse_key_val_condition_is_not() {
        let conds = parse_key_val_condition("sender_id is not receiver_id");
        assert_eq!(conds.len(), 1);
        assert_eq!(conds[0].0, "sender_id");
        assert_eq!(conds[0].1, "not:receiver_id");
    }

    #[test]
    fn parse_assignments_simple() {
        let pairs = parse_assignments("balance = new_balance, status = active");
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], ("balance".to_owned(), "new_balance".to_owned()));
        assert_eq!(pairs[1], ("status".to_owned(), "active".to_owned()));
    }
}
