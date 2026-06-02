<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import { open, confirm } from '@tauri-apps/plugin-dialog';
  import { stat } from '@tauri-apps/plugin-fs';
  import VideoPlayer from '$lib/components/VideoPlayer.svelte';
  import SettingsDialog from '$lib/components/SettingsDialog.svelte';
  import type { Settings, Clip } from '$lib/types';

  let videoPath = $state('');
  let duration = $state(0);
  let position = $state(0);
  let clips: Clip[] = $state([]);

  // Toast notification state
  let toastMessage = $state('');
  let toastType = $state<'success' | 'error'>('success');
  let toastTimeout: ReturnType<typeof setTimeout> | null = null;

  function showToast(message: string, type: 'success' | 'error' = 'success') {
    toastMessage = message;
    toastType = type;
    if (toastTimeout) clearTimeout(toastTimeout);
    toastTimeout = setTimeout(() => { toastMessage = ''; }, 3000);
  }

  let settingsOpen = $state(false);
  let savedSettings: Settings = $state({
    buffer_before: 5.0,
    buffer_after: 5.0,
    clip_key: 'c',
    theme: 'dark'
  });
  let settings: Settings = $state({
    buffer_before: 5.0,
    buffer_after: 5.0,
    clip_key: 'c',
    theme: 'dark'
  });

  function cloneSettings<T>(obj: T): T {
    try {
      if (typeof structuredClone === 'function') {
        return structuredClone(obj);
      }
    } catch {
      // structuredClone may throw on some WebView2 versions
    }
    return JSON.parse(JSON.stringify(obj));
  }

  function openSettings() {
    try {
      savedSettings = cloneSettings(settings);
    } catch (e) {
      console.error('Failed to clone settings:', e);
    }
    settingsOpen = true;
  }

  async function saveSettings(newSettings: Settings) {
    try {
      await api.saveSettings(newSettings);
      settings = newSettings;
      showToast('Settings saved', 'success');
    } catch (e) {
      showToast('Failed to save settings: ' + e, 'error');
      throw e; // Re-throw so SettingsDialog can handle it
    }
  }

  async function refreshClips() {
    try {
      clips = await api.getClips();
    } catch (e) {
      console.error('Failed to load clips:', e);
    }
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
      try {
        // Check file size for large files
        const fileStat = await stat(selected);
        const fileSizeGB = fileStat.size / (1024 * 1024 * 1024);

        if (fileSizeGB > 10) {
          const proceed = await confirm(
            `This file is ${fileSizeGB.toFixed(1)}GB. Conversion may take a long time and require significant disk space. Continue?`,
            { title: 'Large File Warning', kind: 'warning' }
          );
          if (!proceed) return;
        }

        const response = await api.openVideo(selected);
        videoPath = response.play_path;
        duration = response.duration;
        await refreshClips();
      } catch (e) {
        showToast('Failed to open video: ' + e, 'error');
      }
    }
  }

  function onPositionChange(newPosition: number, newDuration: number) {
    position = newPosition;
    duration = newDuration;
  }

  async function saveClip() {
    if (!videoPath) return;
    try {
      const result = await api.saveClip(position, duration);
      if (result.success) {
        const filename = result.path.split('/').pop() ?? result.path;
        showToast('Clip saved: ' + filename, 'success');
        await refreshClips();
      } else {
        showToast('Clip failed: ' + (result.error ?? 'unknown error'), 'error');
      }
    } catch (e) {
      showToast('Clip error: ' + e, 'error');
    }
  }

  async function deleteClip(clip: Clip) {
    try {
      await api.deleteClip(clip.id, clip.clip_path);
      const filename = clip.clip_path.split('/').pop() ?? clip.clip_path;
      showToast('Deleted: ' + filename, 'success');
      await refreshClips();
    } catch (e) {
      showToast('Delete failed: ' + e, 'error');
    }
  }

  // Clips refresh interval
  let clipsRefreshInterval: ReturnType<typeof setInterval> | null = null;

  onMount(() => {
    // Load saved settings from backend
    api.getSettings()
      .then(loadedSettings => {
        settings = loadedSettings;
      })
      .catch((e) => {
        console.error('Failed to load settings:', e);
      });

    // Register global shortcuts
    const handleKeydown = async (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
        return; // Don't intercept when typing
      }

      // Don't trigger shortcuts when settings dialog is open
      if (settingsOpen) return;

      if (e.key === 'o' || e.key === 'O') {
        await openVideo();
      } else if (e.key.toLowerCase() === settings.clip_key.toLowerCase()) {
        await saveClip();
      }
    };

    window.addEventListener('keydown', handleKeydown);

    // Refresh clips list every 3 seconds to detect manual deletions
    // Skip refresh when app is not visible to save resources
    clipsRefreshInterval = setInterval(async () => {
      if (videoPath && !document.hidden) {
        await refreshClips();
      }
    }, 3000);

    return () => {
      window.removeEventListener('keydown', handleKeydown);
      if (clipsRefreshInterval) clearInterval(clipsRefreshInterval);
      if (toastTimeout) clearTimeout(toastTimeout);
    };
  });
</script>

