<script lang="ts">
  // ChatPanel.svelte — stub for always-on right Chat column.
  // Full implementation is task 15.10.
</script>

<aside class="region-chat" data-testid="chat-panel" aria-label="Chat">
  <div class="chat-placeholder">
    <span class="chat-placeholder-label">Chat</span>
    <span class="chat-placeholder-hint">Agent chat wires in task 15.10.</span>
  </div>
</aside>

<style>
  .chat-placeholder {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 8px;
  }
  .chat-placeholder-label { font-size: 13px; font-weight: 600; color: var(--ink-3); }
  .chat-placeholder-hint  { font-size: 11px; color: var(--ink-3); }
</style>
