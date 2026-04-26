<script lang="ts">
  import { flip } from 'svelte/animate';
  import { fade } from 'svelte/transition';
  import { graph, activeLens } from '$lib/stores';
  import { patchEffects } from '$lib/patch-effects';
  import FunctionRow from './FunctionRow.svelte';
  import { moduleMode } from './stage-state';
  import { computeModuleHeadSummary } from './lens';
  import type { ModuleJson } from '$lib/types';
  import type { ModuleMode } from './stage-state';

  export let module = null as unknown as ModuleJson;

  const MODES = ['List', 'Graph'] as const;

  function setMode(m) {
    moduleMode.set(m as ModuleMode);
  }

  $: dotClass =
    module.status === 'fail' ? 'dot-fail'
    : module.status === 'warn' ? 'dot-warn'
    : 'dot-ok';

  $: headSummary = $graph ? computeModuleHeadSummary(module, $graph, $activeLens) : null;

  $: addedIds    = $patchEffects.addedIds;
  $: modifiedIds = $patchEffects.modifiedIds;
  $: removedIds  = $patchEffects.removedIds;

  function patchStateFor(id) {
    if (addedIds.includes(id)) return 'added';
    if (modifiedIds.includes(id)) return 'modified';
    if (removedIds.includes(id)) return 'removed';
    return undefined;
  }
</script>

<section class="module-view" data-testid="module-view">
  <header class="module-view-header">
    <span class="module-view-letter" aria-hidden="true">
      {(module.name || '?').slice(0, 1).toUpperCase()}
    </span>
    <h2
      class="module-view-name"
      data-testid="module-view-name"
    >{module.name}</h2>
    <span
      class="dot {dotClass}"
      data-testid="module-view-status"
      aria-hidden="true"
    ></span>
    <span class="module-view-mode-group">
      {#each MODES as m}
        <button
          class="module-view-mode-btn"
          class:active={$moduleMode === m}
          data-testid="module-view-mode-btn-{m.toLowerCase()}"
          type="button"
          aria-pressed={$moduleMode === m}
          on:click={() => setMode(m)}
        >{m}</button>
      {/each}
    </span>
    {#if headSummary}
      <div class="head-actions" data-testid={headSummary.testid}>
        {#each headSummary.chips as chip}
          <span class="head-chip" data-tone={chip.tone}>{chip.label}</span>
        {/each}
      </div>
    {/if}
  </header>

  {#if $moduleMode === 'List'}
    <ul class="function-list" data-testid="module-view-function-list">
      {#each module.functions as fn_ (fn_.id)}
        <li animate:flip={{ duration: 300 }} out:fade={{ duration: 300 }} data-patch-state={patchStateFor(fn_.id)}>
          <FunctionRow fn={fn_} {module} />
        </li>
      {/each}
    </ul>
  {:else}
    <div class="stage-placeholder">Function call graph — coming soon.</div>
  {/if}
</section>

<style>
  .head-actions { display: flex; gap: 8px; margin-left: auto; align-items: center; }
  .head-chip { font-size: 12px; color: var(--ink-3); background: var(--bg-2); border: 1px solid var(--border); border-radius: 999px; padding: 2px 8px; }
</style>
