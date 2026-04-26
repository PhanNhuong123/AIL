//! Text rendering for `ail sheaf` output.

use ail_contract::CechNerve;

use super::short_id;

pub(super) fn render_text(
    nerve: &CechNerve,
    scope_kind: &str,
    _scope_root_id: Option<&str>,
    scope_root_query: Option<&str>,
    #[cfg(feature = "z3-verify")] obstructions: &[ail_contract::ObstructionResult],
) -> String {
    let mut out = String::new();

    // Header line.
    let header = if scope_kind == "subtree" {
        let query = scope_root_query.unwrap_or("(unknown)");
        format!("Sheaf nerve for project (subtree of `{query}`)")
    } else {
        "Sheaf nerve for project".to_owned()
    };
    out.push_str(&header);
    out.push('\n');
    out.push_str(&format!("  Sections: {}\n", nerve.sections.len()));
    out.push_str(&format!("  Overlaps: {}\n", nerve.overlaps.len()));

    // Sections.
    for section in &nerve.sections {
        let id_prefix = short_id(&section.node_id.to_string());
        out.push('\n');
        out.push_str(&format!("Section {id_prefix}…\n"));

        if section.constraints.is_empty() {
            out.push_str("  Local: (none)\n");
        } else {
            for c in &section.constraints {
                out.push_str(&format!("  Local: {c}\n"));
            }
        }

        if section.inherited.is_empty() {
            out.push_str("  Inherited: (none)\n");
        } else {
            for c in &section.inherited {
                out.push_str(&format!("  Inherited: {c}\n"));
            }
        }
    }

    // Overlaps.
    for overlap in &nerve.overlaps {
        let a_prefix = short_id(&overlap.node_a.to_string());
        let b_prefix = short_id(&overlap.node_b.to_string());
        out.push('\n');
        out.push_str(&format!("Overlap ({a_prefix}…) — ({b_prefix}…)\n"));
        if overlap.combined.is_empty() {
            out.push_str("  Combined: (none)\n");
        } else {
            out.push_str("  Combined:\n");
            for c in &overlap.combined {
                out.push_str(&format!("    {c}\n"));
            }
        }
    }

    // Obstructions section.
    out.push('\n');
    #[cfg(feature = "z3-verify")]
    {
        out.push_str(&format!("Obstructions: {}\n", obstructions.len()));
        for obs in obstructions {
            let a_prefix = short_id(&obs.node_a.to_string());
            let b_prefix = short_id(&obs.node_b.to_string());
            let overlap_idx = obs.overlap_index;
            match &obs.status {
                ail_contract::ObstructionStatus::Consistent => {
                    out.push_str(&format!(
                        "  [Consistent] {a_prefix}…→{b_prefix}… (overlap {overlap_idx})\n"
                    ));
                }
                ail_contract::ObstructionStatus::Contradictory {
                    conflicting_a,
                    conflicting_b,
                } => {
                    out.push_str(&format!(
                        "  [Contradictory] {a_prefix}…→{b_prefix}… (overlap {overlap_idx})\n"
                    ));
                    for c in conflicting_a {
                        out.push_str(&format!("    node_a contributed: {c}\n"));
                    }
                    for c in conflicting_b {
                        out.push_str(&format!("    node_b contributed: {c}\n"));
                    }
                }
                ail_contract::ObstructionStatus::Unknown { reason } => {
                    out.push_str(&format!(
                        "  [Unknown] {a_prefix}…→{b_prefix}… (overlap {overlap_idx}): {reason}\n"
                    ));
                }
            }
        }
    }
    #[cfg(not(feature = "z3-verify"))]
    {
        out.push_str("Obstructions: skipped (rebuild with --features z3-verify to enable)\n");
    }

    out
}
