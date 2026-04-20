<script lang="ts">
  import { graph, overlays } from '$lib/stores';
  import { navigateTo } from '$lib/chrome/toolbar-state';
  import type { ModuleJson, FunctionJson } from '$lib/types';
  import { computeFunctionMetrics } from './lens';
  import LensPills from './LensPills.svelte';

  export let fn = null as unknown as FunctionJson;
  export let module = null as unknown as ModuleJson;

  $: metrics = $graph ? computeFunctionMetrics(fn, module, $graph, $overlays) : null;
  $: dotClass =
    fn.status === 'fail' ? 'dot-fail'
    : fn.status === 'warn' ? 'dot-warn'
    : 'dot-ok';

  function handleClick() {
    const g = $graph;
    if (!g) return;
    const newPath = [g.project.id, module.id, fn.id];
    navigateTo(newPath, 'function', fn.id, 2);
  }

  function handleKey(e) {
    const ev = e as KeyboardEvent;
    if (ev.key === 'Enter' || ev.key === ' ') {
      ev.preventDefault();
      handleClick();
    }
  }
</script>

<button
  class="function-row"
  data-testid="function-row-{fn.id}"
  type="button"
  on:click={handleClick}
  on:keydown={handleKey}
>
  <span
    class="function-row-name"
    data-testid="function-row-name-{fn.id}"
  >ƒ {fn.name}</span>
  {#if metrics}
    <span class="function-row-pills">
      <LensPills pills={metrics.pills} />
    </span>
  {/if}
  <span
    class="dot function-row-status {dotClass}"
    data-testid="function-row-status-{fn.id}"
    aria-hidden="true"
  ></span>
</button>
