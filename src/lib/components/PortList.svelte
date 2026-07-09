<script lang="ts">
  import { ChevronDown, FolderPlus, LockKeyhole, Square } from "@lucide/svelte";
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import CollapsibleSection from "$lib/components/ui/CollapsibleSection.svelte";
  import FilterChip from "$lib/components/ui/FilterChip.svelte";
  import type { ManagedStatus, Project } from "$lib/bindings";
  import {
    groupPortsByPid,
    organizePortsByProject,
    sortPortRows,
    type PortFilterMode,
    type PortProcessGroup,
    type PortProjectGroup,
    type PortRowView,
    type PortSourceBucket
  } from "$lib/snapshot-adapter";

  export type PortActionState = "idle" | "killing" | "needs_privilege" | "failed";

  const FILTER_STORAGE_KEY = "portus:port-filter";

  let {
    ports,
    projects = [],
    managed = [],
    actionStates = {},
    onKill = () => {},
    onSaveAs = () => {}
  }: {
    ports: PortRowView[];
    projects?: Project[];
    managed?: ManagedStatus[];
    actionStates?: Record<string, PortActionState>;
    onKill?: (port: PortRowView) => void;
    onSaveAs?: (port: PortRowView) => void;
  } = $props();

  function readFilterMode(): PortFilterMode {
    if (typeof localStorage === "undefined") return "projects";
    const stored = localStorage.getItem(FILTER_STORAGE_KEY);
    if (stored === "all") return "all";
    // Legacy Relevant → Projects
    return "projects";
  }

  let filterMode = $state<PortFilterMode>(readFilterMode());
  let projectExpanded: Record<string, boolean> = $state({});
  let bucketExpanded: Record<string, boolean> = $state({});

  const projectsView = $derived(organizePortsByProject(ports, projects, managed));
  const allGroups = $derived(groupPortsByPid(sortPortRows(ports)));

  function setFilterMode(mode: PortFilterMode) {
    filterMode = mode;
    projectExpanded = {};
    bucketExpanded = {};
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(FILTER_STORAGE_KEY, mode);
    }
  }

  function isProjectOpen(group: PortProjectGroup): boolean {
    if (group.projectId in projectExpanded) return projectExpanded[group.projectId];
    return group.ports.length > 0;
  }

  function toggleProject(group: PortProjectGroup) {
    projectExpanded = { ...projectExpanded, [group.projectId]: !isProjectOpen(group) };
  }

  function isBucketOpen(bucket: PortSourceBucket): boolean {
    return bucketExpanded[bucket.id] ?? false;
  }

  function toggleBucket(bucket: PortSourceBucket) {
    bucketExpanded = { ...bucketExpanded, [bucket.id]: !isBucketOpen(bucket) };
  }

  function primaryPort(group: PortProcessGroup): PortRowView {
    return group.ports[0];
  }

  function groupActionState(group: PortProcessGroup): PortActionState {
    const states = group.ports.map((port) => actionStates[port.key] ?? "idle");
    if (states.includes("killing")) return "killing";
    if (states.includes("needs_privilege")) return "needs_privilege";
    if (states.includes("failed")) return "failed";
    return "idle";
  }

  function portActionState(port: PortRowView): PortActionState {
    return actionStates[port.key] ?? "idle";
  }

  function actionLabel(item: PortRowView, state: PortActionState) {
    if (state === "needs_privilege") return `${item.process} needs elevated privileges`;
    if (state === "killing") return `Killing ${item.process}`;
    if (state === "failed") return `Kill ${item.process} failed`;
    return `Kill ${item.process}`;
  }

  function portListLabel(group: PortProcessGroup): string {
    return group.ports.map((port) => `:${port.port}`).join(" ");
  }

  function folderBasename(folder: string): string {
    return folder.split("/").filter(Boolean).pop() ?? folder;
  }
</script>

