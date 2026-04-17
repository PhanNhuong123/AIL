//! `ail-coverage` — semantic coverage scoring for AIL parent→children intents.
//!
//! Pipeline position:
//!   ValidGraph + EmbeddingProvider  →  compute_coverage  →  CoverageResult
pub mod concepts;
pub mod coverage;
pub mod errors;
pub mod missing;
pub mod projection;
pub mod types;

pub use ail_graph::cic::{
    ChildContributionInfo, CoverageConfig, CoverageInfo, CoverageStatus, MissingAspectInfo,
};
pub use concepts::{DEFAULT_CONCEPT_LIST, MISSING_ASPECT_THRESHOLD};
pub use coverage::compute_coverage;
pub use errors::CoverageError;
pub use types::{ChildContribution, CoverageResult, MissingAspect};
