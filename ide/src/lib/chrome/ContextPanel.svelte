<script lang="ts">
  import { selection, graph, path } from '$lib/stores';
  import type { NodeDetail } from '$lib/types';
  import type { IconName } from '$lib/icons/icon-types';
  import Icon from '$lib/icons/Icon.svelte';
  import {
    contextActiveTab,
    resetContextTest,
    type ContextTab,
  } from './context-panel-state';
  import ContextTabOverview from './ContextTabOverview.svelte';
  import ContextTabRules from './ContextTabRules.svelte';
  import ContextTabTest from './ContextTabTest.svelte';

  // ---------------------------------------------------------------------------
  // Reactive derivations
  // ---------------------------------------------------------------------------

  $: hasSelection = $selection.kind !== 'none' && $selection.id !== null;
  $: selectionId = $selection.id ?? '';
  $: detail = ($graph?.detail?.[selectionId] ?? null) as NodeDetail | null;
  $: nodeName = detail?.name ?? selectionId;

  // ---------------------------------------------------------------------------
  // Selection-change reset (invariant: resets test, NOT active tab)
  // ---------------------------------------------------------------------------

  let previousSelectionId = null as string | null;
  $: {
    if ($selection.id !== previousSelectionId) {
      previousSelectionId = $selection.id;
      resetContextTest();
    }
  }

  // ---------------------------------------------------------------------------
  // Handlers
  // ---------------------------------------------------------------------------

  function handleClose() {
    selection.set({ kind: 'none', id: null });
    path.set([]);
  }

  function handleEdit() {
    // Phase 17 stub — editor integration not yet wired.
  }

  function handleTest() {
    contextActiveTab.set('test' as ContextTab);
  }

  function handleCode() {
    // Phase 17 stub — code navigation not yet wired.
  }
</script>

<aside class="region-context" data-testid="context-panel">
  {#if hasSelection}
    <!-- Header -->
    <div class="ctx-header" data-testid="ctx-header">
      <span class="ctx-header-name" data-testid="ctx-header-name">{nodeName}</span>
      <button
        class="ctx-close-btn"
        on:click={handleClose}
        data-testid="ctx-close-btn"
        aria-label="Close"
      >
        <Icon name={'close' as IconName} size={12} />
      </button>
    </div>

    <!-- Tab strip -->
    <div class="ctx-tabs" data-testid="ctx-tabs">
      <button
        class="ctx-tab-btn"
        class:active={$contextActiveTab === 'overview'}
        on:click={() => contextActiveTab.set('overview' as ContextTab)}
        data-testid="ctx-tab-btn-overview"
      >Overview</button>
      <button
        class="ctx-tab-btn"
        class:active={$contextActiveTab === 'rules'}
        on:click={() => contextActiveTab.set('rules' as ContextTab)}
        data-testid="ctx-tab-btn-rules"
      >Rules</button>
      <button
        class="ctx-tab-btn"
        class:active={$contextActiveTab === 'test'}
        on:click={() => contextActiveTab.set('test' as ContextTab)}
        data-testid="ctx-tab-btn-test"
      >Test</button>
    </div>

    <!-- Tab body -->
    <div class="ctx-tab-body" data-testid="ctx-tab-body">
      {#if $contextActiveTab === 'overview'}
        <ContextTabOverview {detail} breadcrumbPath={$path} />
      {:else if $contextActiveTab === 'rules'}
        <ContextTabRules {detail} />
      {:else}
        <ContextTabTest nodeId={selectionId} />
      {/if}
    </div>

    <!-- Footer (flex-shrink: 0 — invariant 16.7-B) -->
    <div class="ctx-footer" data-testid="ctx-footer">
      <button class="ctx-footer-btn" on:click={handleEdit} data-testid="ctx-footer-edit">
        <Icon name={'edit' as IconName} size={12} /> Edit
      </button>
      <button class="ctx-footer-btn" on:click={handleTest} data-testid="ctx-footer-test">
        <Icon name={'tests' as IconName} size={12} /> Test
      </button>
      <button class="ctx-footer-btn" on:click={handleCode} data-testid="ctx-footer-code">
        <Icon name={'code' as IconName} size={12} /> Code
      </button>
    </div>
  {/if}
</aside>
