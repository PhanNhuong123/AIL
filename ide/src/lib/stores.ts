import { writable, type Writable } from 'svelte/store';
import type { GraphJson, Lens } from './types';

export type SelectionKind = 'project' | 'module' | 'function' | 'step' | 'type' | 'error' | 'none';
export interface Selection      { kind: SelectionKind; id: string | null; }
export interface Overlays       { rules: boolean; verification: boolean; dataflow: boolean; dependencies: boolean; tests: boolean; }
export interface NavHistory     { back: string[]; forward: string[]; }
export type Theme               = 'dark' | 'light';
export type Density             = 'comfortable' | 'compact' | 'cozy';

export const graph: Writable<GraphJson | null>  = writable(null);
export const selection: Writable<Selection>     = writable({ kind: 'none', id: null });
export const overlays: Writable<Overlays>       = writable({ rules: false, verification: true, dataflow: false, dependencies: false, tests: false });
export const path: Writable<string[]>           = writable([]);
export const history: Writable<NavHistory>      = writable({ back: [], forward: [] });
export const paletteVisible: Writable<boolean>  = writable(false);
export const theme: Writable<Theme>             = writable('dark');
export const density: Writable<Density>         = writable('comfortable');
export const welcomeModalOpen: Writable<boolean> = writable(false);
export const quickCreateModalOpen: Writable<boolean> = writable(false);
export const tweaksPanelOpen: Writable<boolean> = writable(false);

// Inline notice surfaced inside WelcomeModal — used by the route shell to
// communicate UX feedback that the modal itself can render (e.g. "Open in
// browser preview is unavailable; launch the desktop app to load a project").
// Empty string hides the notice. Session-only (no localStorage).
export const welcomeNotice: Writable<string> = writable('');
export type { Lens } from './types';
export const activeLens: Writable<Lens> = writable('verify');
