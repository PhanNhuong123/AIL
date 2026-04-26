<script lang="ts">
  import { onDestroy } from 'svelte';
  import { activeLens, graph } from '$lib/stores';
  import { computeLensMetrics } from '$lib/bridge';
  import { verifyTick } from '$lib/verify/verify-state';
  import type { LensStats } from '$lib/types';
  import {
    LENS_LABEL,
    LENS_DESCRIPTION,
    hueTokenFor,
    formatLensStats,
  } from './lens-banner-copy';

  export let scopeId = null as string | null;

  let stats = null as LensStats | null;
  let lastRequest = 0;
  let destroyed = false;

  onDestroy(() => { destroyed = true; });

  // `$graph` is added as a reactive dependency so that a watcher-driven
  // patch update triggers a fresh metrics fetch (fix 15.11 MED staleness).
  // `$verifyTick` is added as a reactive dependency so that a verify-complete
  // event triggers a fresh metrics fetch (Phase 16.3).
  // Neither value is used in the body — they are triggers only.
  $: void refetch($activeLens, scopeId, $graph, $verifyTick);

  async function refetch(lens, scope, _g, _tick) {
    const lensVal = lens as import('$lib/types').Lens;
    const scopeVal = scope as string | null;
    const reqId = ++lastRequest;
    try {
      const next = await computeLensMetrics(lensVal, scopeVal);
      if (!destroyed && reqId === lastRequest) stats = next;
    } catch (err) {
      if (!destroyed && reqId === lastRequest) stats = null;
      console.warn('[LensBanner] computeLensMetrics failed:', err);
    }
  }
</script>

<div
  class="lens-banner"
  data-testid="lens-banner"
  data-lens={$activeLens}
  style="--lens-hue: var({hueTokenFor($activeLens)})"
>
  <div class="lens-banner-left">
    <span class="lens-banner-title" data-testid="lens-banner-title">
      {LENS_LABEL[$activeLens]}
    </span>
    <span class="lens-banner-desc" data-testid="lens-banner-desc">
      {LENS_DESCRIPTION[$activeLens]}
    </span>
  </div>
  <div class="lens-banner-stats" data-testid="lens-banner-stats">
    {formatLensStats(stats)}
  </div>
</div>