<div class="main-layout" data-theme={settings.theme}>
  <div class="video-section">
    <div class="video-wrapper">
      <VideoPlayer {videoPath} {onPositionChange} settingsOpen={settingsOpen} />
    </div>

    <div class="controls">
      <button onclick={openVideo}>Open (O)</button>
      <button onclick={saveClip} disabled={!videoPath}>
        Clip ({settings.clip_key.toUpperCase()})
      </button>
      <button onclick={openSettings}>Settings</button>
    </div>

    {#if videoPath}
      <div class="status">
        Position: {position.toFixed(1)}s / {duration.toFixed(1)}s
      </div>
    {/if}
  </div>

  <div class="clips-section">
    <h2>Saved Clips ({clips.length})</h2>
    {#if clips.length === 0}
      <p class="placeholder">{videoPath ? 'No clips yet — press C to save one' : 'Open a video to see clips'}</p>
    {:else}
      <ul class="clip-list">
        {#each clips as clip}
          <li class="clip-item">
            <div class="clip-info">
              <div class="clip-name">{clip.clip_path.split('/').pop()}</div>
              <div class="clip-time">
                {clip.start_time.toFixed(1)}s — {clip.end_time.toFixed(1)}s
              </div>
            </div>
            <button class="delete-btn" onclick={() => deleteClip(clip)} title="Delete clip">×</button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  {#if toastMessage}
    <div class="toast" class:toast-success={toastType === 'success'} class:toast-error={toastType === 'error'}>
      {toastMessage}
    </div>
  {/if}

  <SettingsDialog
    bind:open={settingsOpen}
    bind:settings={settings}
    onsave={saveSettings}
    oncancel={() => {
      try {
        settings = cloneSettings(savedSettings);
      } catch {
        // revert failed, keep current settings
      }
      settingsOpen = false;
    }}
  />
</div>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    overflow: hidden;
  }

  /* Dark theme (default) */
  .main-layout[data-theme="dark"], .main-layout:not([data-theme]) {
    --bg-primary: #1a1a2e;
    --bg-secondary: #16213e;
    --bg-tertiary: #0f3460;
    --text-primary: #e0e0e0;
    --text-secondary: #888;
    --accent: #e94560;
    --border: #0f3460;
    --danger: #9b2226;
    --danger-text: #fec89a;
    color-scheme: dark;
  }

  /* Light theme */
  .main-layout[data-theme="light"] {
    --bg-primary: #f5f5f5;
    --bg-secondary: #e8e8e8;
    --bg-tertiary: #d0d0d0;
    --text-primary: #1a1a1a;
    --text-secondary: #666;
    --accent: #d63447;
    --border: #c0c0c0;
    --danger: #ae2012;
    --danger-text: #fec89a;
    color-scheme: light;
  }

  .main-layout {
    display: grid;
    grid-template-columns: 2fr 1fr;
    grid-template-rows: 1fr;
    height: 100vh;
    gap: 1rem;
    padding: 1rem;
    color: var(--text-primary);
    overflow: hidden;
    box-sizing: border-box;
    transition: background 0.3s ease;
    background: var(--bg-primary);
  }

  .video-section {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    min-height: 0;
    overflow: hidden;
  }

  .video-wrapper {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .controls {
    display: flex;
    gap: 0.5rem;
    flex-shrink: 0;
    padding-left: 0.5rem;
  }

  button {
    padding: 0.5rem 1rem;
    background: var(--bg-secondary);
    color: var(--text-primary);
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 1rem;
  }

  button:hover:not(:disabled) {
    background: var(--bg-tertiary);
  }

  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .status {
    color: var(--text-secondary);
    font-size: 0.9rem;
    flex-shrink: 0;
  }

  .clips-section {
    display: flex;
    flex-direction: column;
    background: var(--bg-secondary);
    padding: 1rem;
    border-radius: 4px;
    overflow: hidden;
    min-height: 0;
  }

  .clips-section h2 {
    flex-shrink: 0;
    margin-bottom: 1rem;
    color: var(--accent);
  }

  .clips-section .placeholder {
    flex-shrink: 0;
  }

  .clip-list {
    list-style: none;
    padding: 0;
    margin: 0;
    overflow-y: auto;
    flex: 1;
    min-height: 0;
  }

  .clip-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.5rem;
    margin-bottom: 0.5rem;
    background: var(--bg-primary);
    border-radius: 4px;
    border-left: 3px solid var(--accent);
  }

  .clip-info {
    flex: 1;
    min-width: 0;
  }

  .delete-btn {
    flex-shrink: 0;
    width: 28px;
    height: 28px;
    padding: 0;
    margin-left: 0.5rem;
    background: transparent;
    color: #888;
    font-size: 1.2rem;
    line-height: 1;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .delete-btn:hover {
    background: var(--danger);
    color: var(--danger-text);
  }

  .clip-name {
    font-size: 0.85rem;
    word-break: break-all;
  }

  .clip-time {
    font-size: 0.8rem;
    color: var(--text-secondary);
    margin-top: 0.25rem;
  }

  .toast {
    position: fixed;
    bottom: 2rem;
    left: 50%;
    transform: translateX(-50%);
    padding: 0.75rem 1.5rem;
    border-radius: 6px;
    font-size: 0.9rem;
    z-index: 1000;
    animation: fadeIn 0.2s ease-out;
  }

  .toast-success {
    background: #2d6a4f;
    color: #d8f3dc;
  }

  .toast-error {
    background: #9b2226;
    color: #fec89a;
  }

  @keyframes fadeIn {
    from { opacity: 0; transform: translateX(-50%) translateY(10px); }
    to { opacity: 1; transform: translateX(-50%) translateY(0); }
  }
</style>
