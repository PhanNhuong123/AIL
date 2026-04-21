<script lang="ts">
  import { HISTORY_FIXTURE } from './node-view-state';

  export let entries = HISTORY_FIXTURE;

  function formatTs(ts) {
    try {
      return new Date(ts).toLocaleString();
    } catch {
      return ts;
    }
  }
</script>

<div class="node-tab-history" data-testid="node-tab-history">
  {#if entries.length > 0}
    <ul class="node-tab-history-list">
      {#each entries as entry, i}
        <li class="node-tab-history-entry" data-testid="history-entry-{i}">
          <div class="node-tab-history-meta">
            <span class="node-tab-history-author">{entry.author}</span>
            <span class="node-tab-history-ts">{formatTs(entry.timestamp)}</span>
          </div>
          <span class="node-tab-history-msg">{entry.message}</span>
        </li>
      {/each}
    </ul>
  {:else}
    <div class="node-tab-empty">No history entries.</div>
  {/if}
</div>

<style>
  .node-tab-history { display: flex; flex-direction: column; }
  .node-tab-empty { font-size: 12px; color: var(--ink-3); }
</style>
