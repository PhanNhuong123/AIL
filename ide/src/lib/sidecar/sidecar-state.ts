// Session-only stores for sidecar health-check UI surfaces (closes review
// finding **N3** — the Tweaks panel needs a Sidecars section).
//
// Pattern mirror: `verify-state.ts`, `sheaf-state.ts`, `reviewer-state.ts`.
// The route shell owns IPC calls (`+page.svelte` → `bridge.healthCheck*`)
// and writes to these stores; modal components read only.
//
// Layout:
// - `sidecarHealth` holds the most recent payload per component, or `null`
//   before the user has triggered a check. Cleared only by reset().
// - `sidecarChecking` holds in-flight booleans so the UI can disable the
//   buttons while a check is running.

import { writable } from 'svelte/store';
import type { HealthCheckPayload } from '$lib/types';

export interface SidecarHealthState {
  core: HealthCheckPayload | null;
  agent: HealthCheckPayload | null;
}

export interface SidecarCheckingState {
  core: boolean;
  agent: boolean;
}

const HEALTH_INITIAL: SidecarHealthState = { core: null, agent: null };
const CHECKING_INITIAL: SidecarCheckingState = { core: false, agent: false };

export const sidecarHealth = writable<SidecarHealthState>({ ...HEALTH_INITIAL });
export const sidecarChecking = writable<SidecarCheckingState>({ ...CHECKING_INITIAL });

/** Reset both stores to their initial state. Used by tests. */
export function resetSidecarState(): void {
  sidecarHealth.set({ ...HEALTH_INITIAL });
  sidecarChecking.set({ ...CHECKING_INITIAL });
}
