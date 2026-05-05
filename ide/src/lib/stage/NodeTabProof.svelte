<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { NodeDetail } from '$lib/types';

  export let detail = null as NodeDetail | null;
  /**
   * Path-like id of the step this proof tab is rendering. Forwarded with
   * action-button events so the route can build a focused agent prompt.
   * Empty string when the tab is rendered without a node context — actions
   * are suppressed in that case (test/fixture mounts).
   */
  export let stepId = '' as string;

  $: ok = detail?.verification?.ok ?? false;
  $: proven = detail?.proven ?? [];
  $: rules = detail?.rules ?? [];
  $: counterexample = detail?.verification?.counterexample ?? null;
  $: nodeName = detail?.name ?? '';

  // No type annotations on dispatcher: Svelte 5 + esrap rejects them on
  // script-local helpers (invariant 16.2-E).
  const dispatch = createEventDispatcher();

  // Pass `{ bubbles: true }` so DOM-level addEventListener consumers (the
  // route shell) catch the event — Svelte 5's createEventDispatcher
  // defaults to non-bubbling. The `as never` cast bypasses the incomplete
  // DispatchOptions TS type. Mirrors QuickCreateModal's BUBBLE pattern.
  const BUBBLE = { bubbles: true } as never;

  function isProven(text) {
    return proven.includes(text);
  }

  function handleSuggestFix() {
    if (!stepId || !counterexample) return;
    dispatch(
      'suggestfix',
      {
        stepId,
        nodeName,
        scenario: counterexample.scenario,
        effect: counterexample.effect,
        violates: counterexample.violates,
      },
      BUBBLE,
    );
  }

  function handleRelaxRule() {
    if (!stepId) return;
    dispatch('relaxrule', { stepId, nodeName, rules: rules.map((r) => r.text) }, BUBBLE);
  }

  function handleAddHandler() {
    if (!stepId || !counterexample) return;
    dispatch(
      'addhandler',
      {
        stepId,
        nodeName,
        scenario: counterexample.scenario,
        violates: counterexample.violates,
      },
      BUBBLE,
    );
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

      {#if stepId}
        <!-- Phase 20 action buttons. The route shell handles the dispatched
             events: `suggestfix` invokes the AI agent with a focused prompt;
             `relaxrule` and `addhandler` raise inline notices so the user
             knows the deeper graph mutations are wired in v4.2. -->
        <div class="node-tab-proof-actions" data-testid="node-tab-proof-actions">
          <button
            type="button"
            class="proof-action-btn primary"
            data-testid="proof-action-suggest-fix"
            on:click={handleSuggestFix}
          >Suggest fix</button>
          <button
            type="button"
            class="proof-action-btn"
            data-testid="proof-action-relax-rule"
            on:click={handleRelaxRule}
          >Relax rule</button>
          <button
            type="button"
            class="proof-action-btn"
            data-testid="proof-action-add-handler"
            on:click={handleAddHandler}
          >Add handler</button>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .node-tab-proof {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .node-tab-proof-actions {
    display: flex;
    gap: 8px;
    margin-top: 10px;
    flex-wrap: wrap;
  }
  .proof-action-btn {
    height: 28px;
    padding: 0 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--line);
    background: var(--surface-2);
    color: var(--ink);
    font-size: 12px;
    cursor: pointer;
  }
  .proof-action-btn:hover { background: var(--surface-3); }
  .proof-action-btn.primary {
    background: var(--accent);
    color: white;
    border-color: var(--accent);
  }
  .proof-action-btn.primary:hover {
    filter: brightness(1.05);
  }
</style>
