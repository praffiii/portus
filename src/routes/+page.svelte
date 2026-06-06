<script lang="ts">
  import { Settings } from "@lucide/svelte";

  import DockerList from "$lib/components/DockerList.svelte";
  import PortList from "$lib/components/PortList.svelte";
  import { dockerFixtures, portFixtures } from "$lib/fixtures";

  const runningCount =
    portFixtures.filter((item) => item.status === "running").length +
    dockerFixtures.filter((item) => item.status === "running").length;
  const waitingCount =
    portFixtures.filter((item) => item.status === "waiting").length +
    dockerFixtures.filter((item) => item.status === "waiting").length;
</script>

<main class="popover">
  <header class="glance">
    <div class="identity">
      <p class="product">Portus</p>
      <p class="summary">
        <span class="running">{runningCount} running</span>
        <span aria-hidden="true">·</span>
        <span>{waitingCount} waiting</span>
      </p>
    </div>
    <button class="icon-button" type="button" aria-label="Settings (unavailable)" title="Settings unavailable" disabled>
      <Settings size={16} strokeWidth={1.8} aria-hidden="true" />
    </button>
  </header>

  <div class="scroll-body">
    <PortList ports={portFixtures} />
    <DockerList containers={dockerFixtures} />
  </div>
</main>

<style>
  @font-face {
    font-family: "Geist";
    src: url("/fonts/Geist-Variable.woff2") format("woff2");
    font-style: normal;
    font-weight: 100 900;
    font-display: swap;
  }

  @font-face {
    font-family: "Geist Mono";
    src: url("/fonts/GeistMono-Variable.woff2") format("woff2");
    font-style: normal;
    font-weight: 100 900;
    font-display: swap;
  }

  :global(html),
  :global(body) {
    margin: 0;
    width: 100%;
    min-width: 380px;
    height: 100%;
    min-height: 520px;
    overflow: hidden;
    color: var(--text-primary);
    background: transparent;
    font-family: var(--font-ui);
    font-synthesis: none;
    text-rendering: optimizeLegibility;
    -webkit-font-smoothing: antialiased;
  }

  :global(:root) {
    --font-ui: "Geist", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    --font-mono: "Geist Mono", "SFMono-Regular", Consolas, monospace;
    --app-bg: #fbfbfd;
    --surface: #ffffff;
    --hairline: #e5e7eb;
    --text-primary: #18181b;
    --text-muted: #71717a;
    --accent: #0d9488;
  }

  :global(*) {
    box-sizing: border-box;
  }

  :global(button) {
    font: inherit;
  }

  .popover {
    width: 380px;
    height: 520px;
    overflow: hidden;
    border: 1px solid var(--hairline);
    border-radius: 12px;
    color: var(--text-primary);
    background: var(--app-bg);
  }

  .glance {
    position: relative;
    z-index: 3;
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 56px;
    padding: 0 12px;
    border-bottom: 1px solid var(--hairline);
    background: var(--surface);
  }

  .product,
  .summary {
    margin: 0;
  }

  .identity {
    min-width: 0;
  }

  .product {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .summary {
    display: flex;
    gap: 5px;
    margin-top: 2px;
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
  }

  .running {
    color: #15803d;
  }

  .icon-button {
    display: grid;
    width: 28px;
    height: 28px;
    flex: 0 0 28px;
    place-items: center;
    padding: 0;
    border: 1px solid transparent;
    border-radius: 6px;
    color: var(--text-muted);
    background: transparent;
    cursor: default;
    opacity: 0.55;
  }

  .scroll-body {
    height: calc(520px - 56px);
    max-height: calc(520px - 56px);
    overflow-x: hidden;
    overflow-y: auto;
    overscroll-behavior: contain;
  }

  @media (prefers-color-scheme: dark) {
    :global(:root) {
      --app-bg: #1c1c1e;
      --surface: #242427;
      --hairline: #38383c;
      --text-primary: #f4f4f5;
      --text-muted: #a1a1aa;
      --accent: #2dd4bf;
    }

    .running {
      color: #4ade80;
    }
  }

</style>
