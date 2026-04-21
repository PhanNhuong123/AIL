<script lang="ts">
  import type { NodeDetail } from '$lib/types';

  export let detail = null as NodeDetail | null;
  export let breadcrumbPath = [] as string[];

  $: partOf = breadcrumbPath
    .slice(0, -1)
    .map((seg) => {
      const s = seg as string;
      const idx = s.indexOf(':');
      return idx >= 0 ? s.slice(idx + 1) : s;
    })
    .join(' › ');

  $: verified = detail?.verification?.ok;
  $: statusLabel =
    verified === true
      ? '✓ Verified'
      : verified === false
        ? '✗ Verification failed'
        : '⚠ Unproven';
  $: statusClass =
    verified === true ? 'status-ok' : verified === false ? 'status-fail' : 'status-warn';

  $: description = detail?.description ?? '';
  $: receives = detail?.receives ?? [];
  $: returns = detail?.returns ?? [];
</script>

<div class="ctx-overview" data-testid="ctx-overview">
  <!-- Description -->
  <div class="ctx-section" data-testid="ctx-section-description">
    <div class="ctx-section-label">Description</div>
    <div class="ctx-description" data-testid="ctx-description">
      {description || '—'}
    </div>
  </div>

  <!-- Part Of -->
  <div class="ctx-section" data-testid="ctx-section-partof">
    <div class="ctx-section-label">Part Of</div>
    <div class="ctx-partof ctx-breadcrumb" data-testid="ctx-partof">
      {partOf || '—'}
    </div>
  </div>

  <!-- Receives -->
  {#if receives.length > 0}
    <div class="ctx-section" data-testid="ctx-section-receives">
      <div class="ctx-section-label">Receives</div>
      {#each receives as param}
        <div class="ctx-param-row">
          <span class="ctx-param-name">{param.name}</span>
          <span class="ctx-param-sep"> — </span>
          <span class="ctx-param-desc">{param.desc}</span>
        </div>
      {/each}
    </div>
  {/if}

  <!-- Returns -->
  {#if returns.length > 0}
    <div class="ctx-section" data-testid="ctx-section-returns">
      <div class="ctx-section-label">Returns</div>
      {#each returns as param}
        <div class="ctx-param-row">
          <span class="ctx-param-name">{param.name}</span>
          <span class="ctx-param-sep"> — </span>
          <span class="ctx-param-desc">{param.desc}</span>
        </div>
      {/each}
    </div>
  {/if}

  <!-- Status -->
  <div class="ctx-section" data-testid="ctx-section-status">
    <div class="ctx-section-label">Status</div>
    <span class="ctx-status-text {statusClass}" data-testid="ctx-status-text">
      {statusLabel}
    </span>
  </div>
</div>
