<script lang="ts">
  import type { FunctionJson, FlowchartJson, NodeDetail } from '$lib/types';
  import { flowMode, flowFocusedNodeId, flowSelectedNodeId } from './flow-state';
  import { projectLayout } from '$lib/stores';
  import FlowSwim from './FlowSwim.svelte';
  import FlowchartCanvas from './FlowchartCanvas.svelte';
  import FlowCode from './FlowCode.svelte';
  import FlowMinimap from './FlowMinimap.svelte';
  import Icon from '$lib/icons/Icon.svelte';

  export let fn        = null as FunctionJson | null;
  export let flowchart = null as FlowchartJson | null;
  export let detail    = null as NodeDetail | null;

  // Hand the flowchart canvas the per-project layout entries so previously
  // persisted drag positions take precedence over the swim/system layout
  // defaults. Null until `loadProjectLayout` resolves on project open.
  $: layoutOverrides = $projectLayout?.nodes ?? null;

  function setMode(m) {
    flowMode.set(m);
  }

  // Clear focus when leaving Swim mode
  $: if ($flowMode !== 'Swim') flowFocusedNodeId.set(null);
</script>

<div class="flow-view" data-testid="flow-view">
  <!-- Sub-mode tab strip -->
  <div class="flow-mode-strip">
    <button
      class="flow-mode-btn"
      class:active={$flowMode === 'Swim'}
      aria-pressed={$flowMode === 'Swim'}
      on:click={() => setMode('Swim')}
      data-testid="flow-mode-btn-swim"
    >
      <Icon name="swim" size={12}/>
      Swim
    </button>
    <button
      class="flow-mode-btn"
      class:active={$flowMode === 'Flowchart'}
      aria-pressed={$flowMode === 'Flowchart'}
      on:click={() => setMode('Flowchart')}
      data-testid="flow-mode-btn-flowchart"
    >
      Flowchart
    </button>
    <button
      class="flow-mode-btn"
      class:active={$flowMode === 'Code'}
      aria-pressed={$flowMode === 'Code'}
      on:click={() => setMode('Code')}
      data-testid="flow-mode-btn-code"
    >
      <Icon name="code" size={12}/>
      Code
    </button>
    {#if $flowMode === 'Swim'}
      <div class="flow-mode-sep" aria-hidden="true"></div>
      <button
        class="flow-mode-btn flow-focus-btn"
        class:active={$flowFocusedNodeId !== null}
        aria-pressed={$flowFocusedNodeId !== null}
        on:click={() => {
          const focusedId = $flowFocusedNodeId;
          const selectedId = $flowSelectedNodeId;
          if (focusedId !== null) {
            flowFocusedNodeId.set(null);
          } else if (selectedId !== null) {
            flowFocusedNodeId.set(selectedId);
          }
        }}
        data-testid="flow-focus-btn"
      >Focus</button>
    {/if}
  </div>

  <!-- Content -->
  <div class="flow-body">
    {#if $flowMode === 'Swim'}
      <FlowSwim flowchart={flowchart ?? { nodes: [], edges: [] }} {fn} />
    {:else if $flowMode === 'Flowchart'}
      <FlowchartCanvas
        flowchart={flowchart ?? { nodes: [], edges: [] }}
        functionId={fn?.id ?? ''}
        {layoutOverrides}
      />
    {:else}
      <FlowCode {fn} {detail} />
    {/if}
    {#if $flowMode === 'Swim'}
      <FlowMinimap flowchart={flowchart} selectedId={$flowSelectedNodeId} />
    {/if}
  </div>
</div>

<style>
  .flow-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .flow-mode-strip {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 32px;
    padding: 0 10px;
    background: var(--surface-2);
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
  }

  .flow-mode-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 24px;
    padding: 0 10px;
    background: transparent;
    border: none;
    color: var(--ink-3);
    font-size: 12px;
    cursor: pointer;
    border-radius: var(--radius-sm);
  }

  .flow-mode-btn:hover {
    background: var(--surface-3);
    color: var(--ink);
  }

  .flow-mode-btn.active {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    color: var(--accent);
  }

  .flow-mode-sep {
    width: 1px;
    height: 14px;
    background: var(--line);
    margin: 0 4px;
    flex-shrink: 0;
  }

  .flow-body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
</style>
