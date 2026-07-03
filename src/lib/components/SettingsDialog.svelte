<script lang="ts">
  import type { Settings } from '$lib/types';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';

  let { open = $bindable(false), settings = $bindable(), onsave, oncancel } = $props();

  let isSaving = $state(false);

  // True when the user tried to cancel/close with unsaved changes and is being
  // asked to confirm the discard. Replaces a blocking OS confirm() with an
  // in-app styled prompt consistent with the rest of the UI.
  let pendingDiscard = $state(false);

  // Client-side validation errors
  let validationErrors = $state<{[key: string]: string}>({});

  // Snapshot of settings taken when the dialog opens, used to detect unsaved
  // changes and confirm before discarding them on cancel/backdrop/Escape.
  let snapshot = $state<string>('');

  function settingsSignature(s: Settings): string {
    return JSON.stringify({
      buffer_before: s.buffer_before,
      buffer_after: s.buffer_after,
      clip_key: s.clip_key,
      output_dir: s.output_dir,
      theme: s.theme,
    });
  }

  function hasUnsavedChanges(): boolean {
    return settingsSignature(settings) !== snapshot;
  }

  // Refresh the snapshot whenever the dialog is (re)opened so dirty detection
  // starts from the current values rather than a stale prior session.
  $effect(() => {
    if (open) {
      snapshot = settingsSignature(settings);
      pendingDiscard = false;
    }
  });

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
      snapshot = settingsSignature(settings);
      open = false;
    } catch (error) {
      // Don't close dialog on error, let user see the error
      console.error('Failed to save settings:', error);
    } finally {
      isSaving = false;
    }
  }

  // Attempt to close the dialog. If there are unsaved changes, surface an
  // in-app confirmation instead of discarding immediately.
  function requestCancel() {
    if (isSaving) return;
    if (hasUnsavedChanges()) {
      pendingDiscard = true;
    } else {
      doCancel();
    }
  }

  // Actually discard changes and close.
  function doCancel() {
    pendingDiscard = false;
    oncancel?.();
    open = false;
  }

  // Keep editing — dismiss the discard confirmation.
  function keepEditing() {
    pendingDiscard = false;
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
  <div class="modal-backdrop" onclick={requestCancel} onkeydown={(e) => { if (e.key === 'Escape') requestCancel(); }} role="presentation">
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
        <button onclick={requestCancel} disabled={isSaving}>Cancel</button>
        <button onclick={save} disabled={isSaving}>
          {isSaving ? 'Saving...' : 'Save'}
        </button>
      </div>
    </div>

    {#if pendingDiscard}
      <div
        class="confirm-overlay"
        onclick={(e) => e.stopPropagation()}
        onkeydown={(e) => e.stopPropagation()}
        role="alertdialog"
        aria-modal="true"
        aria-label="Discard unsaved changes?"
        tabindex="-1"
      >
        <p class="confirm-title">Discard unsaved changes?</p>
        <p class="confirm-body">Your edits to these settings will be lost.</p>
        <div class="confirm-actions">
          <button onclick={keepEditing} class="btn-secondary">Keep editing</button>
          <button onclick={doCancel} class="btn-danger">Discard</button>
        </div>
      </div>
    {/if}
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
    z-index: 9999;
  }

  .modal {
    background: var(--bg-primary);
    padding: 2rem;
    border-radius: 8px;
    min-width: 400px;
    position: relative;
    z-index: 10000;
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

  .btn-danger {
    background: var(--danger);
    color: var(--danger-text);
  }

  .btn-danger:hover {
    filter: brightness(1.1);
  }

  .confirm-overlay {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.75);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    border-radius: 8px;
    z-index: 10001;
    padding: 2rem;
    box-sizing: border-box;
  }

  .confirm-title {
    font-size: 1.15rem;
    font-weight: 600;
    margin: 0 0 0.5rem;
    color: var(--text-primary);
  }

  .confirm-body {
    font-size: 0.9rem;
    color: var(--text-secondary);
    margin: 0 0 1.5rem;
  }

  .confirm-actions {
    display: flex;
    gap: 0.5rem;
  }
</style>
