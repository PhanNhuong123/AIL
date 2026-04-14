use std::fmt::Write;

use ail_graph::{ContractKind, Node, Pattern};

/// Render the text representation of a single node (its own line(s) only,
/// no children or contracts). Returns one or more lines without trailing newline.
///
/// For patterns where the walker stores structured data in expression text
/// (Fetch, Save, Update, Remove), the renderer concatenates `{keyword} {name} {expr}`.
/// This relies on the walker building expression text with keywords included
/// (e.g., `"from database where id is sender_id"`).
pub(crate) fn render_node_text(node: &Node) -> String {
    match node.pattern {
        Pattern::Define => render_define(node),
        Pattern::Describe => render_describe(node),
        Pattern::Error => render_error(node),
        Pattern::Do => render_do(node),
        Pattern::Promise => render_promise(node),
        Pattern::Let => render_let(node),
        Pattern::Check => render_check(node),
        Pattern::ForEach => render_for_each(node),
        Pattern::Match => render_match(node),
        Pattern::Fetch => render_fetch(node),
        Pattern::Save => render_save(node),
        Pattern::Update => render_update(node),
        Pattern::Remove => render_remove(node),
        Pattern::Return => render_return(node),
        Pattern::Raise => render_raise(node),
        Pattern::Together => "together".to_string(),
        Pattern::Retry => render_retry(node),
    }
}

// ── Type-defining patterns ──────────────────────────────────────────────────

fn render_define(node: &Node) -> String {
    let name = node.metadata.name.as_deref().unwrap_or("Unknown");
    let base = node.metadata.base_type.as_deref().unwrap_or("unknown");
    match &node.expression {
        Some(expr) => format!("define {name}:{base} where {}", expr.0),
        None => format!("define {name}:{base}"),
    }
}

fn render_describe(node: &Node) -> String {
    let name = match node.metadata.name.as_deref() {
        Some(n) => n,
        None => return String::new(), // directory container — caller skips
    };
    let fields = render_field_list(&node.metadata.fields);
    format!("describe {name} as\n  {fields}")
}

fn render_error(node: &Node) -> String {
    let name = node.metadata.name.as_deref().unwrap_or("UnknownError");
    if node.metadata.carries.is_empty() {
        format!("error {name}")
    } else {
        let carries = render_field_list(&node.metadata.carries);
        format!("error {name}\n  carries {carries}")
    }
}

// ── Structural patterns ─────────────────────────────────────────────────────

fn render_do(node: &Node) -> String {
    let mut out = format!("do {}", node.intent);

    if !node.metadata.params.is_empty() {
        let params: Vec<String> = node
            .metadata
            .params
            .iter()
            .map(|p| format!("{}:{}", p.name, p.type_ref))
            .collect();
        let _ = write!(out, "\n  from {}", params.join(", "));
    }

    if let Some(ref rt) = node.metadata.return_type {
        let parts: Vec<&str> = rt.splitn(2, " or ").collect();
        let _ = write!(out, "\n  -> {}", parts[0]);
        if parts.len() > 1 {
            // Remaining error types on separate "or" continuation lines
            for err_type in parts[1].split(" or ") {
                let _ = write!(out, "\n  or {}", err_type.trim());
            }
        }
    }

    // Render `following` clause if present.
    if let Some(ref tmpl) = node.metadata.following_template_name {
        let _ = write!(out, "\n  following {tmpl}");
    }

    // Render `using` clause if present.
    if let Some(ref pattern_name) = node.metadata.using_pattern_name {
        if node.metadata.using_params.is_empty() {
            let _ = write!(out, "\n  using {pattern_name}");
        } else {
            let params: Vec<String> = node
                .metadata
                .using_params
                .iter()
                .map(|(k, v)| format!("{k} is {v}"))
                .collect();
            let _ = write!(
                out,
                "\n  using {pattern_name}\n    where {}",
                params.join(", ")
            );
        }
    }

    out
}

fn render_promise(node: &Node) -> String {
    // Promise nodes should not exist as separate graph nodes — contracts are
    // merged into their parent Do node. This handles the edge case of a
    // standalone Promise node that somehow survived assembly.
    if let Some(contract) = node.contracts.first() {
        let kind = match contract.kind {
            ContractKind::Before => "before",
            ContractKind::After => "after",
            ContractKind::Always => "always",
        };
        format!("promise {kind}: {}", contract.expression.0)
    } else {
        format!("promise: {}", node.intent)
    }
}

