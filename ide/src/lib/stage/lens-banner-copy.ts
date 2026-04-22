/**
 * lens-banner-copy.ts — Pure helpers for LensBanner (task 15.5).
 *
 * Keeps `LensBanner.svelte` thin: label/description text, per-lens hue token
 * name, and `formatLensStats(stats)` that turns a `LensStats` union into a
 * human-readable string.
 *
 * Hue mapping reuses existing semantic tokens from `styles/tokens.css` — no
 * new `--lens-*-hue` variables are introduced.
 */

import type { Lens, LensStats } from '$lib/types';

export const LENS_LABEL: Record<Lens, string> = {
  structure: 'Structure',
  rules: 'Rules',
  verify: 'Verify',
  data: 'Data',
  tests: 'Tests',
};

export const LENS_DESCRIPTION: Record<Lens, string> = {
  structure: 'Modules, functions, and step composition',
  rules: 'Contracts declared and proven',
  verify: 'Proofs, obligations, and counterexamples',
  data: 'Types and signal flow',
  tests: 'Executed tests and outcomes',
};

export function hueTokenFor(lens: Lens): string {
  switch (lens) {
    case 'structure': return '--ink-3';
    case 'rules':     return '--warn';
    case 'verify':    return '--accent';
    case 'data':      return '--ok';
    case 'tests':     return '--fail';
  }
}

export function formatLensStats(stats: LensStats | null): string {
  if (!stats) return '—';
  switch (stats.lens) {
    case 'structure':
      return `${stats.modules} modules · ${stats.functions} fns · ${stats.steps} steps`;
    case 'rules':
      return `${stats.total} rules · ${stats.unproven} unproven · ${stats.broken} broken`;
    case 'verify':
      return `${stats.proven} proven · ${stats.unproven} unproven · ${stats.counterexamples} cex`;
    case 'data':
      return `${stats.signals} signals · ${stats.types.length} types`;
    case 'tests':
      return `${stats.total} tests · ${stats.passing} passing · ${stats.failing} failing`;
  }
}
