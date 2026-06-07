<script lang="ts">
  import type { ServiceStatus } from "$lib/snapshot-adapter";

  let { status }: { status: ServiceStatus } = $props();

  const statusDetails: Record<ServiceStatus, { glyph: string; label: string }> = {
    running: { glyph: "●", label: "Running" },
    waiting: { glyph: "◐", label: "Waiting" },
    stopped: { glyph: "○", label: "Stopped" },
    crashed: { glyph: "✕", label: "Crashed" }
  };
</script>

<span class:status-running={status === "running"} class:status-waiting={status === "waiting"} class:status-stopped={status === "stopped"} class:status-crashed={status === "crashed"} class="status">
  <span aria-hidden="true">{statusDetails[status].glyph}</span>
  <span class="sr-only">{statusDetails[status].label}</span>
</span>

<style>
  .status {
    display: inline-grid;
    width: 16px;
    height: 20px;
    flex: 0 0 16px;
    place-items: center;
    font-size: 12px;
    line-height: 1;
    transition: color 200ms ease;
  }

  .status-running {
    color: #16a34a;
  }

  .status-waiting {
    color: #d97706;
  }

  .status-stopped {
    color: #9ca3af;
  }

  .status-crashed {
    color: #dc2626;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }

  @media (prefers-color-scheme: dark) {
    .status-running {
      color: #4ade80;
    }

    .status-waiting {
      color: #fbbf24;
    }

    .status-crashed {
      color: #f87171;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .status {
      transition: none;
    }
  }
</style>
