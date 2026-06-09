<script lang="ts">
  import { Channel, isTauri } from "@tauri-apps/api/core";
  import { onDestroy, onMount, tick } from "svelte";

  import { commands, type LogBatch, type LogLine } from "$lib/bindings";

  const MAX_LINES = 200;

  let {
    runId,
    projectId,
    taskId,
    containerId,
    readonly = true,
    terminal = false
  }: {
    runId?: string;
    projectId?: string;
    taskId?: string;
    containerId?: string;
    readonly?: boolean;
    terminal?: boolean;
  } = $props();

  let lines: LogLine[] = $state([]);
  let viewport: HTMLDivElement | undefined = $state();
  let input = $state("");

  function append(batch: LogBatch) {
    lines = [...lines, ...batch.lines].slice(-MAX_LINES);
    void tick().then(() => {
      if (viewport) viewport.scrollTop = viewport.scrollHeight;
    });
  }

  onMount(() => {
    if (!isTauri()) return;

    const channel = new Channel<LogBatch>();
    channel.onmessage = append;
    if (containerId) {
      void commands.subscribeDockerLogs(containerId, channel);
    } else if (runId) {
      void commands.subscribeLogs(runId, channel);
    }

    return () => {
      if (containerId) {
        void commands.unsubscribeDockerLogs(containerId);
      } else if (runId) {
        void commands.unsubscribeLogs(runId);
      }
    };
  });

  onDestroy(() => {
    if (!isTauri()) return;
    if (containerId) {
      void commands.unsubscribeDockerLogs(containerId);
    } else if (runId) {
      void commands.unsubscribeLogs(runId);
    }
  });

  async function sendInput() {
    if (!isTauri() || !runId || input.length === 0 || terminal) return;
    const data = `${input}\n`;
    input = "";
    await commands.sendInput(runId, data);
  }
</script>

<div class="log-peek">
  <div bind:this={viewport} class="log-lines" aria-label="Task output preview">
    {#each lines as line}
      <div class="log-line">{@html line.html}</div>
    {/each}
  </div>
  {#if !readonly}
    <form
      class="log-input"
      onsubmit={(event) => {
        event.preventDefault();
        void sendInput();
      }}
    >
      <input bind:value={input} disabled={terminal} spellcheck="false" aria-label="Send input" />
      <button type="submit" disabled={terminal || input.length === 0}>Send</button>
    </form>
  {/if}
</div>

<style>
  .log-peek {
    margin: 8px 0 0 24px;
    overflow: hidden;
    border: 1px solid var(--hairline);
    border-radius: 6px;
    background: color-mix(in srgb, var(--surface) 86%, transparent);
  }

  .log-lines {
    max-height: 152px;
    overflow: auto;
    padding: 8px;
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1.45;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .log-line {
    min-height: 15px;
  }

  .log-input {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 6px;
    padding: 6px;
    border-top: 1px solid var(--hairline);
  }

  input {
    min-width: 0;
    height: 26px;
    padding: 0 8px;
    border: 1px solid var(--hairline);
    border-radius: 6px;
    color: var(--text-primary);
    background: transparent;
    font: inherit;
  }

  button {
    height: 26px;
    padding: 0 10px;
    border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
    border-radius: 6px;
    color: var(--accent);
    background: transparent;
    font-size: 11px;
    font-weight: 500;
  }

  button:disabled,
  input:disabled {
    opacity: 0.5;
  }

  :global(.ansi-bold) {
    font-weight: 700;
  }

  :global(.ansi-dim) {
    opacity: 0.75;
  }

  :global(.ansi-italic) {
    font-style: italic;
  }

  :global(.ansi-underline) {
    text-decoration: underline;
  }

  :global(.ansi-fg-black),
  :global(.ansi-fg-bright-black) {
    color: var(--text-muted);
  }

  :global(.ansi-fg-red),
  :global(.ansi-fg-bright-red) {
    color: var(--crashed);
  }

  :global(.ansi-fg-green),
  :global(.ansi-fg-bright-green) {
    color: var(--running);
  }

  :global(.ansi-fg-yellow),
  :global(.ansi-fg-bright-yellow) {
    color: var(--waiting);
  }

  :global(.ansi-fg-blue),
  :global(.ansi-fg-bright-blue),
  :global(.ansi-fg-magenta),
  :global(.ansi-fg-bright-magenta),
  :global(.ansi-fg-cyan),
  :global(.ansi-fg-bright-cyan),
  :global(.ansi-fg-white),
  :global(.ansi-fg-bright-white) {
    color: var(--text-primary);
  }
</style>
