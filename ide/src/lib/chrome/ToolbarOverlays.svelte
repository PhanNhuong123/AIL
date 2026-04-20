<script lang="ts">
  import { overlays } from '$lib/stores';
  import Icon from '$lib/icons/Icon.svelte';
  import type { IconName } from '$lib/icons/icon-types';

  const OVERLAY_CONFIG = [
    { key: 'rules',        label: 'Rules',  icon: 'rule'  },
    { key: 'verification', label: 'Verify', icon: 'check' },
    { key: 'dataflow',     label: 'Data',   icon: 'data'  },
    { key: 'dependencies', label: 'Deps',   icon: 'deps'  },
    { key: 'tests',        label: 'Tests',  icon: 'tests' },
  ] as const;

  type OverlayKey = 'rules' | 'verification' | 'dataflow' | 'dependencies' | 'tests';

  function toggle(key) {
    const k = key as OverlayKey;
    overlays.update(o => ({ ...o, [k]: !o[k] }));
  }
</script>

<div class="overlay-group" aria-label="Overlay toggles">
  {#each OVERLAY_CONFIG as cfg}
    <button
      class="toolbar-btn"
      class:active={$overlays[cfg.key]}
      data-testid="toolbar-overlay-{cfg.key}"
      aria-pressed={$overlays[cfg.key]}
      aria-label={cfg.label}
      title={cfg.label}
      on:click={() => toggle(cfg.key)}
    >
      <Icon name={cfg.icon as IconName} size={13} />
      <span class="btn-label">{cfg.label}</span>
    </button>
  {/each}
</div>

<style>
  .overlay-group {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .toolbar-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 26px;
    padding: 0 8px;
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--ink-3);
    font-size: 12px;
    cursor: pointer;
    white-space: nowrap;
  }

  .toolbar-btn:hover {
    background: var(--surface-3);
    color: var(--ink);
  }

  .toolbar-btn.active {
    background: color-mix(in srgb, var(--accent) 20%, transparent);
    color: var(--accent);
  }

  .btn-label {
    font-size: 11px;
  }
</style>
