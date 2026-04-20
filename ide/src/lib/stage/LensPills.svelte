<script lang="ts">
  import type { Pill } from './lens';

  export let pills = [] as Pill[];
</script>

{#each pills as p}
  <span
    class="pill pill-{p.tone}"
    data-testid={p.testid ? `pill-${p.testid}` : undefined}
  >{p.label}</span>
{/each}
