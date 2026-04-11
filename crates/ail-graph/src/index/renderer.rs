use crate::types::ContractKind;

use super::entry::{IndexEntry, IndexKind};
use super::folder_index::FolderIndex;

/// Render a [`FolderIndex`] as `.index.ail` text using AIL syntax.
///
/// Sections (`-- types`, `-- errors`, `-- functions`) are omitted when empty.
/// Each section renders its entries in declaration order.
pub fn render_folder_index(index: &FolderIndex) -> String {
    let types: Vec<&IndexEntry> = index
        .entries
        .iter()
        .filter(|e| matches!(e.kind, IndexKind::Type { .. }))
        .collect();

    let errors: Vec<&IndexEntry> = index
        .entries
        .iter()
        .filter(|e| matches!(e.kind, IndexKind::ErrorType { .. }))
        .collect();

    let functions: Vec<&IndexEntry> = index
        .entries
        .iter()
        .filter(|e| matches!(e.kind, IndexKind::Function { .. }))
        .collect();

    let mut sections: Vec<String> = Vec::new();

    if !types.is_empty() {
        let mut lines = vec!["-- types".to_string()];
        for entry in &types {
            lines.push(render_type_entry(entry));
        }
        sections.push(lines.join("\n"));
    }

    if !errors.is_empty() {
        let mut lines = vec!["-- errors".to_string()];
        for entry in &errors {
            lines.push(render_error_entry(entry));
        }
        sections.push(lines.join("\n"));
    }

    if !functions.is_empty() {
        let mut lines = vec!["-- functions".to_string()];
        for entry in &functions {
            lines.push(render_function_entry(entry));
        }
        sections.push(lines.join("\n"));
    }

    sections.join("\n\n")
}

// ─── per-kind renderers ───────────────────────────────────────────────────────

fn render_type_entry(entry: &IndexEntry) -> String {
    match &entry.kind {
        IndexKind::Type {
            base_type,
            constraint_expr,
            fields,
        } => {
            if fields.is_empty() {
                // Define pattern: `define Name:BaseType where constraint`
                let mut line = format!("define {}", entry.name);
                if let Some(bt) = base_type {
                    line.push(':');
                    line.push_str(bt);
                }
                if let Some(constraint) = constraint_expr {
                    line.push_str(" where ");
                    line.push_str(constraint);
                }
                line
            } else {
                // Describe pattern: `describe Name as\n  field:Type, ...`
                let field_list = fields
                    .iter()
                    .map(|f| format!("{}:{}", f.name, f.type_ref))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("describe {} as\n  {}", entry.name, field_list)
            }
        }
        _ => unreachable!("render_type_entry called with non-Type entry"),
    }
}

fn render_error_entry(entry: &IndexEntry) -> String {
    match &entry.kind {
        IndexKind::ErrorType { carries } => {
            if carries.is_empty() {
                format!("error {}", entry.name)
            } else {
                let carries_list = carries
                    .iter()
                    .map(|f| format!("{}:{}", f.name, f.type_ref))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("error {} carries {}", entry.name, carries_list)
            }
        }
        _ => unreachable!("render_error_entry called with non-ErrorType entry"),
    }
}

fn render_function_entry(entry: &IndexEntry) -> String {
    match &entry.kind {
        IndexKind::Function {
            params,
            return_type,
            contracts,
        } => {
            let mut lines = vec![format!("do {}", entry.name)];

            if !params.is_empty() {
                let param_list = params
                    .iter()
                    .map(|p| format!("{}:{}", p.name, p.type_ref))
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(format!("  from {}", param_list));
            }

            if let Some(rt) = return_type {
                lines.push(format!("  -> {}", rt));
            }

            for contract in contracts {
                let prefix = match contract.kind {
                    ContractKind::Before => "  requires that",
                    ContractKind::After => "  guarantees that",
                    ContractKind::Always => "  always ensures",
                };
                lines.push(format!("{} {}", prefix, contract.expression));
            }

            lines.join("\n")
        }
        _ => unreachable!("render_function_entry called with non-Function entry"),
    }
}

