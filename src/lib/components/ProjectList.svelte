<script lang="ts">
  import { Play, Square } from "@lucide/svelte";
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import type { Lifecycle, ManagedStatus, Project } from "$lib/bindings";
  import type { ServiceStatus } from "$lib/snapshot-adapter";

  let {
    projects,
    managed,
    onStart,
    onStop
  }: {
    projects: Project[];
    managed: ManagedStatus[];
    onStart: (projectId: string, taskId: string) => void;
    onStop: (pid: number) => void;
  } = $props();

  function statusFor(projectId: string, taskId: string): ManagedStatus | undefined {
    return managed.find((m) => m.project_id === projectId && m.task_id === taskId);
  }

  function badgeFor(lifecycle: Lifecycle): ServiceStatus {
    if (lifecycle === "running") return "running";
    if (lifecycle === "crashed") return "crashed";
    if (lifecycle === "exited") return "stopped";
    return "waiting";
  }

  function labelFor(lifecycle: Lifecycle): string {
    return lifecycle.replaceAll("_", " ");
  }
</script>

<section aria-labelledby="projects-heading">
  <div class="section-heading">
    <h2 id="projects-heading">Projects</h2>
    <span>{projects.length}</span>
  </div>

  <ul>
    {#each projects as project (project.id)}
      <li class="project-row">
        <div class="project-name" title={project.folder}>{project.name}</div>
      </li>
      {#each project.tasks as task (task.id)}
        {@const status = statusFor(project.id, task.id)}
        <li class="task-row">
          <StatusBadge status={status ? badgeFor(status.lifecycle) : "stopped"} />
          <div class="details">
            <div class="primary">
              <span class="task-name" title={task.command}>{task.name}</span>
              {#if status}
                <span class="source">Portus-started</span>
              {/if}
            </div>
            <div class="secondary" title={task.command}>
              <span class="command">{task.command}</span>
              {#if status}
                <span class="sec-dot" aria-hidden="true">·</span>
                <span class="lifecycle">{labelFor(status.lifecycle)}</span>
              {/if}
            </div>
          </div>
          <div class="row-actions">
            {#if status && status.lifecycle !== "exited" && status.lifecycle !== "crashed"}
              <button class="act-btn kill" type="button" title={`Stop ${task.name}`} aria-label={`Stop ${task.name}`} onclick={() => onStop(status.pid)}>
                <Square size={10} strokeWidth={2.4} fill="currentColor" aria-hidden="true" />
              </button>
            {:else}
              <button class="act-btn start" type="button" title={`Start ${task.name}`} aria-label={`Start ${task.name}`} onclick={() => onStart(project.id, task.id)}>
                <Play size={13} strokeWidth={2.2} fill="currentColor" aria-hidden="true" />
              </button>
            {/if}
          </div>
        </li>
      {/each}
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
    border-bottom: 1px solid var(--hairline);
  }

  li:last-child {
    border-bottom: 0;
  }

  .project-row {
    display: flex;
    min-height: 30px;
    align-items: center;
    padding: 6px 12px;
    color: var(--text-primary);
    background: color-mix(in srgb, var(--surface) 66%, transparent);
  }

  .project-name {
    min-width: 0;
    overflow: hidden;
    font-size: 13px;
    font-weight: 500;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .task-row {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr) auto;
    gap: 8px;
    align-items: center;
    min-height: 50px;
    padding: 8px 12px;
    transition: background-color 100ms ease;
  }

  .task-row:hover {
    background: var(--surface-hi);
  }

  .details {
    min-width: 0;
  }

  .primary,
  .secondary {
    display: flex;
    min-width: 0;
    align-items: baseline;
  }

  .primary {
    gap: 8px;
  }

  .task-name {
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
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 400;
    white-space: nowrap;
  }

  .secondary {
    gap: 4px;
    margin-top: 3px;
    overflow: hidden;
    color: var(--text-secondary);
    font-size: 11px;
    line-height: 1.25;
    white-space: nowrap;
  }

  .command {
    min-width: 20px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .lifecycle,
  .sec-dot {
    flex: 0 0 auto;
    color: var(--text-muted);
  }

  .lifecycle {
    font-family: var(--font-mono);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }

  .row-actions {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
    opacity: 0;
    transition: opacity 100ms ease;
  }

  .task-row:hover .row-actions,
  .task-row:focus-within .row-actions {
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
    cursor: pointer;
    transition:
      color 100ms ease,
      background 100ms ease,
      border-color 100ms ease;
  }

  .act-btn.start {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 30%, transparent);
  }

  .act-btn.start:hover {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }

  .act-btn.kill:hover {
    border-color: color-mix(in srgb, var(--crashed) 42%, var(--hairline));
    color: var(--crashed);
    background: color-mix(in srgb, var(--crashed) 8%, transparent);
  }

  @media (prefers-reduced-motion: reduce) {
    .task-row,
    .row-actions,
    .act-btn {
      transition: none;
    }
  }
</style>
