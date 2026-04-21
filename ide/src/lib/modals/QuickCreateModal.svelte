<script lang="ts">
  import { quickCreateModalOpen } from '$lib/stores';

  // Component-local form state — reset on every close
  let name = '';
  let description = '';

  // ---------------------------------------------------------------------------
  // Handlers
  // ---------------------------------------------------------------------------

  function close() {
    name = '';
    description = '';
    quickCreateModalOpen.set(false);
  }

  function handleCancel() {
    console.info('[phase-17-stub] quick-create: cancel');
    close();
  }

  function handleCreate() {
    console.info('[phase-17-stub] quick-create: create', { name, description });
    close();
  }

  function handleCreateAI() {
    console.info('[phase-17-stub] quick-create: create-with-ai', { name, description });
    close();
  }

  function handleBackdrop(e) {
    if (e.target === e.currentTarget) {
      close();
    }
  }
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
    <div class="modal-dialog qc" on:click|stopPropagation>
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
        <label>
          Name
          <input
            type="text"
            data-testid="qc-name"
            bind:value={name}
            placeholder="Node or module name"
          />
        </label>
        <label>
          Description
          <textarea
            data-testid="qc-description"
            bind:value={description}
            placeholder="What it does"
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
