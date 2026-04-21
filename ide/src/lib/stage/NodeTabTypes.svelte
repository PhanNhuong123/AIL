<script lang="ts">
  import type { NodeDetail } from '$lib/types';

  export let detail = null as NodeDetail | null;

  $: receives = detail?.receives ?? [];
  $: returns  = detail?.returns  ?? [];
</script>

<div class="node-tab-types" data-testid="node-tab-types">
  <div class="node-tab-section">
    <div class="node-tab-section-label">Receives ({receives.length})</div>
    {#if receives.length > 0}
      <table class="node-detail-table">
        <thead>
          <tr><th>name</th><th>type</th></tr>
        </thead>
        <tbody>
          {#each receives as r}
            <tr data-testid="types-receives-{r.name}">
              <td>{r.name}</td>
              <td>{r.desc}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <div class="node-tab-empty">No inputs.</div>
    {/if}
  </div>

  <div class="node-tab-section">
    <div class="node-tab-section-label">Returns ({returns.length})</div>
    {#if returns.length > 0}
      <table class="node-detail-table">
        <thead>
          <tr><th>name</th><th>type</th></tr>
        </thead>
        <tbody>
          {#each returns as r}
            <tr data-testid="types-returns-{r.name}">
              <td>{r.name}</td>
              <td>{r.desc}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <div class="node-tab-empty">No outputs.</div>
    {/if}
  </div>
</div>

<style>
  .node-tab-types { display: flex; flex-direction: column; gap: 12px; }
  .node-tab-empty { font-size: 12px; color: var(--ink-3); }
</style>
