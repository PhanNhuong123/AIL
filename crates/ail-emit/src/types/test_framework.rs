/// Controls which test framework is targeted when generating TypeScript test stubs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum TestFramework {
    /// Emit `import { describe, it, expect } from 'vitest';` (default).
    #[default]
    Vitest,
    /// Emit `import { describe, it, expect } from '@jest/globals';`.
    Jest,
}
