<script lang="ts">
  import { LockKeyhole, Square } from "@lucide/svelte";
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import type { PortRowView } from "$lib/snapshot-adapter";

  export type PortActionState = "idle" | "killing" | "needs_privilege" | "failed";

  let {
    ports,
    actionStates = {},
    onKill = () => {}
  }: {
    ports: PortRowView[];
    actionStates?: Record<string, PortActionState>;
    onKill?: (port: PortRowView) => void;
  } = $props();

  function actionLabel(item: PortRowView, state: PortActionState) {
    if (state === "needs_privilege") return `${item.process} needs elevated privileges`;
    if (state === "killing") return `Killing ${item.process}`;
    if (state === "failed") return `Kill ${item.process} failed`;
    return `Kill ${item.process}`;
  }
</script>

<section aria-labelledby="ports-heading">
  <div class="section-heading">
    <h2 id="ports-heading">Ports</h2>
    <span>{ports.length}</span>
  </div>

  <ul>
    {#each ports as item (item.key)}
      {@const actionState = actionStates[item.key] ?? "idle"}
      <li>
        <StatusBadge status={item.status} />
        <span class="port">:{item.port}</span>
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
        <div class="row-actions">
          <button
            class:needs-privilege={actionState === "needs_privilege"}
            class:failed={actionState === "failed"}
            class="act-btn kill"
            type="button"
            title={actionLabel(item, actionState)}
            aria-label={actionLabel(item, actionState)}
            disabled={item.pid === 0 || actionState === "killing" || actionState === "needs_privilege"}
            onclick={() => onKill(item)}
          >
            {#if actionState === "needs_privilege"}
              <LockKeyhole size={13} strokeWidth={1.9} aria-hidden="true" />
            {:else}
              <Square size={10} strokeWidth={2.4} fill="currentColor" aria-hidden="true" />
            {/if}
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

  .act-btn:not(:disabled) {
    cursor: pointer;
  }

  .act-btn:not(:disabled):hover {
    border-color: color-mix(in srgb, var(--crashed) 42%, var(--hairline));
    color: var(--crashed);
    background: color-mix(in srgb, var(--crashed) 8%, transparent);
  }

  .act-btn:disabled {
    opacity: 0.72;
  }

  .act-btn.needs-privilege {
    color: var(--waiting);
  }

  .act-btn.failed {
    color: var(--crashed);
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
