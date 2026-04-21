<script lang="ts">
  import type { NodeDetail } from '$lib/types';

  export let detail = null as NodeDetail | null;
  export let stepId = '' as string;

  $: verificationBadge = detail?.verification?.ok === true ? 'ok'
                       : detail?.verification?.ok === false ? 'fail'
                       : 'warn';
</script>

<div class="node-detail-card" data-testid="node-view-detail">
  {#if detail}
    <div class="node-detail-card-section">
      <span class="node-detail-card-label">Node</span>
      <span class="node-detail-card-name" data-testid="node-detail-name">{detail.name}</span>
    </div>

    <div class="node-detail-card-section">
      <span class="node-detail-card-label">Status</span>
      <span class="node-detail-card-badge badge-{detail.status}" data-testid="node-detail-status">
        <span class="dot dot-{detail.status}"></span>
        {detail.status}
      </span>
    </div>

    {#if detail.description}
      <div class="node-detail-card-section">
        <span class="node-detail-card-label">Description</span>
        <span class="node-detail-card-desc">{detail.description}</span>
      </div>
    {/if}

    {#if detail.receives.length > 0}
      <div class="node-detail-card-section">
        <span class="node-detail-card-label">Receives</span>
        <table class="node-detail-table" data-testid="node-detail-receives-table">
          <thead>
            <tr><th>name</th><th>type</th></tr>
          </thead>
          <tbody>
            {#each detail.receives as r}
              <tr data-testid="receives-row-{r.name}">
                <td>{r.name}</td>
                <td>{r.desc}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}

    {#if detail.returns.length > 0}
      <div class="node-detail-card-section">
        <span class="node-detail-card-label">Returns</span>
        <table class="node-detail-table" data-testid="node-detail-returns-table">
          <thead>
            <tr><th>name</th><th>type</th></tr>
          </thead>
          <tbody>
            {#each detail.returns as r}
              <tr data-testid="returns-row-{r.name}">
                <td>{r.name}</td>
                <td>{r.desc}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}

    {#if detail.rules.length > 0}
      <div class="node-detail-card-section">
        <span class="node-detail-card-label">Rules ({detail.rules.length})</span>
        <ul class="node-detail-rules-list">
          {#each detail.rules.slice(0, 3) as rule}
            <li class="node-detail-rule">{rule.text}</li>
          {/each}
          {#if detail.rules.length > 3}
            <li class="node-detail-rule-more">+{detail.rules.length - 3} more</li>
          {/if}
        </ul>
      </div>
    {/if}

    <div class="node-detail-card-section">
      <span class="node-detail-card-label">Verification</span>
      <span class="node-detail-card-badge badge-{verificationBadge}">
        {#if detail.verification.ok}✓ verified{:else}✗ failing{/if}
      </span>
    </div>
  {:else}
    <div class="node-detail-empty">No details for <code>{stepId}</code>.</div>
  {/if}
</div>

<style>
  .node-detail-card {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .node-detail-rules-list {
    list-style: none;
    margin: 0;
    padding: 0;
    font-size: 11px;
    color: var(--ink-2);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .node-detail-rule { padding: 2px 0; }
  .node-detail-rule-more { color: var(--ink-3); }
  .node-detail-empty { font-size: 12px; color: var(--ink-3); }
</style>
