<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { NodeDetail } from '$lib/types';
  import { activeLens } from '$lib/stores';
  import {
    nodeViewActiveTab,
    resetTestResultForStep,
  } from './node-view-state';
  import type { NodeTab } from './node-view-state';
  import NodeDetailCard from './NodeDetailCard.svelte';
  import NodeDetailLensSection from './NodeDetailLensSection.svelte';
  import NodeViewConflictSection from './NodeViewConflictSection.svelte';
  import NodeTabCode from './NodeTabCode.svelte';
  import NodeTabProof from './NodeTabProof.svelte';
  import NodeTabTypes from './NodeTabTypes.svelte';
  import NodeTabRules from './NodeTabRules.svelte';
  // NodeTabTest + NodeTabHistory unmounted in v4.0 (Test stub + Alice/Bob fixture
  // are dishonest for ship). Files retained for v4.1 wire-up.
  import { sheafConflicts } from '$lib/sheaf/sheaf-state';

  export let stepId = '' as string;
  export let detail = null as NodeDetail | null;

  const dispatch = createEventDispatcher();

  const TABS = [
    { id: 'code',    label: 'Code'    },
    { id: 'proof',   label: 'Proof'   },
    { id: 'types',   label: 'Types'   },
    { id: 'rules',   label: 'Rules'   },
  ];

  // INVARIANT 16.6-B: Only reset nodeTestResult on stepId change.
  // Do NOT reset nodeViewActiveTab or nodeCodeLang.
  $: {
    if (stepId) {
      resetTestResultForStep(stepId);
    }
  }

  $: nodeConflicts = $sheafConflicts.filter(
    (c) => c.nodeA === stepId || c.nodeB === stepId
  );

  function handleJump(peerId) {
    dispatch('jumptonode', { peerId });
  }
</script>

<div class="node-view" data-testid="node-view">
  <div class="node-view-layout">
    <!-- Left: detail card -->
    <div class="node-view-detail" data-testid="node-view-detail">
      <NodeDetailCard {detail} {stepId} />
      <NodeDetailLensSection lens={$activeLens} {detail} />
      {#if $activeLens === 'verify'}
        <NodeViewConflictSection
          conflicts={nodeConflicts}
          currentNodeId={stepId}
          onJump={handleJump}
        />
      {/if}
    </div>

    <!-- Right: tabs -->
    <div class="node-view-right">
      <!-- Tab strip — full ARIA tab semantics so keyboard + screen-reader
           users can navigate the side panel. The tab body becomes the
           controlled tabpanel. -->
      <div class="node-view-tabs" role="tablist" aria-label="Node detail" data-testid="node-view-tabs">
        {#each TABS as tab}
          <button
            class="node-tab-btn"
            class:active={$nodeViewActiveTab === tab.id}
            role="tab"
            id="node-tab-btn-{tab.id}"
            aria-selected={$nodeViewActiveTab === tab.id}
            aria-controls="node-tab-panel-{tab.id}"
            tabindex={$nodeViewActiveTab === tab.id ? 0 : -1}
            on:click={() => { nodeViewActiveTab.set(tab.id as NodeTab); }}
            data-testid="node-tab-btn-{tab.id}"
          >
            {tab.label}
          </button>
        {/each}
      </div>

      <!-- Tab body -->
      <div
        class="node-view-tab-body"
        role="tabpanel"
        id="node-tab-panel-{$nodeViewActiveTab}"
        aria-labelledby="node-tab-btn-{$nodeViewActiveTab}"
        tabindex="0"
      >
        {#if $nodeViewActiveTab === 'proof'}
          <NodeTabProof {detail} {stepId} />
        {:else if $nodeViewActiveTab === 'types'}
          <NodeTabTypes {detail} />
        {:else if $nodeViewActiveTab === 'rules'}
          <NodeTabRules {detail} />
        {:else}
          <NodeTabCode {detail} />
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