// ─── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::entry::{ContractSummary, IndexKind};
    use crate::index::folder_index::FolderIndex;
    use crate::types::{ContractKind, Field, NodeId, Param};

    fn field(name: &str, type_ref: &str) -> Field {
        Field {
            name: name.to_string(),
            type_ref: type_ref.to_string(),
        }
    }

    fn make_index(entries: Vec<IndexEntry>) -> FolderIndex {
        FolderIndex {
            folder_name: "wallet".to_string(),
            folder_node_id: NodeId::new(),
            entries,
        }
    }

    fn type_entry(name: &str, base_type: &str, constraint: Option<&str>) -> IndexEntry {
        IndexEntry {
            name: name.to_string(),
            kind: IndexKind::Type {
                base_type: Some(base_type.to_string()),
                constraint_expr: constraint.map(str::to_string),
                fields: vec![],
            },
            node_id: NodeId::new(),
        }
    }

    fn describe_entry(name: &str, fields: Vec<Field>) -> IndexEntry {
        IndexEntry {
            name: name.to_string(),
            kind: IndexKind::Type {
                base_type: None,
                constraint_expr: None,
                fields,
            },
            node_id: NodeId::new(),
        }
    }

    fn error_entry(name: &str, carries: Vec<Field>) -> IndexEntry {
        IndexEntry {
            name: name.to_string(),
            kind: IndexKind::ErrorType { carries },
            node_id: NodeId::new(),
        }
    }

    fn function_entry(
        name: &str,
        params: Vec<Param>,
        return_type: Option<&str>,
        contracts: Vec<ContractSummary>,
    ) -> IndexEntry {
        IndexEntry {
            name: name.to_string(),
            kind: IndexKind::Function {
                params,
                return_type: return_type.map(str::to_string),
                contracts,
            },
            node_id: NodeId::new(),
        }
    }

    #[test]
    fn index010_render_define_with_base_type_and_constraint() {
        let entry = type_entry("Amount", "number", Some("value >= 0"));
        let index = make_index(vec![entry]);
        let text = render_folder_index(&index);
        assert!(
            text.contains("define Amount:number where value >= 0"),
            "unexpected output:\n{text}"
        );
    }

    #[test]
    fn index010b_render_define_without_constraint() {
        let entry = type_entry("Amount", "number", None);
        let index = make_index(vec![entry]);
        let text = render_folder_index(&index);
        assert!(
            text.contains("define Amount:number"),
            "unexpected output:\n{text}"
        );
        assert!(!text.contains("where"), "should not have 'where' when no constraint");
    }

    #[test]
    fn index011_render_describe_entry_with_fields() {
        let entry = describe_entry("Invoice", vec![field("id", "text"), field("amount", "Amount")]);
        let index = make_index(vec![entry]);
        let text = render_folder_index(&index);
        assert!(
            text.contains("describe Invoice as"),
            "unexpected output:\n{text}"
        );
        assert!(
            text.contains("  id:text, amount:Amount"),
            "unexpected output:\n{text}"
        );
    }

    #[test]
    fn index012_render_error_with_carries() {
        let entry = error_entry("NotFound", vec![field("id", "text")]);
        let index = make_index(vec![entry]);
        let text = render_folder_index(&index);
        assert!(
            text.contains("error NotFound carries id:text"),
            "unexpected output:\n{text}"
        );
    }

    #[test]
    fn index012b_render_error_without_carries() {
        let entry = error_entry("Unauthorized", vec![]);
        let index = make_index(vec![entry]);
        let text = render_folder_index(&index);
        assert!(
            text.contains("error Unauthorized"),
            "unexpected output:\n{text}"
        );
        assert!(!text.contains("carries"), "empty carries should be omitted");
    }

    #[test]
    fn index013_render_function_with_contracts() {
        let entry = function_entry(
            "create",
            vec![Param {
                name: "x".to_string(),
                type_ref: "Amount".to_string(),
            }],
            Some("Invoice"),
            vec![ContractSummary {
                kind: ContractKind::Before,
                expression: "x > 0".to_string(),
            }],
        );
        let index = make_index(vec![entry]);
        let text = render_folder_index(&index);

        assert!(text.contains("do create"), "unexpected output:\n{text}");
        assert!(text.contains("  from x:Amount"), "unexpected output:\n{text}");
        assert!(text.contains("  -> Invoice"), "unexpected output:\n{text}");
        assert!(
            text.contains("  requires that x > 0"),
            "unexpected output:\n{text}"
        );
    }

    #[test]
    fn index013b_render_sections_appear_only_when_non_empty() {
        // Only an error entry — should have "-- errors" but not "-- types" or "-- functions"
        let entry = error_entry("NotFound", vec![]);
        let index = make_index(vec![entry]);
        let text = render_folder_index(&index);

        assert!(text.contains("-- errors"), "expected '-- errors' section");
        assert!(!text.contains("-- types"), "unexpected '-- types' section");
        assert!(!text.contains("-- functions"), "unexpected '-- functions' section");
    }
}
