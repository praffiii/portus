<script lang="ts">
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import type { PortItem, PortSource } from "$lib/types";

  let { ports }: { ports: PortItem[] } = $props();

  const sourceClass: Record<PortSource, string> = {
    system: "source-system",
    "from IDE": "source-ide",
    "from Terminal": "source-terminal",
    "orphan?": "source-orphan"
  };
</script>

<section aria-labelledby="ports-heading">
  <div class="section-heading">
    <h2 id="ports-heading">Ports</h2>
    <span>{ports.length}</span>
  </div>

  <ul>
    {#each ports as item (`${item.pid}:${item.port ?? "none"}`)}
      <li>
        <StatusBadge status={item.status} />
        <span class:port-muted={item.port === null} class="port">{item.port === null ? "—" : `:${item.port}`}</span>
        <div class="details">
          <div class="primary">
            <span class="process" title={item.process}>{item.process}</span>
            <span class={`source ${sourceClass[item.source]}`}>{item.source}</span>
          </div>
          <div class="secondary" title={`${item.cwd} · PID ${item.pid} · ${item.cpuPercent}% CPU · ${item.memoryMb} MB`}>
            <span class="cwd">{item.cwd}</span>
            <span aria-hidden="true">·</span>
            <span class="metric">PID {item.pid}</span>
            <span aria-hidden="true">·</span>
            <span class="metric">{item.cpuPercent}% CPU</span>
            <span aria-hidden="true">·</span>
            <span class="metric">{item.memoryMb} MB</span>
          </div>
        </div>
      </li>
    {/each}
  </ul>
</section>

<style>
  section {
    border-bottom: 1px solid var(--hairline);
  }

  .section-heading {
    position: sticky;
    z-index: 2;
    top: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 32px;
    padding: 0 12px;
    border-bottom: 1px solid var(--hairline);
    color: var(--text-muted);
    background: var(--app-bg);
  }

  h2,
  .section-heading span {
    margin: 0;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  ul {
    padding: 0;
    margin: 0;
    list-style: none;
  }

  li {
    display: grid;
    grid-template-columns: 16px 54px minmax(0, 1fr);
    gap: 8px;
    align-items: center;
    min-height: 54px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--hairline);
    outline: none;
    transition: background-color 100ms ease;
  }

  li:last-child {
    border-bottom: 0;
  }

  li:hover {
    background: var(--surface);
  }

  .port,
  .metric {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }

  .port {
    overflow: hidden;
    color: var(--text-primary);
    font-size: 12px;
    font-weight: 500;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .port-muted {
    color: var(--text-muted);
  }

  .details {
    min-width: 0;
  }

  .primary {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 7px;
  }

  .process {
    min-width: 0;
    overflow: hidden;
    color: var(--text-primary);
    font-size: 13px;
    font-weight: 500;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .source {
    flex: 0 0 auto;
    font-size: 11px;
    font-weight: 500;
    white-space: nowrap;
  }

  .source-system {
    color: #6b7280;
  }

  .source-ide {
    color: #7c3aed;
  }

  .source-terminal {
    color: #475569;
  }

  .source-orphan {
    color: #d97706;
  }

  .secondary {
    display: flex;
    min-width: 0;
    gap: 5px;
    margin-top: 3px;
    overflow: hidden;
    color: var(--text-muted);
    font-size: 11px;
    line-height: 1.25;
    white-space: nowrap;
  }

  .cwd {
    min-width: 24px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .metric {
    flex: 0 0 auto;
  }

  @media (prefers-color-scheme: dark) {
    .source-system {
      color: #9ca3af;
    }

    .source-ide {
      color: #a78bfa;
    }

    .source-terminal {
      color: #94a3b8;
    }

    .source-orphan {
      color: #fbbf24;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    li {
      transition: none;
    }
  }
</style>
