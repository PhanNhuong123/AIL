// Vitest global setup. Mark the jsdom test environment as a Tauri WebView so
// the M2 `isTauri()` guard in `src/lib/bridge.ts` permits real `listen()`
// invocation under existing mocks. Real Tauri injects this marker before any
// JS runs; jsdom does not. We also set `__TAURI_INTERNALS__` for symmetry
// with the SDK runtime — Tauri injects both, and any future code that
// happens to read the internals marker stays consistent under jsdom.
//
// Tests that specifically exercise the non-Tauri path (e.g., the M2 guard
// suite in `bridge.test.ts`) must explicitly `delete` both markers in their
// own `beforeEach` and restore them in `afterEach`.
type TauriWindow = Window & {
  __TAURI_INTERNALS__?: unknown;
  isTauri?: boolean;
};
(window as TauriWindow).__TAURI_INTERNALS__ = {};
(window as TauriWindow).isTauri = true;
