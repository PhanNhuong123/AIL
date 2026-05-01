import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
  sidecarHealth,
  sidecarChecking,
  resetSidecarState,
} from './sidecar-state';

describe('sidecar-state stores', () => {
  beforeEach(() => resetSidecarState());

  it('starts with both component slots empty', () => {
    const h = get(sidecarHealth);
    expect(h.core).toBeNull();
    expect(h.agent).toBeNull();
  });

  it('starts with both checking flags false', () => {
    const c = get(sidecarChecking);
    expect(c.core).toBe(false);
    expect(c.agent).toBe(false);
  });

  it('writes only the targeted component when health is updated', () => {
    sidecarHealth.update((s) => ({
      ...s,
      core: {
        component: 'ail-core',
        ok: true,
        mode: 'dev',
        version: '0.1.0',
      },
    }));
    const h = get(sidecarHealth);
    expect(h.core?.ok).toBe(true);
    expect(h.agent).toBeNull();
  });

  it('resetSidecarState clears prior writes', () => {
    sidecarHealth.set({
      core: { component: 'ail-core', ok: true, mode: 'dev' },
      agent: { component: 'ail-agent', ok: false, mode: 'dev', error: 'x' },
    });
    sidecarChecking.set({ core: true, agent: true });

    resetSidecarState();

    expect(get(sidecarHealth).core).toBeNull();
    expect(get(sidecarHealth).agent).toBeNull();
    expect(get(sidecarChecking).core).toBe(false);
    expect(get(sidecarChecking).agent).toBe(false);
  });
});
