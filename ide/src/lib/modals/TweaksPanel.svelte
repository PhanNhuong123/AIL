<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { tweaksPanelOpen, theme, density } from '$lib/stores';
  import type { Density } from '$lib/stores';

  $: isLight = $theme === 'light';

  const shortcuts = [
    { key: '⌘K', label: 'Quick Create' },
    { key: '⌘P', label: 'Command Palette' },
    { key: '⌘1..4', label: 'Zoom level' },
    { key: '⌘[ / ⌘]', label: 'Navigate back/forward' },
    { key: 'Esc', label: 'Close modal' },
  ];

  let panelEl = undefined as HTMLElement | undefined;

  function close() {
    tweaksPanelOpen.set(false);
  }

  function toggleTheme() {
    const next = isLight ? 'dark' : 'light';
    theme.set(next);
    if (typeof document !== 'undefined') {
      document.documentElement.classList.toggle('light', next === 'light');
    }
  }

  function setAccent(e) {
    const value = (e.currentTarget as HTMLInputElement).value;
    if (typeof document !== 'undefined') {
      document.documentElement.style.setProperty('--accent', value);
    }
  }

  function setDensity(next) {
    density.set(next as Density);
  }

  onMount(() => {
    function handleKey(e) {
      if (e.key === 'Escape' && get(tweaksPanelOpen)) close();
    }
    function handleOutsideClick(e) {
      if (!panelEl) return;
      const target = e.target as Node | null;
      if (!target) return;
      if (panelEl.contains(target)) return;
      // Skip when target is the TitleBar gear toggle so the gear acts as a
      // clean toggle (open → close) instead of close-then-reopen (15.12 B6 fix).
      const gear = (target as HTMLElement).closest?.('[data-testid="tweaks-toggle-btn"]');
      if (gear) return;
      close();
    }
    if (typeof document !== 'undefined') {
      document.addEventListener('keydown', handleKey);
      document.addEventListener('mousedown', handleOutsideClick);
    }
    return () => {
      if (typeof document !== 'undefined') {
        document.removeEventListener('keydown', handleKey);
        document.removeEventListener('mousedown', handleOutsideClick);
      }
    };
  });
</script>

{#if $tweaksPanelOpen}
  <aside
    class="tweaks-floating"
    data-testid="tweaks-panel"
    bind:this={panelEl}
    role="region"
    aria-label="Tweaks"
  >
    <header class="modal-header">
      <span class="modal-title">Tweaks</span>
      <button
        type="button"
        class="modal-close-btn"
        data-testid="tweaks-close"
        on:click={close}
        aria-label="Close"
      >
        ✕
      </button>
    </header>

    <div class="tweaks-body">
      <div class="tweaks-section">
        <span class="tweaks-section-label">Theme</span>
        <button
          type="button"
          class="tweaks-theme-btn"
          data-testid="tweaks-theme-toggle"
          on:click={toggleTheme}
        >
          {isLight ? 'Light — switch to Dark' : 'Dark — switch to Light'}
        </button>
      </div>

      <div class="tweaks-section">
        <span class="tweaks-section-label">Accent colour</span>
        <div class="tweaks-accent-row">
          <input
            type="color"
            data-testid="tweaks-accent-input"
            value="#2997ff"
            on:input={setAccent}
          />
          <span class="tweaks-hint">Click to pick</span>
        </div>
      </div>

      <div class="tweaks-section">
        <span class="tweaks-section-label">Density</span>
        <div class="tweaks-segmented">
          <button
            type="button"
            class:active={$density === 'compact'}
            data-testid="tweaks-density-compact"
            on:click={() => setDensity('compact')}
          >
            Compact
          </button>
          <button
            type="button"
            class:active={$density === 'cozy'}
            data-testid="tweaks-density-cozy"
            on:click={() => setDensity('cozy')}
          >
            Cozy
          </button>
          <button
            type="button"
            class:active={$density === 'comfortable'}
            data-testid="tweaks-density-comfortable"
            on:click={() => setDensity('comfortable')}
          >
            Comfy
          </button>
        </div>
      </div>

      <div class="tweaks-section">
        <span class="tweaks-section-label">Shortcuts</span>
        <ul class="tweaks-shortcuts">
          {#each shortcuts as s}
            <li>
              <span>{s.label}</span>
              <kbd>{s.key}</kbd>
            </li>
          {/each}
        </ul>
      </div>
    </div>
  </aside>
{/if}
