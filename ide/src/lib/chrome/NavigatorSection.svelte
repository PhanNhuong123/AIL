<script lang="ts">
  export let title = '' as string;
</script>

<div class="nav-section">
  <div class="nav-section-header">{title}</div>
  <slot />
</div>

<style>
  .nav-section {
    margin-top: 4px;
  }

  .nav-section-header {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.06em;
    color: var(--ink-3);
    padding: 6px 8px 2px 8px;
    text-transform: uppercase;
    user-select: none;
  }
</style>
