<script lang="ts">
  export let videoLoaded = false;
</script>

<div class="video-player" class:loaded={videoLoaded}>
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
      <p class="hint">Open a video file to begin</p>
    </div>
  {:else}
    <div class="mpv-container" id="mpv-container">
      <!-- mpv will render here -->
    </div>
  {/if}
</div>

<style>
  .video-player {
    width: 100%;
    aspect-ratio: 16 / 9;
    background: #000;
    border-radius: 8px;
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .video-player.loaded {
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

  .placeholder .hint {
    font-size: 0.875rem;
    opacity: 0.7;
  }

  .mpv-container {
    width: 100%;
    height: 100%;
  }
</style>
