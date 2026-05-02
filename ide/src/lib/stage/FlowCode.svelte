<script lang="ts">
  import type { FunctionJson, NodeDetail } from '$lib/types';

  export let fn     = null as FunctionJson | null;
  export let detail = null as NodeDetail | null;

  let lang = 'python';

  $: code = lang === 'python'
    ? (detail?.code?.python ?? '')
    : (detail?.code?.typescript ?? '');

  $: hasCode = !!(detail?.code?.python || detail?.code?.typescript);
  $: hasTs   = !!(detail?.code?.typescript);
  $: hasPy   = !!(detail?.code?.python);
</script>

<div class="flow-code" data-testid="flow-code">
  <div class="flow-code-header">
    <span class="flow-code-fn-name">{fn?.name ?? '—'}</span>
    {#if hasCode}
      <div class="flow-code-lang-toggle" role="group" aria-label="Language">
        <button
          class="lang-btn"
          class:active={lang === 'python'}
          aria-pressed={lang === 'python'}
          on:click={() => { lang = 'python'; }}
          disabled={!hasPy}
        >py</button>
        <button
          class="lang-btn"
          class:active={lang === 'typescript'}
          aria-pressed={lang === 'typescript'}
          on:click={() => { lang = 'typescript'; }}
          disabled={!hasTs}
        >ts</button>
      </div>
    {/if}
  </div>

  {#if hasCode && code}
    <div class="flow-code-body">
      <pre><code class="language-{lang}" data-testid="flow-code-text">{code}</code></pre>
    </div>
  {:else}
    <div class="flow-code-empty">No code available for this function.</div>
  {/if}

  {#if detail?.description}
    <p class="flow-code-desc">{detail.description}</p>
  {/if}
</div>

<style>
  .flow-code {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 16px;
    gap: 12px;
  }

  .flow-code-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--line);
  }

  .flow-code-fn-name {
    font-size: 13px;
    font-weight: 600;
    color: var(--ink);
  }

  .flow-code-lang-toggle {
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

  .lang-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .flow-code-body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    background: var(--surface-2);
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
  }

  .flow-code-body pre {
    margin: 0;
    padding: 12px;
    overflow: auto;
  }

  .flow-code-body code {
    font-family: ui-monospace, Consolas, monospace;
    font-size: 12px;
    color: var(--ink);
    white-space: pre;
  }

  .flow-code-empty {
    color: var(--ink-3);
    font-size: 13px;
  }

  .flow-code-desc {
    font-size: 12px;
    color: var(--ink-2);
    margin: 0;
  }
</style>
