<script lang="ts">
  import { nodeTestResult, runTestStub } from './node-view-state';

  export let stepId = '' as string;

  function handleRun() {
    runTestStub(stepId);
  }
</script>

<div class="node-tab-test" data-testid="node-tab-test">
  <button
    class="node-tab-test-btn"
    on:click={handleRun}
    data-testid="node-tab-test-run-btn"
  >
    Run
  </button>

  {#if $nodeTestResult !== null}
    <div
      class="node-tab-test-result {$nodeTestResult.passed ? 'test-result-pass' : 'test-result-fail'}"
      data-testid="node-tab-test-result"
    >
      <div>{$nodeTestResult.message}</div>
      <div class="test-result-duration">{$nodeTestResult.durationMs} ms</div>
    </div>
  {:else}
    <div class="node-tab-empty" data-testid="node-tab-test-empty">
      Click Run to execute assertions for this node.
    </div>
  {/if}
</div>

<style>
  .node-tab-test { display: flex; flex-direction: column; gap: 10px; }
  .node-tab-empty { font-size: 12px; color: var(--ink-3); }
  .test-result-duration { font-size: 10px; opacity: 0.7; margin-top: 2px; }
</style>
