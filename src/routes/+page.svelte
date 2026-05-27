<script lang="ts">
  let count = $state(0);

  function increment() {
    count += 1;
  }
</script>

<main>
  <h1>Jorja Clipper</h1>
  <p>Cross-platform desktop app for instant sports highlight extraction.</p>
  <button onclick={increment}>Count: {count}</button>
</main>

<style>
  main {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 100vh;
    font-family: system-ui, -apple-system, sans-serif;
    color: #e0e0e0;
    background: #1a1a2e;
  }

  h1 {
    font-size: 2rem;
    margin-bottom: 0.5rem;
  }

  p {
    color: #a0a0b0;
    margin-bottom: 1.5rem;
  }

  button {
    padding: 0.6rem 1.2rem;
    font-size: 1rem;
    border: 1px solid #444;
    border-radius: 6px;
    background: #2a2a4a;
    color: #e0e0e0;
    cursor: pointer;
    transition: background 0.15s;
  }

  button:hover {
    background: #3a3a6a;
  }
</style>
