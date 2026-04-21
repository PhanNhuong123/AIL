<script lang="ts">
  import { contextTestScenario, contextTestResult, runContextTestStub } from './context-panel-state';

  export let nodeId = '' as string;

  function handleGenerate() {
    // Phase 16 stub — Phase 17 will wire to real data generator.
    console.log('Generate test data stub for:', nodeId);
  }
</script>

<div class="ctx-test" data-testid="ctx-tab-test">
  <div class="ctx-section-label">Scenario</div>

  <textarea
    class="ctx-test-scenario"
    bind:value={$contextTestScenario}
    rows="4"
    placeholder="Describe the test scenario..."
    data-testid="ctx-test-scenario"
  ></textarea>

  <div class="ctx-test-actions">
    <button
      class="ctx-test-btn ctx-test-btn-secondary"
      on:click={handleGenerate}
      data-testid="ctx-test-generate-btn"
    >
      Generate data
    </button>
    <button
      class="ctx-test-btn ctx-test-btn-primary"
      on:click={() => runContextTestStub(nodeId)}
      data-testid="ctx-test-run-btn"
    >
      Run
    </button>
  </div>

  {#if $contextTestResult !== null}
    <div
      class="ctx-test-result {$contextTestResult.passed ? 'ctx-result-pass' : 'ctx-result-fail'}"
      data-testid="ctx-test-result"
    >
      {$contextTestResult.message}
    </div>
  {:else}
    <div class="ctx-test-result-placeholder" data-testid="ctx-test-result-placeholder">
      Result will appear here after Run.
    </div>
  {/if}
</div>
