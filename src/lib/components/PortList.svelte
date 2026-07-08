<script lang="ts">
  import { FolderPlus, LockKeyhole, Square } from "@lucide/svelte";
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import FilterChip from "$lib/components/ui/FilterChip.svelte";
  import SectionHeader from "$lib/components/ui/SectionHeader.svelte";
  import {
    partitionPorts,
    sortPortRows,
    type PortFilterMode,
    type PortRowView
  } from "$lib/snapshot-adapter";

  export type PortActionState = "idle" | "killing" | "needs_privilege" | "failed";

  const FILTER_STORAGE_KEY = "portus:port-filter";

  let {
    ports,
    actionStates = {},
    onKill = () => {},
    onSaveAs = () => {}
  }: {
    ports: PortRowView[];
    actionStates?: Record<string, PortActionState>;
    onKill?: (port: PortRowView) => void;
    onSaveAs?: (port: PortRowView) => void;
  } = $props();

  function readFilterMode(): PortFilterMode {
    if (typeof localStorage === "undefined") return "relevant";
    const stored = localStorage.getItem(FILTER_STORAGE_KEY);
    return stored === "all" ? "all" : "relevant";
  }

  let filterMode = $state<PortFilterMode>(readFilterMode());
  let systemExpanded = $state(false);

  const partitioned = $derived(partitionPorts(ports));
  const visiblePorts = $derived(
    filterMode === "all" ? sortPortRows(ports) : partitioned.relevant
  );
  const systemPorts = $derived(partitioned.system);
  const showSystemGroup = $derived(filterMode === "relevant" && systemPorts.length > 0);

  function setFilterMode(mode: PortFilterMode) {
    filterMode = mode;
    systemExpanded = false;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(FILTER_STORAGE_KEY, mode);
    }
  }

  function actionLabel(item: PortRowView, state: PortActionState) {
    if (state === "needs_privilege") return `${item.process} needs elevated privileges`;
    if (state === "killing") return `Killing ${item.process}`;
    if (state === "failed") return `Kill ${item.process} failed`;
    return `Kill ${item.process}`;
  }
</script>

{#snippet filterChips()}
  <FilterChip label="Relevant" active={filterMode === "relevant"} onclick={() => setFilterMode("relevant")} />
  <FilterChip label="All" active={filterMode === "all"} onclick={() => setFilterMode("all")} />
{/snippet}

<section class="list-section" aria-labelledby="ports-heading">
  <SectionHeader
    id="ports-heading"
    label="Ports"
    count={ports.length}
    trailing={filterChips}
  />

  <ul>
    {#each visiblePorts as item (item.key)}
      {@const actionState = actionStates[item.key] ?? "idle"}
      <li class="list-row" class:has-error={actionState === "failed" || actionState === "needs_privilege"}>
        <div class="row-grid with-port">
          <StatusBadge status={item.status} />
          <span class="port">:{item.port}</span>
          <div class="details">
            <div class="row-primary">
              <span class="row-title" title={item.process}>{item.process}</span>
              <span class="row-source" class:row-source-orphan={item.source === "orphan?"}>{item.source}</span>
            </div>
            <div
              class="row-secondary"
              title={`${item.cwd} · PID ${item.pid} · ${item.cpuPercent.toFixed(1)}% CPU · ${item.memoryMb} MB`}
            >
              <span class="cwd">{item.cwd}</span>
              <span class="sec-dot" aria-hidden="true">·</span>
              <span class="row-metric">{item.pid}</span>
              <span class="sec-dot" aria-hidden="true">·</span>
              <span class="row-metric">{item.cpuPercent.toFixed(1)}%</span>
              <span class="sec-dot" aria-hidden="true">·</span>
              <span class="row-metric">{item.memoryMb} MB</span>
            </div>
          </div>
          <div class="row-actions">
            <button
              class="act-btn save"
              type="button"
              title={`Save ${item.process} as project`}
              aria-label={`Save ${item.process} as project`}
              disabled={item.pid === 0 || !item.cwd}
              onclick={() => onSaveAs(item)}
            >
              <FolderPlus size={13} strokeWidth={1.9} aria-hidden="true" />
            </button>
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
        </div>
        {#if actionState === "failed"}
          <p class="row-error port-error-offset">Stop failed. The port is still listening.</p>
        {:else if actionState === "needs_privilege"}
          <p class="row-error warning port-error-offset">Needs elevated privileges to stop this process.</p>
        {/if}
      </li>
    {/each}

    {#if showSystemGroup}
      <li class="system-group">
        <button
          class="system-group-trigger list-row"
          type="button"
          aria-expanded={systemExpanded}
          onclick={() => (systemExpanded = !systemExpanded)}
        >
          <span class="system-glyph" aria-hidden="true">○</span>
          <span class="system-label">System ports</span>
          <span class="sec-dot" aria-hidden="true">·</span>
          <span class="row-metric">{systemPorts.length}</span>
          <span class="system-chevron" class:expanded={systemExpanded} aria-hidden="true">▾</span>
        </button>
        {#if systemExpanded}
          <ul class="system-ports">
            {#each systemPorts as item (item.key)}
              {@const actionState = actionStates[item.key] ?? "idle"}
              <li class="list-row system-port-row">
                <div class="row-grid with-port">
                  <StatusBadge status={item.status} />
                  <span class="port">:{item.port}</span>
                  <div class="details">
                    <div class="row-primary">
                      <span class="row-title" title={item.process}>{item.process}</span>
                      <span class="row-source">{item.source}</span>
                    </div>
                    <div class="row-secondary" title={`PID ${item.pid}`}>
                      <span class="row-metric">{item.pid}</span>
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
                </div>
              </li>
            {/each}
          </ul>
        {/if}
      </li>
    {/if}
  </ul>
</section>

<style>
  .port {
    overflow: hidden;
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 500;
    font-variant-numeric: tabular-nums;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .cwd {
    min-width: 20px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .act-btn.needs-privilege {
    color: var(--waiting);
  }

  .act-btn.failed {
    color: var(--crashed);
  }

  .port-error-offset {
    margin-left: 76px;
  }

  .system-group-trigger {
    display: grid;
    width: 100%;
    grid-template-columns: 16px auto auto auto 1fr auto;
    gap: 6px;
    align-items: center;
    padding: var(--row-pad-y) var(--row-pad-x);
    border: none;
    color: var(--text-muted);
    background: transparent;
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  .system-group-trigger::before {
    display: none;
  }

  .system-group-trigger:hover {
    color: var(--text-secondary);
    background: var(--surface-hi);
  }

  .system-glyph {
    color: var(--stopped);
    font-size: 12px;
    line-height: 1;
  }

  .system-label {
    font-weight: 500;
  }

  .system-chevron {
    justify-self: end;
    color: var(--text-muted);
    font-size: 11px;
    transition: transform var(--motion-fast);
  }

  .system-chevron.expanded {
    transform: rotate(180deg);
  }

  .system-ports {
    padding: 0;
    margin: 0;
    list-style: none;
    border-top: 1px solid var(--hairline);
  }

  .system-port-row {
    padding-left: calc(var(--row-pad-x) + 8px);
  }

  @media (prefers-reduced-motion: reduce) {
    .system-chevron {
      transition: none;
    }
  }
</style>
