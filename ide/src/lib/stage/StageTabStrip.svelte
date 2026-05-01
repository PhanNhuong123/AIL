<script lang="ts">
  import Icon from '$lib/icons/Icon.svelte';
</script>

<div class="stage-tabstrip" data-testid="stage-tabstrip" role="tablist">
  <button
    class="stage-tab active"
    data-testid="stage-tab-canvas"
    role="tab"
    aria-selected="true"
    type="button"
  >
    Canvas
  </button>
  <button
    class="stage-tab-add"
    data-testid="stage-tab-add"
    aria-label="New tab (coming in v5)"
    title="Multi-tab editing arrives in v5"
    aria-disabled="true"
    disabled
    type="button"
  >
    <Icon name="plus-circle" size={12} />
  </button>
</div>
