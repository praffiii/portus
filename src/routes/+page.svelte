<script lang="ts">
  import { isTauri } from "@tauri-apps/api/core";
  import { Anchor, Plus, Settings } from "@lucide/svelte";
  import { onMount } from "svelte";

  import { commands, events, type Snapshot } from "$lib/bindings";
  import DockerList from "$lib/components/DockerList.svelte";
  import PortList from "$lib/components/PortList.svelte";
  import { snapshotFixture } from "$lib/fixtures";
  import { containersToDockerRows, snapshotToPortRows } from "$lib/snapshot-adapter";

  let snapshot: Snapshot = $state(snapshotFixture);
  const ports = $derived(snapshotToPortRows(snapshot));
  const containers = $derived(containersToDockerRows(snapshot.docker.data.containers));
  const runningCount = $derived(
    ports.filter((item) => item.status === "running").length +
      containers.filter((item) => item.status === "running").length
  );
  const waitingCount = $derived(
    ports.filter((item) => item.status === "waiting").length +
      containers.filter((item) => item.status === "waiting").length
  );

  onMount(() => {
    if (!isTauri()) return;

    let disposed = false;
    let stopListening: (() => void) | undefined;

    void events.snapshot.listen((event) => {
      snapshot = event.payload;
    }).then((unlisten) => {
      if (disposed) {
        unlisten();
      } else {
        stopListening = unlisten;
      }
    });
    void commands.setActive();

    return () => {
      disposed = true;
      stopListening?.();
      void commands.setIdle();
    };
  });
</script>

