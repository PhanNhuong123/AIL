<script lang="ts">
  import Icon from '$lib/icons/Icon.svelte';
  import type { Status } from '$lib/types';
  import type { IconName } from '$lib/icons/icon-types';

  export let kind = '' as string;
  export let name = '' as string;
  export let status = 'ok' as Status;
  export let depth = 0;
  export let hasChildren = false;
  export let expanded = false;
  export let selected = false;
  export let onClick = undefined as ((() => void) | undefined);
  export let onToggle = undefined as ((() => void) | undefined);

  const iconMap = {
    project: 'project',
    module: 'module',
    function: 'function',
    step: 'step',
    type: 'type',
    error: 'error',
  } as Record<string, IconName>;

  $: iconName = (iconMap[kind] ?? 'step') as IconName;

  function handleRowClick() {
    if (onClick) onClick();
  }

  function handleToggleClick(e) {
    e.stopPropagation();
    if (onToggle) onToggle();
  }
</script>

<div
  class="nav-row"
  class:selected
  style="padding-left: calc(8px + {depth} * 14px);"
  role="treeitem"
  aria-selected={selected}
  aria-expanded={hasChildren ? expanded : undefined}
  tabindex="0"
  on:click={handleRowClick}
  on:keydown={(e) => { if (e.key === 'Enter' || e.key === ' ') handleRowClick(); }}
>
  <span
    class="chevron"
    class:hidden={!hasChildren}
    on:click={handleToggleClick}
    on:keydown={(e) => { if (e.key === 'Enter') handleToggleClick(e); }}
    role="button"
    tabindex="-1"
    aria-label={expanded ? 'Collapse' : 'Expand'}
  >
    <Icon name={expanded ? 'chevron-down' : 'chevron-right'} size={10} />
  </span>

  <span class="row-icon">
    <Icon name={iconName} size={13} />
  </span>

  <span class="row-label">{name}</span>

  <span class="dot dot-{status}" aria-label="status: {status}"></span>
</div>

<style>
  .nav-row {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 26px;
    cursor: pointer;
    border-radius: var(--radius-sm);
    color: var(--ink-2);
    font-size: 12px;
    user-select: none;
    padding-right: 8px;
  }

  .nav-row:hover {
    background: var(--surface-3);
    color: var(--ink);
  }

  .nav-row.selected {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    color: var(--ink);
  }

  .chevron {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    color: var(--ink-3);
  }

  .chevron.hidden {
    visibility: hidden;
  }

  .row-icon {
    display: flex;
    align-items: center;
    flex-shrink: 0;
    color: var(--ink-3);
  }

  .row-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .dot-ok   { background: var(--ok); }
  .dot-warn { background: var(--warn); }
  .dot-fail { background: var(--fail); }
</style>
