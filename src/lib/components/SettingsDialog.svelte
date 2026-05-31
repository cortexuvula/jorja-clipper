<script lang="ts">
  import type { Settings } from '$lib/types';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';

  let { open = $bindable(false), settings = $bindable(), onsave, oncancel } = $props();

  let isSaving = $state(false);

  // Client-side validation errors
  let validationErrors = $state<{[key: string]: string}>({});

  function validate(): boolean {
    const errors: {[key: string]: string} = {};

    // Buffer validation (must be 0-60 seconds)
    if (settings.buffer_before < 0 || settings.buffer_before > 60) {
      errors.buffer_before = 'Must be between 0 and 60 seconds';
    }
    if (settings.buffer_after < 0 || settings.buffer_after > 60) {
      errors.buffer_after = 'Must be between 0 and 60 seconds';
    }

    // Clip key validation (must be single character)
    if (!settings.clip_key || settings.clip_key.length !== 1) {
      errors.clip_key = 'Must be exactly one character';
    }

    validationErrors = errors;
    return Object.keys(errors).length === 0;
  }

  async function save() {
    if (isSaving) return;

    if (!validate()) return;

    isSaving = true;
    try {
      await onsave?.(settings);
      open = false;
    } catch (error) {
      // Don't close dialog on error, let user see the error
      console.error('Failed to save settings:', error);
    } finally {
      isSaving = false;
    }
  }

  function cancel() {
    if (isSaving) return;
    oncancel?.();
    open = false;
  }

  async function selectDirectory() {
    const selected = await openDialog({
      directory: true,
      multiple: false,
    });
    if (selected) {
      settings.output_dir = selected;
    }
  }

  function resetToDefaults() {
    settings.buffer_before = 5.0;
    settings.buffer_after = 5.0;
    settings.clip_key = 'c';
    settings.output_dir = undefined;
    settings.theme = 'dark';
    validationErrors = {};
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
          min="0"
          max="60"
          bind:value={settings.buffer_before}
          class:error={validationErrors.buffer_before}
        />
        {#if validationErrors.buffer_before}
          <span class="error-text">{validationErrors.buffer_before}</span>
        {/if}
      </div>

      <div class="field">
        <label for="buffer-after">Buffer After (seconds)</label>
        <input
          id="buffer-after"
          type="number"
          step="0.5"
          min="0"
          max="60"
          bind:value={settings.buffer_after}
          class:error={validationErrors.buffer_after}
        />
        {#if validationErrors.buffer_after}
          <span class="error-text">{validationErrors.buffer_after}</span>
        {/if}
      </div>

      <div class="field">
        <label for="clip-key">Clip Hotkey</label>
        <input
          id="clip-key"
          type="text"
          maxlength="1"
          bind:value={settings.clip_key}
          class:error={validationErrors.clip_key}
        />
        {#if validationErrors.clip_key}
          <span class="error-text">{validationErrors.clip_key}</span>
        {/if}
      </div>

      <div class="field">
        <label for="output-dir">Output Directory (optional)</label>
        <div class="input-group">
          <input
            id="output-dir"
            type="text"
            bind:value={settings.output_dir}
            placeholder="Leave empty to use video's directory"
          />
          <button onclick={selectDirectory} class="btn-secondary">Browse</button>
        </div>
      </div>

      <div class="field">
        <label for="theme">Theme</label>
        <select id="theme" bind:value={settings.theme}>
          <option value="dark">Dark</option>
          <option value="light">Light</option>
        </select>
      </div>

      <div class="actions">
        <button onclick={resetToDefaults} class="btn-secondary">Reset to Defaults</button>
        <button onclick={cancel} disabled={isSaving}>Cancel</button>
        <button onclick={save} disabled={isSaving}>
          {isSaving ? 'Saving...' : 'Save'}
        </button>
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
    background: var(--bg-primary);
    padding: 2rem;
    border-radius: 8px;
    min-width: 400px;
  }

  h2 {
    margin-bottom: 1.5rem;
    color: var(--accent);
  }

  .field {
    margin-bottom: 1rem;
  }

  label {
    display: block;
    margin-bottom: 0.5rem;
    color: var(--text-primary);
  }

  input, select {
    width: 100%;
    padding: 0.5rem;
    background: var(--bg-secondary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: 4px;
    font-size: 1rem;
    box-sizing: border-box;
  }

  input.error {
    border-color: var(--accent);
  }

  .error-text {
    display: block;
    color: var(--accent);
    font-size: 0.85rem;
    margin-top: 0.25rem;
  }

  select {
    cursor: pointer;
  }

  select option {
    background: var(--bg-secondary);
    color: var(--text-primary);
  }

  .actions {
    display: flex;
    gap: 0.5rem;
    justify-content: flex-end;
    margin-top: 2rem;
  }

  button {
    padding: 0.5rem 1.5rem;
    background: var(--bg-secondary);
    color: var(--text-primary);
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 1rem;
  }

  button:hover {
    background: var(--bg-tertiary);
  }

  .input-group {
    display: flex;
    gap: 0.5rem;
  }

  .input-group input {
    flex: 1;
  }

  .btn-secondary {
    padding: 0.5rem 1rem;
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 1rem;
    white-space: nowrap;
  }

  .btn-secondary:hover {
    background: var(--bg-secondary);
  }
</style>
