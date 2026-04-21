<script lang="ts">
  import type { NodeDetail, InheritedRule } from '$lib/types';

  export let detail = null as NodeDetail | null;

  $: ownRules = detail?.rules ?? [];
  $: inheritedList = detail?.inherited ?? [];
  $: provenList = detail?.proven ?? [];
  $: inheritedByFrom = groupByFrom(inheritedList);

  function groupByFrom(list) {
    const listTyped = list as readonly InheritedRule[];
    const map = new Map() as Map<string, InheritedRule[]>;
    for (const rule of listTyped) {
      const from = rule.from;
      const arr = map.get(from) ?? [];
      arr.push(rule);
      map.set(from, arr);
    }
    return map;
  }
</script>

<div class="ctx-rules" data-testid="ctx-tab-rules">
  <!-- Own rules -->
  <div class="ctx-rules-section" data-testid="ctx-rules-own">
    <div class="ctx-section-label">Own rules ({ownRules.length})</div>
    {#if ownRules.length === 0}
      <div class="ctx-rules-empty">No own rules.</div>
    {:else}
      {#each ownRules as rule}
        <div class="ctx-rule-row">
          <span class="ctx-rule-badge ctx-badge-own">[own]</span>
          <span class="ctx-rule-text">{rule.text}</span>
        </div>
      {/each}
    {/if}
    <button class="ctx-add-rule-btn" data-testid="ctx-add-rule-btn">+ Add rule</button>
  </div>

  <!-- Inherited rules -->
  {#if inheritedList.length > 0}
    <div class="ctx-rules-section ctx-rules-inherited" data-testid="ctx-rules-inherited">
      <div class="ctx-section-label">Inherited</div>
      {#each [...inheritedByFrom.entries()] as [from, rules]}
        <div
          class="ctx-inherited-group"
          data-testid="ctx-inherited-from-{from}"
        >
          <div class="ctx-inherited-from-label">from {from}</div>
          {#each rules as rule}
            <div class="ctx-rule-row ctx-rule-row-inherited">
              <span class="ctx-rule-badge ctx-badge-inherited">↓ [from {from}]</span>
              <span class="ctx-rule-text">{rule.text}</span>
            </div>
          {/each}
        </div>
      {/each}
    </div>
  {/if}

  <!-- Proven facts -->
  {#if provenList.length > 0}
    <div class="ctx-rules-section ctx-rules-proven" data-testid="ctx-rules-proven">
      <div class="ctx-section-label">Proven facts</div>
      {#each provenList as fact}
        <div class="ctx-rule-row ctx-rule-row-proven">
          <span class="ctx-rule-badge ctx-badge-proven">✓</span>
          <span class="ctx-rule-text ctx-rule-text-proven">{fact}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>
