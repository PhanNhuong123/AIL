/**
 * Phase 16.5 — Sidecar IPC contract tests.
 *
 * Verifies that `healthCheckCore` and `healthCheckAgent` in `bridge.ts` invoke
 * the correct command names and return typed `HealthCheckPayload` shapes.
 * Uses vi.mock to replace the Tauri `invoke` call so no real sidecar is needed.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
// event module is not used by the sidecar commands but the bridge imports it
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

import { healthCheckCore, healthCheckAgent } from '$lib/bridge';
import type { HealthCheckPayload, SidecarMode } from '$lib/types';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makePayload(
  component: string,
  ok: boolean,
  mode: SidecarMode,
  version?: string,
  error?: string,
): HealthCheckPayload {
  return { component, ok, mode, version, error };
}

describe('sidecar IPC contract', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  // 1. healthCheckCore returns HealthCheckPayload shape with bundled mode
  it('healthCheckCore returns HealthCheckPayload shape with bundled mode', async () => {
    const expected = makePayload('ail-core', true, 'bundled', '0.1.0');
    invoke.mockResolvedValue(expected);

    const result = await healthCheckCore();

    expect(result.component).toBe('ail-core');
    expect(result.ok).toBe(true);
    expect(result.mode).toBe('bundled');
    expect(result.version).toBe('0.1.0');
    expect(result.error).toBeUndefined();
  });

  // 2. healthCheckAgent returns HealthCheckPayload shape with bundled mode
  it('healthCheckAgent returns HealthCheckPayload shape with bundled mode', async () => {
    const expected = makePayload('ail-agent', true, 'bundled', '0.1.0');
    invoke.mockResolvedValue(expected);

    const result = await healthCheckAgent();

    expect(result.component).toBe('ail-agent');
    expect(result.ok).toBe(true);
    expect(result.mode).toBe('bundled');
    expect(result.version).toBe('0.1.0');
  });

  // 3. healthCheckCore ok=false carries error string
  it('healthCheckCore ok=false carries error string', async () => {
    const expected = makePayload('ail-core', false, 'dev', undefined, 'spawn failed: No such file');
    invoke.mockResolvedValue(expected);

    const result = await healthCheckCore();

    expect(result.ok).toBe(false);
    expect(result.mode).toBe('dev');
    expect(result.error).toBe('spawn failed: No such file');
    expect(result.version).toBeUndefined();
  });

  // 4. healthCheckCore ok=true omits error
  it('healthCheckCore ok=true omits error field', async () => {
    const expected = makePayload('ail-core', true, 'dev', '0.1.0');
    invoke.mockResolvedValue(expected);

    const result = await healthCheckCore();

    expect(result.ok).toBe(true);
    expect(result.error).toBeUndefined();
  });

  // 5. SidecarMode bundled and dev are exhaustive (TS type-level)
  it('SidecarMode accepts only bundled and dev', () => {
    const bundled: SidecarMode = 'bundled';
    const dev: SidecarMode = 'dev';
    expect(bundled).toBe('bundled');
    expect(dev).toBe('dev');

    // @ts-expect-error — unknown mode must be rejected by TypeScript
    const _bad: SidecarMode = 'unknown';
    void _bad;
  });

  // 6. healthCheckCore invokes 'health_check_core' command name
  it("healthCheckCore invokes 'health_check_core' command name", async () => {
    invoke.mockResolvedValue(makePayload('ail-core', true, 'bundled', '0.1.0'));

    await healthCheckCore();

    expect(invoke).toHaveBeenCalledWith('health_check_core');
  });

  // 7. healthCheckAgent invokes 'health_check_agent' command name
  it("healthCheckAgent invokes 'health_check_agent' command name", async () => {
    invoke.mockResolvedValue(makePayload('ail-agent', true, 'bundled', '0.1.0'));

    await healthCheckAgent();

    expect(invoke).toHaveBeenCalledWith('health_check_agent');
  });

  // 8. bridge.ts exports both health-check wrappers
  it('bridge.ts exports both health-check wrappers', async () => {
    // We already imported them at the top; if the imports resolve, the test passes.
    expect(typeof healthCheckCore).toBe('function');
    expect(typeof healthCheckAgent).toBe('function');
  });
});
