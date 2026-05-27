<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import { open } from '@tauri-apps/plugin-dialog';
  import VideoPlayer from '$lib/components/VideoPlayer.svelte';
  import SettingsDialog from '$lib/components/SettingsDialog.svelte';
  import type { Settings } from '$lib/types';

  let videoLoaded = $state(false);
  let videoPath = $state('');
  let duration = $state(0);
  let position = $state(0);
  let paused = $state(true);

  let settingsOpen = $state(false);
  let settings: Settings = $state({
    buffer_before: 5.0,
    buffer_after: 5.0,
    clip_key: 'c',
    theme: 'dark'
  });

  function openSettings() {
    settingsOpen = true;
  }

  function saveSettings(newSettings: Settings) {
    settings = newSettings;
    // TODO: Persist to backend
  }

  async function openVideo() {
    const selected = await open({
      multiple: false,
      filters: [{
        name: 'Video',
        extensions: ['mp4', 'mkv', 'avi', 'mov', 'webm', 'ts']
      }]
    });

    if (selected) {
      videoPath = selected;
      duration = await api.openVideo(selected);
      videoLoaded = true;
      paused = true;
    }
  }

  async function togglePause() {
    await api.togglePause();
    paused = !paused;
  }

  async function seek(seconds: number) {
    await api.seek(seconds);
    position = await api.getPosition();
  }

  async function saveClip() {
    try {
      const result = await api.saveClip();
      if (result.success) {
        console.log('Clip saved:', result.path);
      } else {
        console.error('Clip failed:', result.error);
      }
    } catch (e) {
      console.error('Clip error:', e);
    }
  }

  // Position polling loop
  let pollInterval: ReturnType<typeof setInterval> | null = null;

  onMount(() => {
    // Register global shortcuts
    const handleKeydown = async (e: KeyboardEvent) => {
      if (e.key === 'o' || e.key === 'O') {
        await openVideo();
      } else if (e.key === ' ') {
        e.preventDefault();
        await togglePause();
      } else if (e.key === 'c' || e.key === 'C') {
        await saveClip();
      } else if (e.key === 'ArrowLeft') {
        await seek(e.shiftKey ? -1 : -5);
      } else if (e.key === 'ArrowRight') {
        await seek(e.shiftKey ? 1 : 5);
      }
    };

    window.addEventListener('keydown', handleKeydown);

    // Poll for position updates every 200ms
    pollInterval = setInterval(async () => {
      if (videoLoaded && !paused) {
        try {
          position = await api.getPosition();
        } catch (e) {
          // ignore polling errors
        }
      }
    }, 200);

    return () => {
      window.removeEventListener('keydown', handleKeydown);
      if (pollInterval) clearInterval(pollInterval);
      api.shutdown();
    };
  });
</script>

<div class="main-layout">
  <div class="video-section">
    <VideoPlayer {videoLoaded} />

    <div class="controls">
      <button onclick={openVideo}>Open (O)</button>
      <button onclick={togglePause} disabled={!videoLoaded}>
        {paused ? 'Play' : 'Pause'} (Space)
      </button>
      <button onclick={saveClip} disabled={!videoLoaded}>
        Clip (C)
      </button>
      <button onclick={openSettings}>Settings</button>
    </div>

    {#if videoLoaded}
      <div class="status">
        Position: {position.toFixed(1)}s / {duration.toFixed(1)}s
      </div>
    {/if}
  </div>

  <div class="clips-section">
    <h2>Saved Clips</h2>
    <p class="placeholder">No clips yet</p>
  </div>
</div>

<SettingsDialog
  bind:open={settingsOpen}
  bind:settings={settings}
  onsave={saveSettings}
  oncancel={() => settingsOpen = false}
/>

<style>
  .main-layout {
    display: grid;
    grid-template-columns: 2fr 1fr;
    height: 100vh;
    gap: 1rem;
    padding: 1rem;
    background: #1a1a2e;
    color: #e0e0e0;
  }

  .video-section {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .controls {
    display: flex;
    gap: 0.5rem;
  }

  button {
    padding: 0.5rem 1rem;
    background: #16213e;
    color: #e0e0e0;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 1rem;
  }

  button:hover:not(:disabled) {
    background: #0f3460;
  }

  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .status {
    color: #888;
    font-size: 0.9rem;
  }

  .clips-section {
    background: #16213e;
    padding: 1rem;
    border-radius: 4px;
    overflow-y: auto;
  }

  h2 {
    margin-bottom: 1rem;
    color: #e94560;
  }

  .placeholder {
    color: #888;
    font-style: italic;
  }
</style>
