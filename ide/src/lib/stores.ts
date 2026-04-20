import { writable, type Writable } from 'svelte/store';
import type { GraphJson } from './types';

export type SelectionKind = 'module' | 'function' | 'step' | 'none';
export interface Selection      { kind: SelectionKind; id: string | null; }
export interface Overlays       { verify: boolean; coverage: boolean; agent: boolean; }
export interface NavHistory     { back: string[]; forward: string[]; }
export type Theme               = 'dark' | 'light';
export type Density             = 'comfortable' | 'compact';

export const graph: Writable<GraphJson | null>  = writable(null);
export const selection: Writable<Selection>     = writable({ kind: 'none', id: null });
export const overlays: Writable<Overlays>       = writable({ verify: true, coverage: false, agent: false });
export const path: Writable<string[]>           = writable([]);
export const history: Writable<NavHistory>      = writable({ back: [], forward: [] });
export const paletteVisible: Writable<boolean>  = writable(false);
export const theme: Writable<Theme>             = writable('dark');
export const density: Writable<Density>         = writable('comfortable');
