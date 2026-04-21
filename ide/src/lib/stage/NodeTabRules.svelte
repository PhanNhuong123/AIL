<script lang="ts">
  import type { NodeDetail, InheritedRule } from '$lib/types';

  export let detail = null as NodeDetail | null;

  $: ownRules       = detail?.rules     ?? [];
  $: inheritedRules = detail?.inherited ?? [];

  $: inheritedByFrom = groupByFrom(inheritedRules);

  function groupByFrom(rules) {
    const m = new Map();
    for (const r of rules) {
      const bucket = m.get(r.from) ?? [];
      bucket.push(r);
      m.set(r.from, bucket);
    }
    return m;
  }
</script>

<div class="node-tab-rules" data-testid="node-tab-rules">
  <div class="node-tab-section">
    <div class="node-tab-section-label">Own Rules ({ownRules.length})</div>
    {#if ownRules.length > 0}
      {#each ownRules as rule}
        <div class="node-tab-rule-row" data-testid="rule-own-{rule.text}">
          <span class="rule-source-badge">own</span>
          <span>{rule.text}</span>
        </div>
      {/each}
    {:else}
      <div class="node-tab-empty">No own rules.</div>
    {/if}
  </div>

  {#if inheritedRules.length > 0}
    <div class="node-tab-section">
      <div class="node-tab-section-label">Inherited ({inheritedRules.length})</div>
      {#each [...inheritedByFrom.entries()] as [from, rules]}
        <div class="inherited-group" data-testid="rules-inherited-from-{from}">
          <div class="inherited-from-label">from {from}</div>
          {#each rules as rule}
            <div class="node-tab-rule-row" data-testid="rule-inherited-{rule.text}">
              <span class="rule-source-badge badge-inherited">↳</span>
              <span>{rule.text}</span>
            </div>
          {/each}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .node-tab-rules { display: flex; flex-direction: column; gap: 12px; }
  .node-tab-empty { font-size: 12px; color: var(--ink-3); }

  .rule-source-badge {
    display: inline-block;
    padding: 1px 5px;
    font-size: 10px;
    border-radius: var(--radius-sm);
    background: var(--surface-3);
    color: var(--ink-3);
    flex-shrink: 0;
  }

  .badge-inherited {
    background: color-mix(in srgb, var(--accent) 15%, transparent);
    color: var(--accent);
  }

  .inherited-group { margin-bottom: 8px; }
  .inherited-from-label { font-size: 10px; color: var(--accent); margin-bottom: 4px; font-style: italic; }
</style>
