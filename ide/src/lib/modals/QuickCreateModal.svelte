<script lang="ts">
  import { createEventDispatcher, onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { graph, quickCreateModalOpen, quickCreateNotice } from '$lib/stores';
  import { focusTrap } from './focus-trap';

  // v4.0: when a project is open, scaffolding a fresh project rời rạc is
  // misleading. The modal switches into "Add to {project}" mode — kinds
  // narrow to additive shapes only, the raw Create button is disabled, and
  // Create with AI is the only ship-truthful path.
  const KINDS_DEFAULT = ['module', 'function', 'rule', 'test'];
  const KINDS_ADD = ['function', 'type', 'error'];

  $: openProject = $graph?.project ?? null;
  $: hasOpenProject = openProject !== null;
  $: kinds = hasOpenProject ? KINDS_ADD : KINDS_DEFAULT;

  // Component-local form state — reset on every close
  let kind = 'module';
  // Re-anchor `kind` whenever the kind set narrows past the current value
  // (e.g. user opens a project after selecting "module", which is no longer
  // available). Defaults to first available kind for the current mode.
  $: if (!kinds.includes(kind)) kind = kinds[0];
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
    // Reset `kind` to the first option available for the *current* mode so a
    // re-open never lands on a button that has just been narrowed away.
    kind = (hasOpenProject ? KINDS_ADD : KINDS_DEFAULT)[0];
    // Clear the notice so a stale "unavailable in browser preview" message
    // doesn't flash on the next re-open. Mirror of WelcomeModal.close().
    quickCreateNotice.set('');
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
        <span class="modal-title" data-testid="qc-title">
          {#if hasOpenProject}
            Add to {openProject?.name ?? 'project'}
          {:else}
            Quick Create
          {/if}
        </span>
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
          {#each kinds as k}
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
            on:keydown={(e) => {
              // Production-grade form ergonomics: Enter in the name field
              // submits the default action so the user doesn't have to reach
              // for the mouse. The form is a `<div>` (not a real `<form>`),
              // so we wire this manually. v4.0: when a project is open the
              // raw Create button is disabled, so Enter routes to Create
              // with AI instead.
              if (e.key === 'Enter') {
                e.preventDefault();
                if (hasOpenProject) handleCreateAI();
                else handleCreate();
              }
            }}
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
        <button
          data-testid="qc-create"
          on:click={handleCreate}
          disabled={hasOpenProject}
          title={hasOpenProject
            ? "Direct creation arrives in v4.1 — use 'Create with AI' for now"
            : ''}
        >Create</button>
        <button
          class="primary"
          data-testid="qc-create-ai"
          on:click={handleCreateAI}
        >
          ✦ Create with AI
        </button>
      </div>

      {#if $quickCreateNotice}
        <p
          class="qc-notice"
          data-testid="qc-notice"
          role="status"
          aria-live="polite"
        >
          {$quickCreateNotice}
        </p>
      {/if}
    </div>
  </div>
{/if}
