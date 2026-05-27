<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  let { videoLoaded = false } = $props();

  let container: HTMLDivElement;
  let rect: DOMRect | null = $state(null);

  async function updateMpvWindow() {
    if (!rect || !videoLoaded) return;

    const appWindow = getCurrentWindow();
    const scaleFactor = await appWindow.scaleFactor();

    // Convert from logical to physical pixels
    const physicalRect = {
      x: Math.round(rect.x * scaleFactor),
      y: Math.round(rect.y * scaleFactor),
      width: Math.round(rect.width * scaleFactor),
      height: Math.round(rect.height * scaleFactor),
    };

    await invoke('position_mpv_window', physicalRect);
  }

  // Reactively update mpv window when rect or videoLoaded changes
  $effect(() => {
    if (rect && videoLoaded) {
      updateMpvWindow();
    }
  });

  onMount(() => {
    const observer = new ResizeObserver(() => {
      rect = container.getBoundingClientRect();
    });

    observer.observe(container);

    return () => observer.disconnect();
  });
</script>

<div bind:this={container} class="video-container" class:loaded={videoLoaded}>
  {#if !videoLoaded}
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
    aspect-ratio: 16/9;
    background: #000;
    border-radius: 8px;
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .video-container.loaded {
    background: transparent;
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
