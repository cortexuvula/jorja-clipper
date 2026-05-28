<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { convertFileSrc } from '@tauri-apps/api/core';

  let { videoPath = '', onPositionChange } = $props<{
    videoPath?: string;
    onPositionChange?: (position: number, duration: number) => void;
  }>();

  let videoElement = $state() as HTMLVideoElement;
  let duration = $state(0);
  let currentTime = $state(0);
  let paused = $state(true);

  // Conversion progress state
  let isConverting = $state(false);
  let conversionProgress = $state(0);
  let conversionDuration = $state(0);

  // Update parent when position changes
  $effect(() => {
    if (videoPath && onPositionChange) {
      onPositionChange(currentTime, duration);
    }
  });

  // Listen for conversion events
  onMount(() => {
    const unlistenStarted = listen<number>('conversion-started', (event) => {
      isConverting = true;
      conversionProgress = 0;
      conversionDuration = event.payload;
    });

    const unlistenProgress = listen<number>('conversion-progress', (event) => {
      conversionProgress = event.payload;
    });

    const unlistenCompleted = listen<string>('conversion-completed', () => {
      isConverting = false;
      conversionProgress = 100;
    });

    const unlistenFailed = listen<string>('conversion-failed', (event) => {
      isConverting = false;
      conversionProgress = 0;
      console.error('Conversion failed:', event.payload);
    });

    // Keyboard shortcuts
    const handleKeydown = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
        return; // Don't intercept when typing
      }

      if (e.code === 'Space' && videoPath) {
        e.preventDefault();
        togglePlayPause();
      } else if (e.code === 'ArrowLeft' && videoPath) {
        e.preventDefault();
        seek(-5);
      } else if (e.code === 'ArrowRight' && videoPath) {
        e.preventDefault();
        seek(5);
      }
    };

    window.addEventListener('keydown', handleKeydown);

    return () => {
      unlistenStarted.then(f => f());
      unlistenProgress.then(f => f());
      unlistenCompleted.then(f => f());
      unlistenFailed.then(f => f());
      window.removeEventListener('keydown', handleKeydown);
    };
  });

  function onLoadedMetadata() {
    duration = videoElement.duration;
  }

  function onTimeUpdate() {
    currentTime = videoElement.currentTime;
  }

  function togglePlayPause() {
    if (videoElement.paused) {
      videoElement.play();
      paused = false;
    } else {
      videoElement.pause();
      paused = true;
    }
  }

  function seek(seconds: number) {
    const newTime = Math.max(0, Math.min(duration, currentTime + seconds));
    videoElement.currentTime = newTime;
  }

  // Convert file path to URL for video element
  function getVideoUrl(path: string): string {
    // Use Tauri's asset protocol to access local files
    return convertFileSrc(path);
  }
</script>

<div class="video-container">
  {#if isConverting}
    <div class="conversion-overlay">
      <div class="conversion-info">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10"></circle>
          <polyline points="12 6 12 12 16 14"></polyline>
        </svg>
        <p class="conversion-title">Converting video for playback...</p>
        <p class="conversion-subtitle">This may take a few minutes</p>
        <div class="progress-bar">
          <div class="progress-fill" style="width: {conversionProgress}%"></div>
        </div>
        <p class="progress-text">{conversionProgress.toFixed(0)}%</p>
      </div>
    </div>
  {:else if videoPath}
    <video
      bind:this={videoElement}
      src={getVideoUrl(videoPath)}
      onloadedmetadata={onLoadedMetadata}
      ontimeupdate={onTimeUpdate}
      onplay={() => paused = false}
      onpause={() => paused = true}
      class="video-element"
    >
      <track kind="captions" />
    </video>
  {:else}
    <div class="placeholder">
      <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <rect x="2" y="2" width="20" height="20" rx="2.18" ry="2.18"></rect>
        <line x1="7" y1="2" x2="7" y2="22"></line>
        <line x1="17" y1="2" x2="17" y2="22"></line>
        <line x1="2" y1="12" x2="22" y2="12"></line>
        <line x1="2" y1="7" x2="7" y2="7"></line>
        <line x1="2" y1="17" x2="7" y2="17"></line>
        <line x1="17" y1="17" x2="22" y2="17"></line>
        <line x1="17" y1="7" x2="22" y2="7"></line>
      </svg>
      <p>No video loaded</p>
      <p class="hint">Press O to open a video file</p>
    </div>
  {/if}
</div>

<style>
  .video-container {
    width: 100%;
    height: 100%;
    max-height: 100%;
    background: #000;
    border-radius: 8px;
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
    flex: 1;
    min-height: 0;
    position: relative;
  }

  .video-element {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .conversion-overlay {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.9);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 10;
  }

  .conversion-info {
    text-align: center;
    color: #e0e0e0;
    max-width: 400px;
    padding: 2rem;
  }

  .conversion-info svg {
    margin-bottom: 1rem;
    opacity: 0.7;
  }

  .conversion-title {
    font-size: 1.25rem;
    font-weight: 600;
    margin-bottom: 0.5rem;
  }

  .conversion-subtitle {
    font-size: 0.875rem;
    color: #888;
    margin-bottom: 2rem;
  }

  .progress-bar {
    width: 100%;
    height: 8px;
    background: #333;
    border-radius: 4px;
    overflow: hidden;
    margin-bottom: 0.5rem;
  }

  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #e94560 0%, #f06292 100%);
    transition: width 0.3s ease;
  }

  .progress-text {
    font-size: 0.875rem;
    color: #888;
  }

  .placeholder {
    text-align: center;
    color: #666;
  }

  .placeholder svg {
    margin-bottom: 1rem;
    opacity: 0.5;
  }

  .placeholder p {
    margin: 0.5rem 0;
  }

  .hint {
    font-size: 0.875rem;
    opacity: 0.7;
  }
</style>
