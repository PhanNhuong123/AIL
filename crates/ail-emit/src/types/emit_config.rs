use crate::types::{ContractMode, TestFramework};

/// Configuration for the emitter targets.
#[derive(Debug, Clone, Default)]
pub struct EmitConfig {
    /// Emit `async def` / `async function` and `await`-prefixed repository calls when true.
    ///
    /// When false, functions emit as `def` / `function` and repository calls are synchronous.
    /// Functions containing `together` blocks always require `async_mode = true`
    /// because `together` unconditionally emits `async with transaction():` (Python) or
    /// `await source.transaction(...)` (TypeScript).
    pub async_mode: bool,

    /// Controls how contract assertions are written into generated code.
    ///
    /// `On` (default) emits live runtime checks; `Comments` emits them as comments;
    /// `Off` omits all contract output; `Test` emits contracts only in test files.
    pub contract_mode: ContractMode,

    /// Controls which test framework is targeted when generating TypeScript test stubs.
    ///
    /// `Vitest` (default) emits `import { describe, it, expect } from 'vitest';`.
    /// `Jest` emits `import { describe, it, expect } from '@jest/globals';`.
    /// This field has no effect on Python output.
    pub test_framework: TestFramework,
}
