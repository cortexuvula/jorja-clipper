<script lang="ts">
  import type { Settings } from '$lib/types';

  let { open = $bindable(false), settings = $bindable(), onsave, oncancel } = $props();

  function save() {
    onsave?.(settings);
    open = false;
  }

  function cancel() {
    oncancel?.();
    open = false;
  }
</script>

{#if open}
  <div class="modal-backdrop" onclick={cancel} onkeydown={(e) => { if (e.key === 'Escape') cancel(); }} role="presentation">
    <div class="modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" tabindex="-1">
      <h2>Settings</h2>

      <div class="field">
        <label for="buffer-before">Buffer Before (seconds)</label>
        <input
          id="buffer-before"
          type="number"
          step="0.5"
          bind:value={settings.buffer_before}
        />
      </div>

      <div class="field">
        <label for="buffer-after">Buffer After (seconds)</label>
        <input
          id="buffer-after"
          type="number"
          step="0.5"
          bind:value={settings.buffer_after}
        />
      </div>

      <div class="field">
        <label for="clip-key">Clip Hotkey</label>
        <input
          id="clip-key"
          type="text"
          bind:value={settings.clip_key}
        />
      </div>

      <div class="actions">
        <button onclick={cancel}>Cancel</button>
        <button onclick={save}>Save</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal {
    background: #1a1a2e;
    padding: 2rem;
    border-radius: 8px;
    min-width: 400px;
  }

  h2 {
    margin-bottom: 1.5rem;
    color: #e94560;
  }

  .field {
    margin-bottom: 1rem;
  }

  label {
    display: block;
    margin-bottom: 0.5rem;
    color: #e0e0e0;
  }

  input {
    width: 100%;
    padding: 0.5rem;
    background: #16213e;
    color: #e0e0e0;
    border: 1px solid #0f3460;
    border-radius: 4px;
    font-size: 1rem;
    box-sizing: border-box;
  }

  .actions {
    display: flex;
    gap: 0.5rem;
    justify-content: flex-end;
    margin-top: 2rem;
  }

  button {
    padding: 0.5rem 1.5rem;
    background: #16213e;
    color: #e0e0e0;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 1rem;
  }

  button:hover {
    background: #0f3460;
  }
</style>
