<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export let disabled = true;

  const dispatch = createEventDispatcher();

  function handleOpen() {
    dispatch('open');
  }

  function handlePlayPause() {
    dispatch('playpause');
  }

  function handleClip() {
    dispatch('clip');
  }

  function handleSettings() {
    dispatch('settings');
  }
</script>

<div class="controls">
  <button on:click={handleOpen} class="btn btn-primary">
    Open Video
  </button>

  <button on:click={handlePlayPause} {disabled} class="btn">
    Play/Pause
  </button>

  <button on:click={handleClip} {disabled} class="btn btn-accent">
    Clip
  </button>

  <button on:click={handleSettings} class="btn">
    Settings
  </button>
</div>

<style>
  .controls {
    display: flex;
    gap: 0.5rem;
    padding: 1rem;
    background: var(--surface-color, #1a1a1a);
    border-radius: 8px;
  }

  .btn {
    padding: 0.5rem 1rem;
    border: none;
    border-radius: 4px;
    background: #2a2a2a;
    color: #fff;
    cursor: pointer;
    font-size: 0.875rem;
    transition: background 0.2s;
  }

  .btn:hover:not(:disabled) {
    background: #3a3a3a;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-primary {
    background: #4a90e2;
  }

  .btn-primary:hover:not(:disabled) {
    background: #357abd;
  }

  .btn-accent {
    background: #e94560;
  }

  .btn-accent:hover:not(:disabled) {
    background: #d63651;
  }
</style>
