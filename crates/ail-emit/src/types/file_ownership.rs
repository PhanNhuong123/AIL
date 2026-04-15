/// Ownership policy for an emitted file.
///
/// Consumers (e.g. `ail build`) use this to decide whether to overwrite an existing file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileOwnership {
    /// AIL owns this file. Always overwritten on every rebuild.
    ///
    /// All files under `generated/` carry this variant.
    Generated,
    /// Developer owns this file after first creation. Never overwritten by the emitter.
    ///
    /// All files under `scaffolded/` carry this variant.
    Scaffolded,
}
