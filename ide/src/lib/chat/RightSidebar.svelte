<script lang="ts">
  import { onMount } from 'svelte';
  import {
    sidebarCollapsed,
    sidebarActiveTab,
    sidebarSlots,
    sidebarHydrated,
    initSidebarState,
  } from './sidebar-state';
  import ChatPanel from './ChatPanel.svelte';
  import Icon from '$lib/icons/Icon.svelte';

  onMount(() => {
    initSidebarState();
  });

  $: sortedSlots = Object.values($sidebarSlots).sort((a, b) => a.order - b.order);

  // Fallback: if active tab no longer exists in slots and isn't 'chat', reset.
  // Gated on sidebarHydrated so pre-hydration reactive runs don't defeat
  // localStorage-restored tab values.
  $: if ($sidebarHydrated && $sidebarActiveTab !== 'chat' && !$sidebarSlots[$sidebarActiveTab]) {
    sidebarActiveTab.set('chat');
  }

  function selectChat() {
    sidebarActiveTab.set('chat');
    if ($sidebarCollapsed) sidebarCollapsed.set(false);
  }

  function selectSlot(id) {
    sidebarActiveTab.set(id);
    if ($sidebarCollapsed) sidebarCollapsed.set(false);
  }

  function toggleCollapsed() {
    sidebarCollapsed.update((v) => !v);
  }
</script>

<aside
  class="right-sidebar"
  class:collapsed={$sidebarCollapsed}
  data-testid="right-sidebar"
>
  {#if !$sidebarCollapsed}
    <div class="sb-content" data-testid="sb-content">
      {#if $sidebarActiveTab === 'chat'}
        <ChatPanel on:previewapply on:previewdismiss />
      {:else if $sidebarSlots[$sidebarActiveTab]}
        <svelte:component this={$sidebarSlots[$sidebarActiveTab].component} />
      {/if}
    </div>
  {/if}

  <nav class="sb-rail" aria-label="Sidebar tabs" data-testid="sb-rail">
    <button
      type="button"
      class="sb-rail-btn"
      class:active={$sidebarActiveTab === 'chat' && !$sidebarCollapsed}
      aria-pressed={$sidebarActiveTab === 'chat' && !$sidebarCollapsed}
      data-testid="sb-rail-btn-chat"
      title="Chat"
      on:click={selectChat}
    >
      <Icon name="sparkle" size={14} />
    </button>

    {#each sortedSlots as slot (slot.id)}
      <button
        type="button"
        class="sb-rail-btn"
        class:active={$sidebarActiveTab === slot.id && !$sidebarCollapsed}
        aria-pressed={$sidebarActiveTab === slot.id && !$sidebarCollapsed}
        data-testid="sb-rail-btn-{slot.id}"
        title={slot.label}
        on:click={() => selectSlot(slot.id)}
      >
        <Icon name={slot.icon} size={14} />
      </button>
    {/each}

    <div style="flex: 1"></div>

    <button
      type="button"
      class="sb-rail-btn"
      aria-pressed={$sidebarCollapsed}
      data-testid="sb-collapse-btn"
      title={$sidebarCollapsed ? 'Expand sidebar' : 'Collapse sidebar'}
      on:click={toggleCollapsed}
    >
      <Icon name={$sidebarCollapsed ? 'chevron-left' : 'chevron-right'} size={14} />
    </button>
  </nav>
</aside>
