/// Configuration for the Python emitter.
#[derive(Debug, Clone, Default)]
pub struct EmitConfig {
    /// Emit `async def` and `await`-prefixed repository calls when true.
    ///
    /// When false, functions emit as `def` and repository calls are synchronous.
    /// Functions containing `together` blocks always require `async_mode = true`
    /// because `together` unconditionally emits `async with transaction():`.
    pub async_mode: bool,
}