{#snippet filterChips()}
  <FilterChip label="Projects" active={filterMode === "projects"} onclick={() => setFilterMode("projects")} />
  <FilterChip label="All" active={filterMode === "all"} onclick={() => setFilterMode("all")} />
{/snippet}

{#snippet processGroup(group: PortProcessGroup, options?: { compact?: boolean; hideSource?: boolean })}
  {@const actionState = groupActionState(group)}
  {@const primary = primaryPort(group)}
  {@const multi = group.ports.length > 1}
  {@const compact = options?.compact ?? false}
  {@const hideSource = options?.hideSource ?? false}
  <li class="list-row process-group" class:has-error={actionState === "failed" || actionState === "needs_privilege"}>
    <div class="row-grid" class:with-port={!multi} class:with-status={multi}>
      <StatusBadge status={group.status} />
      {#if !multi}
        <span class="port">:{primary.port}</span>
      {/if}

      <div class="details">
        <div class="row-primary">
          <span class="row-title" title={group.process}>{group.process}</span>
          {#if !hideSource}
            <span class="row-source" class:row-source-orphan={group.source === "orphan?"}>{group.source}</span>
          {/if}
          {#if multi}
            <span class="port-count">{group.ports.length} ports</span>
          {/if}
        </div>
        <div
          class="row-secondary"
          title={`${group.cwd} · PID ${group.pid} · ${group.cpuPercent.toFixed(1)}% CPU · ${group.memoryMb} MB`}
        >
          {#if !compact}
            <span class="cwd">{group.cwd}</span>
            <span class="sec-dot" aria-hidden="true">·</span>
          {/if}
          <span class="row-metric">{group.pid}</span>
          {#if !compact}
            <span class="sec-dot" aria-hidden="true">·</span>
            <span class="row-metric">{group.cpuPercent.toFixed(1)}%</span>
            <span class="sec-dot" aria-hidden="true">·</span>
            <span class="row-metric">{group.memoryMb} MB</span>
          {/if}
        </div>
        {#if multi}
          <div class="port-preview" title={portListLabel(group)}>
            {#each group.ports as port (port.key)}
              <span class="port-chip">:{port.port}</span>
            {/each}
          </div>
        {/if}
      </div>

      <div class="row-actions">
        {#if !compact}
          <button
            class="act-btn save"
            type="button"
            title={`Save ${group.process} as project`}
            aria-label={`Save ${group.process} as project`}
            disabled={group.pid === 0 || !group.cwd}
            onclick={() => onSaveAs(primary)}
          >
            <FolderPlus size={13} strokeWidth={1.9} aria-hidden="true" />
          </button>
        {/if}
        <button
          class:needs-privilege={actionState === "needs_privilege"}
          class:failed={actionState === "failed"}
          class="act-btn kill"
          type="button"
          title={actionLabel(primary, actionState)}
          aria-label={actionLabel(primary, actionState)}
          disabled={group.pid === 0 || actionState === "killing" || actionState === "needs_privilege"}
          onclick={() => onKill(primary)}
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
      <p class="row-error" class:port-error-offset={!multi}>Stop failed. The port is still listening.</p>
    {:else if actionState === "needs_privilege"}
      <p class="row-error warning" class:port-error-offset={!multi}>Needs elevated privileges to stop this process.</p>
    {/if}
  </li>
{/snippet}

{#snippet matchedPortRow(port: PortRowView)}
  {@const actionState = portActionState(port)}
  <li class="list-row matched-port" class:has-error={actionState === "failed" || actionState === "needs_privilege"}>
    <div class="row-grid with-port">
      <StatusBadge status={port.status} />
      <span class="port">:{port.port}</span>
      <div class="details">
        <div class="row-primary">
          <span class="row-title" title={port.process}>{port.process}</span>
        </div>
        <div
          class="row-secondary"
          title={`${port.cwd} · PID ${port.pid} · ${port.cpuPercent.toFixed(1)}% CPU · ${port.memoryMb} MB`}
        >
          <span class="row-metric">{port.pid}</span>
          <span class="sec-dot" aria-hidden="true">·</span>
          <span class="row-metric">{port.cpuPercent.toFixed(1)}%</span>
          <span class="sec-dot" aria-hidden="true">·</span>
          <span class="row-metric">{port.memoryMb} MB</span>
        </div>
      </div>
      <div class="row-actions">
        <button
          class:needs-privilege={actionState === "needs_privilege"}
          class:failed={actionState === "failed"}
          class="act-btn kill"
          type="button"
          title={actionLabel(port, actionState)}
          aria-label={actionLabel(port, actionState)}
          disabled={port.pid === 0 || actionState === "killing" || actionState === "needs_privilege"}
          onclick={() => onKill(port)}
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
{/snippet}

<CollapsibleSection id="ports" label="Ports" count={ports.length} trailing={filterChips}>
  {#if filterMode === "projects"}
    <ul>
      {#if projectsView.projectGroups.length === 0}
        <li class="empty-hint list-row">
          <span class="empty-text">No project ports yet</span>
        </li>
      {:else}
        {#each projectsView.projectGroups as group (group.projectId)}
          {@const open = isProjectOpen(group)}
          <li class="project-port-group">
            <button
              class="project-port-header list-row"
              type="button"
              aria-expanded={open}
              onclick={() => toggleProject(group)}
            >
              <span class="project-chevron" class:expanded={open}>
                <ChevronDown size={13} strokeWidth={2.1} aria-hidden="true" />
              </span>
              <div class="project-meta">
                <span class="row-title" title={group.folder}>{group.projectName}</span>
                {#if group.folder}
                  <span class="folder-basename">{folderBasename(group.folder)}</span>
                {/if}
              </div>
              <span class="port-badge">{group.ports.length}</span>
            </button>
            {#if open}
              <ul class="matched-ports" aria-label={`${group.projectName} ports`}>
                {#each group.ports as port (port.key)}
                  {@render matchedPortRow(port)}
                {/each}
              </ul>
            {/if}
          </li>
        {/each}
      {/if}

      {#each projectsView.buckets as bucket (bucket.id)}
        {@const open = isBucketOpen(bucket)}
        <li class="source-bucket">
          <button
            class="source-bucket-trigger list-row"
            type="button"
            aria-expanded={open}
            onclick={() => toggleBucket(bucket)}
          >
            <span class="system-glyph" aria-hidden="true">○</span>
            <span class="system-label">{bucket.label}</span>
            <span class="sec-dot" aria-hidden="true">·</span>
            <span class="row-metric">{bucket.ports.length}</span>
            <span class="system-chevron" class:expanded={open} aria-hidden="true">▾</span>
          </button>
          {#if open}
            <ul class="bucket-ports">
              {#each groupPortsByPid(bucket.ports) as group (group.key)}
                {@render processGroup(group, { compact: true })}
              {/each}
            </ul>
          {/if}
        </li>
      {/each}
    </ul>
  {:else}
    <ul>
      {#each allGroups as group (group.key)}
        {@render processGroup(group)}
      {/each}
    </ul>
  {/if}
</CollapsibleSection>

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

  .row-grid.with-status {
    grid-template-columns: 16px minmax(0, 1fr) auto;
  }

  .project-chevron {
    display: grid;
    place-items: center;
    transition: transform var(--motion-fast);
  }

  .project-chevron.expanded {
    transform: rotate(180deg);
  }

  .port-count {
    flex: 0 0 auto;
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .port-preview {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    align-items: center;
    margin-top: 4px;
  }

  .port-chip {
    flex: 0 0 auto;
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
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

  .empty-hint {
    display: flex;
    align-items: center;
    min-height: 36px;
  }

  .empty-hint::before {
    display: none;
  }

  .empty-text {
    color: var(--text-muted);
    font-size: 12px;
  }

  .project-port-group {
    list-style: none;
  }

  .project-port-header {
    display: grid;
    width: 100%;
    grid-template-columns: 20px minmax(0, 1fr) auto;
    gap: 8px;
    align-items: center;
    padding: var(--row-pad-y) var(--row-pad-x);
    border: none;
    color: inherit;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }

  .project-port-header::before {
    display: none;
  }

  .project-port-header:hover {
    background: var(--surface-hi);
  }

  .project-meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .folder-basename {
    overflow: hidden;
    color: var(--text-muted);
    font-size: 11px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .port-badge {
    flex: 0 0 auto;
    padding: 1px 6px;
    border-radius: 6px;
    color: var(--text-secondary);
    background: var(--surface-pressed);
    font-family: var(--font-mono);
    font-size: 10px;
    font-variant-numeric: tabular-nums;
  }

  .matched-ports {
    padding: 0;
    margin: 0;
    list-style: none;
    border-top: 1px solid var(--hairline);
  }

  .matched-ports :global(.list-row) {
    padding-left: calc(var(--row-pad-x) + 20px);
  }

  .source-bucket-trigger {
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

  .source-bucket-trigger::before {
    display: none;
  }

  .source-bucket-trigger:hover {
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

  .bucket-ports {
    padding: 0;
    margin: 0;
    list-style: none;
    border-top: 1px solid var(--hairline);
  }

  .bucket-ports :global(.list-row) {
    padding-left: calc(var(--row-pad-x) + 8px);
  }

  @media (prefers-reduced-motion: reduce) {
    .project-chevron,
    .system-chevron {
      transition: none;
    }
  }
</style>
