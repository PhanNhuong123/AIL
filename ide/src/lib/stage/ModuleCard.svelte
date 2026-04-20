<script lang="ts">
  import { graph, overlays } from '$lib/stores';
  import { navigateTo } from '$lib/chrome/toolbar-state';
  import type { ModuleJson } from '$lib/types';
  import { computeModuleMetrics } from './lens';
  import LensPills from './LensPills.svelte';
  import LensBar from './LensBar.svelte';

  export let module = null as unknown as ModuleJson;

  const MAX_FN_PREVIEW = 4;

  $: metrics = $graph ? computeModuleMetrics(module, $graph, $overlays) : null;
  $: functionsPreview = module.functions.slice(0, MAX_FN_PREVIEW);
  $: moreCount = Math.max(0, module.functions.length - MAX_FN_PREVIEW);
  $: letter = (module.name || '?').slice(0, 1).toUpperCase();
  $: dotClass =
    module.status === 'fail' ? 'dot-fail'
    : module.status === 'warn' ? 'dot-warn'
    : 'dot-ok';

  function handleClick() {
    const g = $graph;
    if (!g) return;
    const newPath = [g.project.id, module.id];
    navigateTo(newPath, 'module', module.id, 1);
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
  class="module-card"
  data-testid="module-card-{module.id}"
  style="border-left-color: {module.clusterColor}"
  type="button"
  on:click={handleClick}
  on:keydown={handleKey}
>
  <header class="module-card-header">
    <span class="module-card-letter" aria-hidden="true">{letter}</span>
    <span
      class="module-card-name"
      data-testid="module-card-name-{module.id}"
    >{module.name}</span>
    <span class="dot {dotClass}" data-testid="module-card-status-{module.id}" aria-hidden="true"></span>
  </header>

  {#if metrics && metrics.showDescription && module.description}
    <p class="module-card-desc">{module.description}</p>
  {/if}

  {#if metrics}
    <div class="module-card-pills" data-testid="module-card-pills-{module.id}">
      <LensPills pills={metrics.pills} />
    </div>
    <div class="module-card-bar" data-testid="module-card-bar-{module.id}">
      <LensBar bar={metrics.bar} />
    </div>
  {/if}

  <ul class="module-card-fn-list">
    {#each functionsPreview as fn_}
      <li
        class="module-card-fn-row"
        data-testid="module-card-fn-row-{fn_.id}"
      >
        <span class="module-card-fn-name">ƒ {fn_.name}</span>
        <span
          class="dot dot-sm {fn_.status === 'fail' ? 'dot-fail' : fn_.status === 'warn' ? 'dot-warn' : 'dot-ok'}"
          aria-hidden="true"
        ></span>
      </li>
    {/each}
    {#if moreCount > 0}
      <li class="module-card-more" data-testid="module-card-more-{module.id}">
        +{moreCount} more
      </li>
    {/if}
  </ul>
</button>
