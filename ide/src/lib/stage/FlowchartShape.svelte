<script lang="ts">
  import type { FlowNodeKind, Status } from '$lib/types';

  export let kind     = '' as FlowNodeKind;
  export let id       = '';
  export let label    = '';
  export let x        = 0;
  export let y        = 0;
  export let w        = 120;
  export let h        = 48;
  export let status   = undefined as Status | undefined;
  export let selected = false;

  $: cx = x + w / 2;
  $: cy = y + h / 2;
  $: rx = w / 2;
  $: ry = h / 2;

  $: statusClass = status === 'fail' ? 'shape-fail'
                 : status === 'warn' ? 'shape-warn'
                 : 'shape-ok';
</script>

<g
  data-testid="shape-{kind}-{id}"
  class="flowchart-shape {statusClass}"
  class:selected
  role="presentation"
>
  {#if kind === 'start' || kind === 'end'}
    <ellipse {cx} {cy} {rx} {ry} class="shape-fill shape-stroke"/>
    <text x={cx} y={cy + 1} dominant-baseline="middle" text-anchor="middle" class="shape-label">{label}</text>

  {:else if kind === 'process'}
    <rect {x} {y} width={w} height={h} rx="6" class="shape-fill shape-stroke"/>
    <text x={cx} y={cy + 1} dominant-baseline="middle" text-anchor="middle" class="shape-label">{label}</text>

  {:else if kind === 'decision'}
    <polygon
      points="{cx},{y} {x + w},{cy} {cx},{y + h} {x},{cy}"
      class="shape-fill shape-stroke"
    />
    <text x={cx} y={cy + 1} dominant-baseline="middle" text-anchor="middle" class="shape-label">{label}</text>

  {:else if kind === 'io'}
    <polygon
      points="{x + 12},{y} {x + w},{y} {x + w - 12},{y + h} {x},{y + h}"
      class="shape-fill shape-stroke"
    />
    <text x={cx} y={cy + 1} dominant-baseline="middle" text-anchor="middle" class="shape-label">{label}</text>

  {:else if kind === 'sub'}
    <rect {x} {y} width={w} height={h} rx="2" class="shape-fill shape-stroke"/>
    <line x1={x + 12} y1={y} x2={x + 12} y2={y + h} class="shape-stroke-inner"/>
    <line x1={x + w - 12} y1={y} x2={x + w - 12} y2={y + h} class="shape-stroke-inner"/>
    <text x={cx} y={cy + 1} dominant-baseline="middle" text-anchor="middle" class="shape-label">{label}</text>

  {:else if kind === 'storage'}
    <ellipse cx={cx} cy={y + 8}       rx={rx} ry={8}  class="shape-fill shape-stroke"/>
    <rect    x={x}   y={y + 8}  width={w} height={h - 16} class="shape-fill shape-stroke-sides"/>
    <ellipse cx={cx} cy={y + h - 8}   rx={rx} ry={8}  class="shape-fill shape-stroke"/>
    <line x1={x}     y1={y + 8} x2={x}     y2={y + h - 8} class="shape-stroke-inner"/>
    <line x1={x + w} y1={y + 8} x2={x + w} y2={y + h - 8} class="shape-stroke-inner"/>
    <text x={cx} y={cy + 1} dominant-baseline="middle" text-anchor="middle" class="shape-label">{label}</text>
  {/if}
</g>

<style>
  .shape-fill         { fill: var(--surface-2); }
  .shape-stroke       { stroke: var(--ink-3); stroke-width: 1.5px; }
  .shape-stroke-inner { stroke: var(--ink-3); stroke-width: 1px; fill: none; }
  .shape-stroke-sides { stroke: none; fill: var(--surface-2); }
  .shape-label        {
    font-size: 11px;
    fill: var(--ink);
    pointer-events: none;
  }

  .selected .shape-fill   { fill: color-mix(in srgb, var(--accent) 18%, var(--surface-2)); }
  .selected .shape-stroke { stroke: var(--accent); }

  .shape-fail .shape-stroke { stroke: var(--fail); }
  .shape-warn .shape-stroke { stroke: var(--warn); }
</style>
