<script lang="ts">
  import { createEventDispatcher, onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { quickCreateModalOpen } from '$lib/stores';
  import { focusTrap } from './focus-trap';

  // Component-local form state — reset on every close
  let kind = 'module';
  const KINDS = ['module', 'function', 'rule', 'test'];
  let name = '';
  let description = '';

  // The component dispatches `create` / `createAi` events with the form
  // payload `{ kind, name, description }`. The route shell (`+page.svelte`)
  // owns scaffolding/agent calls because modals must NOT import
  // `$lib/bridge.ts` (modals/CLAUDE.md). `cancel` is also dispatched so
  // consumers can react (e.g. test snapshotting).
  // Note: untyped `createEventDispatcher()` because esrap rejects generic
  // type annotations on it (invariant 16.2-E).
  const dispatch = createEventDispatcher();

  // ---------------------------------------------------------------------------
  // Handlers
  // ---------------------------------------------------------------------------

  function close() {
    name = '';
    description = '';
    kind = 'module';
    quickCreateModalOpen.set(false);
  }

  // Pass `{ bubbles: true }` on every dispatch so DOM-level
  // `addEventListener` ancestors (and tests) catch the event — Svelte 5's
  // `createEventDispatcher` defaults to non-bubbling.
  // The `as never` cast bypasses the incomplete `DispatchOptions` TS type
  // (it omits `bubbles` though the runtime supports it).
  const BUBBLE = { bubbles: true } as never;

  function snapshot() {
    return { kind, name, description };
  }

  function handleCancel() {
    dispatch('cancel', undefined, BUBBLE);
    close();
  }

  function handleCreate() {
    dispatch('create', snapshot(), BUBBLE);
  }

  function handleCreateAI() {
    dispatch('createAi', snapshot(), BUBBLE);
  }

  function handleBackdrop(e) {
    if (e.target === e.currentTarget) {
      close();
    }
  }

  // ESC closes the Quick Create modal. The listener is attached for the
  // component lifetime and gates on the open store so ESC is a no-op when the
  // modal is hidden. Compatible with invariant 15.11-B: the listener triggers
  // only the close path, never the open path. Cmd/Ctrl+K open shortcut stays
  // owned by TitleBar.svelte::openQuickCreate (15.3-B).
  onMount(() => {
    function handleKey(e) {
      if (e.key === 'Escape' && get(quickCreateModalOpen)) close();
    }
    if (typeof document !== 'undefined') {
      document.addEventListener('keydown', handleKey);
    }
    return () => {
      if (typeof document !== 'undefined') {
        document.removeEventListener('keydown', handleKey);
      }
    };
  });
</script>

{#if $quickCreateModalOpen}
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
  <div
    class="modal-backdrop"
    data-testid="qc-backdrop"
    on:click={handleBackdrop}
    role="dialog"
    aria-modal="true"
    aria-label="Quick Create"
  >
    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div
      class="modal-dialog qc"
      tabindex="-1"
      use:focusTrap
      on:click|stopPropagation
    >
      <header class="modal-header">
        <span class="modal-title">Quick Create</span>
        <button
          class="modal-close-btn"
          data-testid="qc-close"
          on:click={close}
          aria-label="Close"
        >
          ✕
        </button>
      </header>

      <div class="qc-form">
        <div class="qc-kind-row" data-testid="qc-kind-row" role="group" aria-label="Kind">
          {#each KINDS as k}
            <button
              type="button"
              class="qc-kind-btn"
              class:active={kind === k}
              aria-pressed={kind === k}
              data-testid="qc-kind-btn-{k}"
              on:click={() => (kind = k)}
            >{k}</button>
          {/each}
        </div>
        <label for="qc-name-input">
          Name
          <input
            id="qc-name-input"
            type="text"
            data-testid="qc-name"
            bind:value={name}
            placeholder="Node or module name"
            aria-label="Name"
          />
        </label>
        <label for="qc-description-input">
          Description
          <textarea
            id="qc-description-input"
            data-testid="qc-description"
            bind:value={description}
            placeholder="What it does"
            aria-label="Description"
          ></textarea>
        </label>
      </div>

      <div class="qc-actions">
        <button data-testid="qc-cancel" on:click={handleCancel}>Cancel</button>
        <button data-testid="qc-create" on:click={handleCreate}>Create</button>
        <button
          class="primary"
          data-testid="qc-create-ai"
          on:click={handleCreateAI}
        >
          ✦ Create with AI
        </button>
      </div>
    </div>
  </div>
{/if}
