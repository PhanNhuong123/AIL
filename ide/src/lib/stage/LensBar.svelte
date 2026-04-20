<script lang="ts">
  import type { Bar } from './lens';

  export let bar = { kind: 'none' } as Bar;

  function pctOf(n, total) {
    const a = n as number;
    const b = total as number;
    if (b <= 0) return 0;
    return Math.round((a / b) * 100);
  }
</script>

{#if bar.kind === 'seg'}
  {@const total = bar.proven + bar.unproven + bar.broken}
  <div class="bar bar-seg" data-testid="bar-seg">
    {#if total > 0}
      <span
        class="bar-seg-proven"
        style="flex-basis: {pctOf(bar.proven, total)}%"
      ></span>
      <span
        class="bar-seg-unproven"
        style="flex-basis: {pctOf(bar.unproven, total)}%"
      ></span>
      <span
        class="bar-seg-broken"
        style="flex-basis: {pctOf(bar.broken, total)}%"
      ></span>
    {/if}
  </div>
{:else if bar.kind === 'dots'}
  <div class="bar bar-dots" data-testid="bar-dots">
    {#each bar.statuses as s}
      <span
        class="bar-dot dot-{s === 'fail' ? 'fail' : s === 'warn' ? 'warn' : 'ok'}"
        aria-hidden="true"
      ></span>
    {/each}
  </div>
{:else if bar.kind === 'types'}
  <div class="bar bar-types" data-testid="bar-types">
    {#each bar.names as n}
      <span class="bar-type-chip">{n}</span>
    {/each}
  </div>
{:else}
  <div class="bar bar-none" data-testid="bar-none"></div>
{/if}