<main class="popover">
  <header class="glance">
    <div class="brand">
      <Anchor class="brand-mark" size={18} strokeWidth={1.75} aria-hidden="true" />
      <div class="identity">
        <p class="product">Portus</p>
        <p class="summary">
          <span class="running">{runningCount} running</span>
          <span class="sep" aria-hidden="true">·</span>
          <span class="neutral">{waitingCount} waiting?</span>
        </p>
      </div>
    </div>
    <button class="icon-button" type="button" aria-label="Settings (unavailable)" title="Settings unavailable" disabled>
      <Settings size={15} strokeWidth={1.8} aria-hidden="true" />
    </button>
  </header>

  <div class="scroll-body">
    <PortList {ports} />
    <DockerList {containers} />
  </div>

  <!-- Quick-run bar: visual chrome; command execution lands with Layer 2-3 (T8). -->
  <div class="quick-run">
    <span class="qr-prompt" aria-hidden="true">›</span>
    <input
      class="qr-input"
      type="text"
      placeholder="Quick run a command…"
      spellcheck="false"
      title="Quick run (unavailable)"
      aria-label="Quick run a command (unavailable)"
      disabled
    />
  </div>

  <footer class="footer">
    <div class="footer-left">
      <button class="footer-btn" type="button" title="Add project (unavailable)" disabled>
        <Plus size={12} strokeWidth={2} aria-hidden="true" />
        <span>Add project</span>
      </button>
      <button class="footer-btn" type="button" title="Settings (unavailable)" disabled>
        <Settings size={12} strokeWidth={1.8} aria-hidden="true" />
        <span>Settings</span>
      </button>
    </div>
    <kbd class="kbd">⌥⌘P</kbd>
  </footer>
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
    --app-bg: rgb(251 251 253 / 72%);
    --surface: rgb(255 255 255 / 70%);
    --surface-hi: rgb(255 255 255 / 92%);
    --hairline: #e7e7ea;
    --text-primary: #1a1a1e;
    --text-secondary: #5c5c63;
    --text-muted: #8a8a92;
    --accent: #0f9d63;
    --running: #12a266;
    --waiting: #b07a2e;
    --stopped: #a3a3ab;
    --crashed: #cf5a4c;
    --popover-shadow: 0 0 0 0.5px rgb(0 0 0 / 4%) inset, 0 16px 44px rgb(0 0 0 / 14%);
  }

  :global(*) {
    box-sizing: border-box;
  }

  :global(button) {
    font: inherit;
  }

  .popover {
    display: flex;
    width: 380px;
    height: 520px;
    flex-direction: column;
    overflow: hidden;
    border: 1px solid var(--hairline);
    border-radius: 12px;
    color: var(--text-primary);
    background: var(--app-bg);
    box-shadow: var(--popover-shadow);
  }

  .glance {
    position: relative;
    z-index: 3;
    display: flex;
    height: 56px;
    flex: 0 0 56px;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px;
    border-bottom: 1px solid var(--hairline);
    background: var(--surface);
  }

  .brand {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 9px;
  }

  :global(.brand-mark) {
    flex-shrink: 0;
    color: var(--text-primary);
    opacity: 0.5;
  }

  .product,
  .summary {
    margin: 0;
  }

  .identity {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: 2px;
  }

  .product {
    font-size: 11px;
    font-weight: 600;
    line-height: 1;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .summary {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 13px;
    font-weight: 600;
    line-height: 1;
    white-space: nowrap;
  }

  .running {
    color: var(--running);
  }

  .summary .sep {
    color: var(--text-muted);
  }

  .summary .neutral {
    color: var(--text-primary);
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
    opacity: 0.5;
    transition:
      opacity 100ms ease,
      background 100ms ease;
  }

  .scroll-body {
    flex: 1 1 0;
    overflow-x: hidden;
    overflow-y: auto;
    overscroll-behavior: contain;
  }

  .scroll-body::-webkit-scrollbar {
    width: 5px;
  }

  .scroll-body::-webkit-scrollbar-track {
    background: transparent;
  }

  .scroll-body::-webkit-scrollbar-thumb {
    border-radius: 3px;
    background: rgb(120 120 130 / 35%);
  }

  .icon-button:hover:not(:disabled) {
    opacity: 1;
    background: rgb(127 127 127 / 12%);
  }

  /* Quick-run bar — visual chrome until command execution lands (T8). */
  .quick-run {
    display: flex;
    height: 36px;
    flex: 0 0 36px;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    border-top: 1px solid var(--hairline);
    background: var(--app-bg);
  }

  .qr-prompt {
    margin-top: -1px;
    color: var(--accent);
    font-family: var(--font-mono);
    font-size: 14px;
    line-height: 1;
    user-select: none;
  }

  .qr-input {
    min-width: 0;
    flex: 1;
    border: none;
    background: transparent;
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: 12px;
    outline: none;
  }

  .qr-input::placeholder {
    color: var(--text-muted);
  }

  .footer {
    display: flex;
    height: 36px;
    flex: 0 0 36px;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px;
    border-top: 1px solid var(--hairline);
    background: var(--surface);
  }

  .footer-left {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .footer-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 0;
    border: none;
    background: transparent;
    color: var(--text-muted);
    font-family: var(--font-ui);
    font-size: 12px;
    cursor: default;
  }

  .footer-btn:disabled {
    opacity: 0.65;
  }

  .kbd {
    padding: 2px 6px;
    border: 1px solid var(--hairline);
    border-radius: 4px;
    background: rgb(127 127 127 / 10%);
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: 10px;
    user-select: none;
  }

  @media (prefers-color-scheme: dark) {
    :global(:root) {
      --app-bg: rgb(22 22 24 / 72%);
      --surface: rgb(28 28 31 / 70%);
      --surface-hi: #212125;
      --hairline: #2a2a2e;
      --text-primary: #ededee;
      --text-secondary: #8c8c93;
      --text-muted: #5c5c63;
      --accent: #45ce93;
      --running: #45ce93;
      --waiting: #d2a24c;
      --stopped: #56565c;
      --crashed: #d97066;
      --popover-shadow: 0 0 0 0.5px rgb(255 255 255 / 3.5%) inset,
        0 16px 44px rgb(0 0 0 / 55%);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .icon-button {
      transition: none;
    }
  }
</style>
