<script lang="ts">
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import type { PortRow } from "$lib/snapshot-adapter";

  let { ports }: { ports: PortRow[] } = $props();
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
            <span class="source" class:source-orphan={item.source === "orphan?"}>{item.source}</span>
          </div>
          <div class="secondary" title={`${item.cwd} · PID ${item.pid} · ${item.cpuPercent.toFixed(1)}% CPU · ${item.memoryMb} MB`}>
            <span class="cwd">{item.cwd}</span>
            <span class="sec-dot" aria-hidden="true">·</span>
            <span class="metric">{item.pid}</span>
            <span class="sec-dot" aria-hidden="true">·</span>
            <span class="metric">{item.cpuPercent.toFixed(1)}%</span>
            <span class="sec-dot" aria-hidden="true">·</span>
            <span class="metric">{item.memoryMb} MB</span>
          </div>
        </div>
        <!-- Presentational until the kill command is wired to the frontend (see-kill). -->
        <div class="row-actions">
          <button class="act-btn kill" type="button" title="Kill process (unavailable)" aria-label={`Kill ${item.process} (unavailable)`} disabled>
            <svg width="9" height="9" viewBox="0 0 12 12" fill="currentColor" aria-hidden="true">
              <rect x="1.5" y="1.5" width="9" height="9" rx="1.5" />
            </svg>
          </button>
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
    grid-template-columns: 16px 52px minmax(0, 1fr) auto;
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
    background: var(--surface-hi);
  }

  .sec-dot {
    flex: 0 0 auto;
    color: var(--text-muted);
    opacity: 0.7;
    user-select: none;
  }

  .row-actions {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
    opacity: 0;
    transition: opacity 100ms ease;
  }

  li:hover .row-actions,
  li:focus-within .row-actions {
    opacity: 1;
  }

  .act-btn {
    display: grid;
    width: 26px;
    height: 26px;
    place-items: center;
    border: 1px solid var(--hairline);
    border-radius: 6px;
    background: transparent;
    color: var(--text-muted);
    cursor: default;
    transition:
      color 100ms ease,
      background 100ms ease,
      border-color 100ms ease;
  }

  .act-btn:disabled {
    opacity: 0.7;
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

  /* Source tags are best-effort hints, not ground truth — so they stay quiet and uniform. */
  .source {
    flex: 0 0 auto;
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 400;
    white-space: nowrap;
  }

  .source-orphan {
    font-style: italic;
  }

  .secondary {
    display: flex;
    min-width: 0;
    gap: 4px;
    margin-top: 3px;
    overflow: hidden;
    color: var(--text-secondary);
    font-size: 11px;
    line-height: 1.25;
    white-space: nowrap;
  }

  .cwd {
    min-width: 20px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .metric {
    flex: 0 0 auto;
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }

  @media (prefers-reduced-motion: reduce) {
    li,
    .row-actions,
    .act-btn {
      transition: none;
    }
  }
</style>
