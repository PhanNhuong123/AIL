<script lang="ts">
  import type { IconName } from './icon-types';

  export let name = '' as IconName;
  export let size = 14;
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 16 16"
  fill="none"
  stroke="currentColor"
  stroke-width="1.5"
  stroke-linecap="round"
  stroke-linejoin="round"
  data-testid="icon-{name}"
  aria-hidden="true"
>
  {#if name === 'project'}
    <path d="M1 4a1 1 0 011-1h4l1.5 2H14a1 1 0 011 1v7a1 1 0 01-1 1H2a1 1 0 01-1-1V4z"/>
  {:else if name === 'module'}
    <rect x="2" y="2" width="12" height="12" rx="1"/>
    <path d="M2 6h12"/>
    <path d="M6 2v4"/>
    <path d="M10 2v4"/>
  {:else if name === 'function'}
    <path d="M5 3c0-1 1-2 3-1v2c-1 0-2 0-2 1v1h2v2H6v6H4V8H3V6h1V5a2 2 0 011-2z"/>
    <path d="M10 6h2a1 1 0 011 1v1h-2V7h-1V6z"/>
    <path d="M10 9h3"/>
    <path d="M10 12h2"/>
  {:else if name === 'step'}
    <circle cx="8" cy="8" r="6"/>
    <path d="M6 8h4M8 6l2 2-2 2"/>
  {:else if name === 'type'}
    <path d="M2 4h12"/>
    <path d="M8 4v9"/>
  {:else if name === 'error'}
    <path d="M8 2L14 13H2L8 2z"/>
    <path d="M8 7v3"/>
    <circle cx="8" cy="11.5" r="0.5" fill="currentColor" stroke="none"/>
  {:else if name === 'sequence'}
    <path d="M3 8h10"/>
    <path d="M9 4l4 4-4 4"/>
  {:else if name === 'async'}
    <path d="M3 8h3M8 8h2M12 8h1" stroke-dasharray="2 1"/>
    <path d="M9 4l4 4-4 4"/>
  {:else if name === 'rule'}
    <path d="M10 2L5 9h4l-1 5 6-7H10l1-5z"/>
  {:else if name === 'check'}
    <path d="M2 8l4 4 8-8"/>
  {:else if name === 'warn'}
    <path d="M8 2L14 13H2L8 2z"/>
    <path d="M8 6v4"/>
    <circle cx="8" cy="11.5" r="0.5" fill="currentColor" stroke="none"/>
  {:else if name === 'tweaks'}
    <path d="M4 4v8"/>
    <path d="M8 4v8"/>
    <path d="M12 4v8"/>
    <circle cx="4" cy="6" r="1.5" fill="var(--surface-2)" stroke="currentColor"/>
    <circle cx="8" cy="9" r="1.5" fill="var(--surface-2)" stroke="currentColor"/>
    <circle cx="12" cy="5" r="1.5" fill="var(--surface-2)" stroke="currentColor"/>
  {:else if name === 'chevron-right'}
    <path d="M6 4l4 4-4 4"/>
  {:else if name === 'chevron-down'}
    <path d="M4 6l4 4 4-4"/>
  {:else if name === 'chevron-left'}
    <path d="M10 4L6 8l4 4"/>
  {:else if name === 'minus'}
    <path d="M3 8h10"/>
  {:else if name === 'plus'}
    <path d="M8 3v10M3 8h10"/>
  {:else if name === 'data'}
    <path d="M8 2L4 8a4 4 0 108 0L8 2z"/>
  {:else if name === 'deps'}
    <path d="M5 9l1-1a2.5 2.5 0 003.5 0l1-1"/>
    <path d="M11 7l1-1a2.5 2.5 0 000-3.5 2.5 2.5 0 00-3.5 0L7 3.5"/>
    <path d="M5 9L9.5 4.5"/>
  {:else if name === 'tests'}
    <path d="M6 2v5L3 13h10L10 7V2"/>
    <path d="M5 2h6"/>
  {:else if name === 'palette'}
    <rect x="2" y="2" width="5" height="5" rx="1"/>
    <rect x="9" y="2" width="5" height="5" rx="1"/>
    <rect x="2" y="9" width="5" height="5" rx="1"/>
    <rect x="9" y="9" width="5" height="5" rx="1"/>
  {/if}
</svg>
