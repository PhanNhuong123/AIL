<script lang="ts">
  import FunctionRow from './FunctionRow.svelte';
  import { moduleMode } from './stage-state';
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
  </header>

  {#if $moduleMode === 'List'}
    <ul class="function-list" data-testid="module-view-function-list">
      {#each module.functions as fn_ (fn_.id)}
        <li>
          <FunctionRow fn={fn_} {module} />
        </li>
      {/each}
    </ul>
  {:else}
    <div class="stage-placeholder">Function call graph — coming soon.</div>
  {/if}
</section>
