<script lang="ts">
  import type { NodeDetail } from '$lib/types';

  export let detail = null as NodeDetail | null;

  $: ok = detail?.verification?.ok ?? false;
  $: proven = detail?.proven ?? [];
  $: rules = detail?.rules ?? [];
  $: counterexample = detail?.verification?.counterexample ?? null;

  function isProven(text) {
    return proven.includes(text);
  }
</script>

<div class="node-tab-proof" data-testid="node-tab-proof">
  <div class="node-tab-proof-badge {ok ? 'proof-badge-ok' : 'proof-badge-fail'}" data-testid="node-tab-proof-badge">
    {#if ok}
      ✓ Verified
    {:else}
      ✗ Verification Failed
    {/if}
  </div>

  {#if proven.length > 0}
    <div class="node-tab-section">
      <div class="node-tab-section-label">Proven ({proven.length})</div>
      {#each proven as fact}
        <div class="node-tab-rule-row node-tab-rule-proven" data-testid="proof-fact-{fact}">
          <span>✓</span>
          <span>{fact}</span>
        </div>
      {/each}
    </div>
  {/if}

  {#if rules.length > 0}
    <div class="node-tab-section">
      <div class="node-tab-section-label">Rules</div>
      {#each rules as rule}
        {@const proven_ = isProven(rule.text)}
        <div
          class="node-tab-rule-row {proven_ ? 'node-tab-rule-proven' : 'node-tab-rule-unproven'}"
          data-testid="proof-rule-{rule.text}"
        >
          <span>{proven_ ? '✓' : '○'}</span>
          <span>{rule.text}</span>
        </div>
      {/each}
    </div>
  {/if}

  {#if !ok && counterexample}
    <div class="node-tab-section">
      <div class="node-tab-section-label">Counterexample</div>
      <div class="node-tab-counterexample" data-testid="node-tab-proof-counterexample">
        <div class="counterexample-field">
          <span class="counterexample-key">Scenario</span>
          <span class="counterexample-val">{counterexample.scenario}</span>
        </div>
        <div class="counterexample-field">
          <span class="counterexample-key">Effect</span>
          <span class="counterexample-val">{counterexample.effect}</span>
        </div>
        <div class="counterexample-field">
          <span class="counterexample-key">Violates</span>
          <span class="counterexample-val">{counterexample.violates}</span>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .node-tab-proof {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
</style>