// ── Action patterns ─────────────────────────────────────────────────────────

fn render_let(node: &Node) -> String {
    let name = node.metadata.name.as_deref().unwrap_or("x");
    let type_ref = node.metadata.return_type.as_deref().unwrap_or("Unknown");
    match &node.expression {
        Some(expr) => format!("let {name}:{type_ref} = {}", expr.0),
        None => format!("let {name}:{type_ref}"),
    }
}

fn render_check(node: &Node) -> String {
    let condition = node.expression.as_ref().map(|e| e.0.as_str()).unwrap_or("");
    let mut out = format!("check {condition}");

    if let Some(ref err) = node.metadata.otherwise_error {
        let _ = write!(out, "\n  otherwise raise {err}");
        if !node.metadata.otherwise_assigns.is_empty() {
            let assigns: Vec<String> = node
                .metadata
                .otherwise_assigns
                .iter()
                .map(|(k, v)| format!("{k} = {v}"))
                .collect();
            let _ = write!(out, " carries {}", assigns.join(", "));
        }
    }

    out
}

fn render_for_each(node: &Node) -> String {
    let var_name = node.metadata.name.as_deref().unwrap_or("item");
    let var_type = node.metadata.params.first().map(|p| p.type_ref.as_str());
    let collection = node.metadata.collection.as_deref().unwrap_or("");

    let var_part = match var_type {
        Some(t) if !t.is_empty() => format!("{var_name}:{t}"),
        _ => var_name.to_string(),
    };

    let mut out = format!("for each {var_part} in {collection}");

    if let Some(ref intent) = node.metadata.body_intent {
        let _ = write!(out, " do {intent}");
    }

    out
}

fn render_match(node: &Node) -> String {
    let disc = node.metadata.discriminant.as_deref().unwrap_or("");
    let mut out = format!("match {disc}");

    for (val, then) in &node.metadata.arms {
        let _ = write!(out, "\n  when {val}: {then}");
    }

    if let Some(ref ow) = node.metadata.otherwise_result {
        let _ = write!(out, "\n  otherwise: {ow}");
    }

    out
}

fn render_fetch(node: &Node) -> String {
    let name = node.metadata.name.as_deref().unwrap_or("x");
    let type_part = match node.metadata.return_type.as_deref() {
        Some(t) if !t.is_empty() => format!("{name}:{t}"),
        _ => name.to_string(),
    };
    match &node.expression {
        Some(expr) => format!("fetch {type_part} {}", expr.0),
        None => format!("fetch {type_part}"),
    }
}

fn render_save(node: &Node) -> String {
    let name = node.metadata.name.as_deref().unwrap_or("entity");
    match &node.expression {
        Some(expr) => format!("save {name} {}", expr.0),
        None => format!("save {name}"),
    }
}

fn render_update(node: &Node) -> String {
    let name = node.metadata.name.as_deref().unwrap_or("Unknown");
    match &node.expression {
        Some(expr) => format!("update {name} {}", expr.0),
        None => format!("update {name}"),
    }
}

fn render_remove(node: &Node) -> String {
    let name = node.metadata.name.as_deref().unwrap_or("Unknown");
    match &node.expression {
        Some(expr) => format!("remove {name} {}", expr.0),
        None => format!("remove {name}"),
    }
}

fn render_return(node: &Node) -> String {
    let type_name = node.metadata.name.as_deref().unwrap_or("Unknown");
    match &node.expression {
        Some(expr) => format!("return {type_name}\n  {}", expr.0),
        None => format!("return {type_name}"),
    }
}

fn render_raise(node: &Node) -> String {
    let name = node.metadata.name.as_deref().unwrap_or("UnknownError");
    if node.metadata.carries.is_empty() {
        format!("raise {name}")
    } else {
        // Raise carries stores (field_name, value) in Field { name, type_ref }.
        // The walker maps assignment key→name, value→type_ref.
        let assigns: Vec<String> = node
            .metadata
            .carries
            .iter()
            .map(|c| format!("{} = {}", c.name, c.type_ref))
            .collect();
        format!("raise {name}\n  carries {}", assigns.join(", "))
    }
}

fn render_retry(node: &Node) -> String {
    match &node.expression {
        Some(expr) => format!("retry {}", expr.0),
        None => "retry".to_string(),
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn render_field_list(fields: &[ail_graph::Field]) -> String {
    fields
        .iter()
        .map(|f| format!("{}:{}", f.name, f.type_ref))
        .collect::<Vec<_>>()
        .join(", ")
}
