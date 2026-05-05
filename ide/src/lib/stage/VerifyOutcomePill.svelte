<script lang="ts">
  import type { VerifyOutcome } from '$lib/types';

  /** Whether verification passed. */
  export let ok = false as boolean;
  /** Outcome subtype. Undefined defaults to 'fail' when ok=false. */
  export let outcome = undefined as VerifyOutcome | undefined;

  $: tone = computeTone(ok, outcome);
  $: label = computeLabel(ok, outcome);
  $: glyph = computeGlyph(ok, outcome);

  // No return-type or parameter-type annotations: Svelte 5 + esrap rejects
  // them on script-local helpers (invariant 16.2-E).
  // v4.0 outcome semantics (mirrors the Rust enum):
  //   sat     → ✓ Verified (postcondition entailed)
  //   unsat   → ✗ Counterexample found
  //   unknown → ? Solver inconclusive
  //   timeout → ⏱ Solver timeout
  //   fail    → ✗ Failing (legacy pre-v4.0 payloads with no solver classification)
  function computeTone(ok, outcome) {
    if (ok || outcome === 'sat') return 'ok';
    if (outcome === 'timeout' || outcome === 'unknown') return 'warn';
    return 'fail';
  }

  function computeLabel(ok, outcome) {
    if (ok || outcome === 'sat') return 'verified';
    if (outcome === 'unsat') return 'counterexample';
    if (outcome === 'timeout') return 'timeout';
    if (outcome === 'unknown') return 'unknown';
    return 'failing';
  }

  function computeGlyph(ok, outcome) {
    if (ok || outcome === 'sat') return '✓';
    if (outcome === 'timeout') return '⏱';
    if (outcome === 'unknown') return '?';
    return '✗';
  }
</script>

<span class="vop vop--{tone}" data-testid="verify-outcome-pill" data-tone={tone}>
  <span class="vop__glyph">{glyph}</span>
  <span class="vop__label">{label}</span>
</span>

<style>
  .vop {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    border-radius: 999px;
    font-size: 11px;
  }
  .vop--ok   { color: var(--ok);   background: color-mix(in srgb, var(--ok)   12%, transparent); }
  .vop--fail { color: var(--fail); background: color-mix(in srgb, var(--fail) 12%, transparent); }
  .vop--warn { color: var(--warn); background: color-mix(in srgb, var(--warn) 12%, transparent); }
  .vop__glyph { font-weight: 600; }
</style>
