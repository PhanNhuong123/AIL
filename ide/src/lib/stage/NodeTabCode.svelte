<script lang="ts">
  import type { NodeDetail } from '$lib/types';
  import { nodeCodeLang } from './node-view-state';

  export let detail = null as NodeDetail | null;

  $: code = $nodeCodeLang === 'python'
    ? (detail?.code?.python ?? '')
    : (detail?.code?.typescript ?? '');

  $: hasTs = !!(detail?.code?.typescript);
  $: hasPy = !!(detail?.code?.python);
</script>

<div class="node-tab-code" data-testid="node-tab-code">
  <div class="node-tab-code-lang" role="group" aria-label="Language">
    <button
      class="lang-btn"
      class:active={$nodeCodeLang === 'python'}
      disabled={!hasPy}
      on:click={() => { nodeCodeLang.set('python'); }}
      data-testid="node-code-lang-py"
    >py</button>
    <button
      class="lang-btn"
      class:active={$nodeCodeLang === 'typescript'}
      disabled={!hasTs}
      on:click={() => { nodeCodeLang.set('typescript'); }}
      data-testid="node-code-lang-ts"
    >ts</button>
  </div>

  {#if code}
    <div class="node-tab-code-wrap">
      <pre><code
        class="language-{$nodeCodeLang}"
        data-testid="node-tab-code-text"
      >{code}</code></pre>
    </div>
  {:else}
    <div class="node-tab-empty">No code blob for this node.</div>
  {/if}
</div>

<style>
  .node-tab-code {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .node-tab-code-lang {
    display: flex;
    gap: 2px;
  }

  .lang-btn {
    height: 22px;
    padding: 0 8px;
    background: transparent;
    border: 1px solid var(--line);
    color: var(--ink-3);
    font-size: 11px;
    cursor: pointer;
    border-radius: var(--radius-sm);
  }

  .lang-btn.active {
    background: var(--accent);
    color: var(--surface);
    border-color: var(--accent);
  }

  .lang-btn:disabled { opacity: 0.4; cursor: not-allowed; }

  .node-tab-empty { font-size: 12px; color: var(--ink-3); }
</style>
