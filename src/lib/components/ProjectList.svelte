<script module lang="ts">
  export type TaskActionState = "starting" | "stopping" | { kind: "failed"; message: string };
</script>

<script lang="ts">
  import { ChevronDown, Play, Square, Trash2 } from "@lucide/svelte";
  import LogPeek from "$lib/components/LogPeek.svelte";
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import SectionHeader from "$lib/components/ui/SectionHeader.svelte";
  import type { Lifecycle, ManagedStatus, Project } from "$lib/bindings";
  import type { ServiceStatus } from "$lib/snapshot-adapter";

  let {
    projects,
    managed,
    taskActions,
    onStart,
    onStop,
    onRemove = () => {}
  }: {
    projects: Project[];
    managed: ManagedStatus[];
    taskActions: Record<string, TaskActionState>;
    onStart: (projectId: string, taskId: string) => void;
    onStop: (runId: string, projectId: string, taskId: string) => void;
    onRemove?: (project: Project) => void;
  } = $props();

  let expanded: Record<string, boolean> = $state({});
  let projectExpanded: Record<string, boolean> = $state({});

  function taskKey(projectId: string, taskId: string): string {
    return `${projectId}:${taskId}`;
  }

  function statusFor(projectId: string, taskId: string): ManagedStatus | undefined {
    return managed.find(
      (m) =>
        m.origin.kind === "project" &&
        m.origin.project_id === projectId &&
        m.origin.task_id === taskId
    );
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

  function isFailed(action: TaskActionState | undefined): action is { kind: "failed"; message: string } {
    return typeof action === "object" && action.kind === "failed";
  }

  function isTerminalLifecycle(lifecycle: Lifecycle): boolean {
    return lifecycle === "exited" || lifecycle === "crashed";
  }

  function isActiveLifecycle(lifecycle: Lifecycle): boolean {
    return (
      lifecycle === "running" ||
      lifecycle === "running_no_port" ||
      lifecycle === "waiting" ||
      lifecycle === "starting"
    );
  }

  function runningCount(project: Project): number {
    return project.tasks.filter((task) => {
      const status = statusFor(project.id, task.id);
      return status && isActiveLifecycle(status.lifecycle);
    }).length;
  }

  function shouldExpandProject(project: Project): boolean {
    return runningCount(project) > 0;
  }

  function isProjectExpanded(projectId: string, project: Project): boolean {
    if (projectId in projectExpanded) return projectExpanded[projectId];
    return shouldExpandProject(project);
  }

  function folderBasename(folder: string): string {
    return folder.split("/").filter(Boolean).pop() ?? folder;
  }

  function toggle(projectId: string, taskId: string) {
    const key = taskKey(projectId, taskId);
    expanded = { ...expanded, [key]: !expanded[key] };
  }

  function toggleProject(projectId: string, project: Project) {
    const next = !isProjectExpanded(projectId, project);
    projectExpanded = { ...projectExpanded, [projectId]: next };
  }

  function removeProject(event: MouseEvent, project: Project) {
    event.stopPropagation();
    onRemove(project);
  }
</script>

{#if projects.length > 0}
  <section class="list-section" aria-labelledby="projects-heading">
    <SectionHeader id="projects-heading" label="Projects" count={projects.length} />

    <ul>
      {#each projects as project (project.id)}
        {@const activeCount = runningCount(project)}
        {@const open = isProjectExpanded(project.id, project)}
        <li class="project-group">
          <div class="project-header-row list-row">
            <button
              class="project-header"
              type="button"
              aria-expanded={open}
              onclick={() => toggleProject(project.id, project)}
            >
              <span class="project-chevron" class:expanded={open}>
                <ChevronDown size={13} strokeWidth={2.1} aria-hidden="true" />
              </span>
              <div class="project-meta">
                <span class="row-title" title={project.folder}>{project.name}</span>
                {#if folderBasename(project.folder) !== project.name}
                  <span class="folder-basename">{folderBasename(project.folder)}</span>
                {/if}
              </div>
              {#if activeCount > 0}
                <span class="task-badge">{activeCount}/{project.tasks.length}</span>
              {:else}
                <span class="task-badge muted">{project.tasks.length}</span>
              {/if}
            </button>
            <div class="row-actions project-actions">
              <button
                class="act-btn remove"
                type="button"
                title={`Remove ${project.name}`}
                aria-label={`Remove ${project.name}`}
                onclick={(event) => removeProject(event, project)}
              >
                <Trash2 size={13} strokeWidth={1.9} aria-hidden="true" />
              </button>
            </div>
          </div>

          {#if open}
            <ul class="task-list">
              {#each project.tasks as task (task.id)}
                {@const status = statusFor(project.id, task.id)}
                {@const action = taskActions[taskKey(project.id, task.id)]}
                {@const key = taskKey(project.id, task.id)}
                {@const terminal = status ? isTerminalLifecycle(status.lifecycle) : false}
                <li class="list-row task-row">
                  <div class="row-grid">
                    <StatusBadge status={status ? badgeFor(status.lifecycle) : "stopped"} />
                    <div class="details">
                      <div class="row-primary">
                        <span class="row-title" title={task.command}>{task.name}</span>
                        {#if status}
                          <span class="row-source">Portus-started</span>
                        {/if}
                      </div>
                      <div class="row-secondary" title={task.command}>
                        <span class="command">{task.command}</span>
                        {#if status}
                          <span class="sec-dot" aria-hidden="true">·</span>
                          <span class="row-metric">{labelFor(status.lifecycle)}</span>
                        {:else if action === "starting"}
                          <span class="sec-dot" aria-hidden="true">·</span>
                          <span class="row-metric">starting</span>
                        {/if}
                      </div>
                    </div>
                    <div class="row-actions">
                      {#if status}
                        <button
                          class="act-btn"
                          class:expanded={expanded[key]}
                          type="button"
                          title={`${expanded[key] ? "Hide" : "Show"} output for ${task.name}`}
                          aria-label={`${expanded[key] ? "Hide" : "Show"} output for ${task.name}`}
                          onclick={() => toggle(project.id, task.id)}
                        >
                          <ChevronDown size={13} strokeWidth={2.1} aria-hidden="true" />
                        </button>
                      {/if}
                      {#if status && !terminal}
                        <button
                          class="act-btn kill"
                          type="button"
                          title={`Stop ${task.name}`}
                          aria-label={`Stop ${task.name}`}
                          disabled={action === "stopping"}
                          onclick={() => onStop(status.run_id, project.id, task.id)}
                        >
                          <Square size={10} strokeWidth={2.4} fill="currentColor" aria-hidden="true" />
                        </button>
                      {:else}
                        <button
                          class="act-btn start"
                          type="button"
                          title={`Start ${task.name}`}
                          aria-label={`Start ${task.name}`}
                          disabled={action === "starting"}
                          onclick={() => onStart(project.id, task.id)}
                        >
                          <Play size={13} strokeWidth={2.2} fill="currentColor" aria-hidden="true" />
                        </button>
                      {/if}
                    </div>
                  </div>
                  {#if isFailed(action)}
                    <p class="row-error">{action.message}</p>
                  {/if}
                  {#if status && expanded[key]}
                    <LogPeek
                      runId={status.run_id}
                      projectId={project.id}
                      taskId={task.id}
                      readonly={terminal}
                      terminal={terminal}
                    />
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        </li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .project-group {
    border-bottom: 1px solid var(--hairline);
  }

  .project-group:last-child {
    border-bottom: 0;
  }

  .project-header-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 4px;
    align-items: center;
    padding: 0;
  }

  .project-header-row::before {
    display: none;
  }

  .project-header-row:hover {
    background: var(--surface-hi);
  }

  .project-header {
    display: grid;
    width: 100%;
    min-width: 0;
    grid-template-columns: 16px minmax(0, 1fr) auto;
    gap: 8px;
    align-items: center;
    padding: var(--row-pad-y) 0 var(--row-pad-y) var(--row-pad-x);
    border: none;
    border-bottom: 0;
    color: inherit;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }

  .project-actions {
    padding-right: var(--row-pad-x);
  }

  .project-chevron {
    display: grid;
    place-items: center;
    color: var(--text-muted);
    transition: transform var(--motion-fast);
  }

  .project-chevron.expanded {
    transform: rotate(180deg);
  }

  .project-meta {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: 2px;
  }

  .folder-basename {
    overflow: hidden;
    color: var(--text-muted);
    font-size: 11px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .task-badge {
    flex-shrink: 0;
    padding: 2px 6px;
    border-radius: 6px;
    background: var(--accent-subtle);
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: 10px;
    font-variant-numeric: tabular-nums;
  }

  .task-badge.muted {
    color: var(--text-muted);
  }

  .task-list {
    padding: 0;
    margin: 0;
    list-style: none;
    border-left: 2px solid var(--hairline);
    margin-left: 20px;
  }

  .task-row {
    border-bottom: 1px solid var(--hairline);
  }

  .task-row:last-child {
    border-bottom: 0;
  }

  .command {
    min-width: 20px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  @media (prefers-reduced-motion: reduce) {
    .project-chevron {
      transition: none;
    }
  }
</style>
