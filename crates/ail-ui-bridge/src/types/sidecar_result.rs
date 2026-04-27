//! DTOs for Phase 16.5 sidecar health-check results.
//!
//! These types are returned by the `health_check_core` and `health_check_agent`
//! Tauri commands. Wire format is camelCase JSON matching the TypeScript consumer.

use serde::{Deserialize, Serialize};

/// Whether the sidecar is running from a bundled binary or dev-mode path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SidecarMode {
    Bundled,
    Dev,
}

/// Result of a sidecar health check — returned by `health_check_core` and
/// `health_check_agent`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckPayload {
    /// Sidecar component name (`"ail-core"` or `"ail-agent"`).
    pub component: String,
    /// `true` if the binary was found and `--version` parsed successfully.
    pub ok: bool,
    /// Whether the binary was resolved via the bundle path or dev-mode.
    pub mode: SidecarMode,
    /// Parsed version string (e.g. `"0.1.0"`). `None` when `ok = false`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Human-readable error description. `None` when `ok = true`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
