<script lang="ts">
  import type { NodeDetail } from '$lib/types';
  import {
    nodeViewActiveTab,
    resetTestResultForStep,
    HISTORY_FIXTURE,
  } from './node-view-state';
  import type { NodeTab } from './node-view-state';
  import NodeDetailCard from './NodeDetailCard.svelte';
  import NodeTabCode from './NodeTabCode.svelte';
  import NodeTabProof from './NodeTabProof.svelte';
  import NodeTabTypes from './NodeTabTypes.svelte';
  import NodeTabRules from './NodeTabRules.svelte';
  import NodeTabTest from './NodeTabTest.svelte';
  import NodeTabHistory from './NodeTabHistory.svelte';

  export let stepId = '' as string;
  export let detail = null as NodeDetail | null;

  const TABS = [
    { id: 'code',    label: 'Code'    },
    { id: 'proof',   label: 'Proof'   },
    { id: 'types',   label: 'Types'   },
    { id: 'rules',   label: 'Rules'   },
    { id: 'test',    label: 'Test'    },
    { id: 'history', label: 'History' },
  ];

  // INVARIANT 16.6-B: Only reset nodeTestResult on stepId change.
  // Do NOT reset nodeViewActiveTab or nodeCodeLang.
  $: {
    if (stepId) {
      resetTestResultForStep(stepId);
    }
  }
</script>

<div class="node-view" data-testid="node-view">
  <div class="node-view-layout">
    <!-- Left: detail card -->
    <div class="node-view-detail">
      <NodeDetailCard {detail} {stepId} />
    </div>

    <!-- Right: tabs -->
    <div class="node-view-right">
      <!-- Tab strip -->
      <div class="node-view-tabs" data-testid="node-view-tabs">
        {#each TABS as tab}
          <button
            class="node-tab-btn"
            class:active={$nodeViewActiveTab === tab.id}
            on:click={() => { nodeViewActiveTab.set(tab.id as NodeTab); }}
            data-testid="node-tab-btn-{tab.id}"
          >
            {tab.label}
          </button>
        {/each}
      </div>

      <!-- Tab body -->
      <div class="node-view-tab-body">
        {#if $nodeViewActiveTab === 'code'}
          <NodeTabCode {detail} />
        {:else if $nodeViewActiveTab === 'proof'}
          <NodeTabProof {detail} />
        {:else if $nodeViewActiveTab === 'types'}
          <NodeTabTypes {detail} />
        {:else if $nodeViewActiveTab === 'rules'}
          <NodeTabRules {detail} />
        {:else if $nodeViewActiveTab === 'test'}
          <NodeTabTest {stepId} />
        {:else}
          <NodeTabHistory entries={HISTORY_FIXTURE} />
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .node-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .node-view-layout {
    display: flex;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .node-view-detail {
    width: 280px;
    min-width: 220px;
    flex-shrink: 0;
    overflow-y: auto;
    border-right: 1px solid var(--line);
    padding: 12px;
  }

  .node-view-right {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }

  .node-view-tabs {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    height: 32px;
    padding: 0 8px;
    background: var(--surface-2);
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
  }

  .node-tab-btn {
    display: inline-flex;
    align-items: center;
    height: 26px;
    padding: 0 10px;
    background: transparent;
    border: none;
    color: var(--ink-3);
    font-size: 12px;
    cursor: pointer;
    border-radius: var(--radius-sm) var(--radius-sm) 0 0;
  }

  .node-tab-btn:hover { background: var(--surface-3); color: var(--ink); }
  .node-tab-btn.active {
    background: var(--surface);
    color: var(--ink);
    border-bottom: 2px solid var(--accent);
  }

  .node-view-tab-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 14px;
  }
</style>
