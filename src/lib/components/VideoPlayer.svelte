<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';

  let { videoLoaded = false, onMpvWindowCreated } = $props<{
    videoLoaded?: boolean;
    onMpvWindowCreated?: (wid: number) => void;
  }>();

  let container: HTMLDivElement;
  let rect: DOMRect | null = $state(null);

  async function sendPosition() {
    if (!rect || !videoLoaded) return;
    const x = Math.round(rect.x);
    const y = Math.round(rect.y);
    const w = Math.round(rect.width);
    const h = Math.round(rect.height);
    await api.positionMpvWindow(x, y, w, h);
  }

  // Reactively update mpv window when rect or videoLoaded changes
  $effect(() => {
    if (rect && videoLoaded) {
      sendPosition();
    }
  });

  onMount(() => {
    // Create the mpv overlay window and get its native handle
    api.createMpvWindow().then((wid) => {
      onMpvWindowCreated?.(wid);
    }).catch((e) => {
      console.error('Failed to create mpv window:', e);
    });

    // Track container position/size for mpv overlay
    const observer = new ResizeObserver(() => {
      rect = container.getBoundingClientRect();
    });
    observer.observe(container);

    // Get initial rect
    rect = container.getBoundingClientRect();

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
