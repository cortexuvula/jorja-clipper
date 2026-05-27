<script lang="ts">
  import VideoPlayer from '$lib/components/VideoPlayer.svelte';
  import Controls from '$lib/components/Controls.svelte';
  import { playerStore } from '$lib/stores/player';

  let videoLoaded = false;

  function handleOpen() {
    console.log('Open video requested');
    // TODO: Implement file dialog
  }

  function handlePlayPause() {
    console.log('Play/Pause requested');
    // TODO: Implement play/pause
  }

  function handleClip() {
    console.log('Clip requested');
    // TODO: Implement clip
  }

  function handleSettings() {
    console.log('Settings requested');
    // TODO: Implement settings dialog
  }
</script>

<main>
  <div class="app-container">
    <VideoPlayer {videoLoaded} />
    <Controls
      disabled={!videoLoaded}
      on:open={handleOpen}
      on:playpause={handlePlayPause}
      on:clip={handleClip}
      on:settings={handleSettings}
    />
  </div>
</main>

<style>
  main {
    min-height: 100vh;
    background: #121212;
    color: #fff;
    padding: 1rem;
  }

  .app-container {
    max-width: 1200px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
</style>
