<script module lang="ts">
  export type TaskActionState = "starting" | "stopping" | { kind: "failed"; message: string };
</script>

<script lang="ts">
  import { ChevronDown, Play, Square } from "@lucide/svelte";
  import LogPeek from "$lib/components/LogPeek.svelte";
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import type { Lifecycle, ManagedStatus, Project } from "$lib/bindings";
  import type { ServiceStatus } from "$lib/snapshot-adapter";

  let {
    projects,
    managed,
    taskActions,
    onStart,
    onStop
  }: {
    projects: Project[];
    managed: ManagedStatus[];
    taskActions: Record<string, TaskActionState>;
    onStart: (projectId: string, taskId: string) => void;
    onStop: (pid: number, projectId: string, taskId: string) => void;
  } = $props();

  const OUTPUT_LIMIT = 8;
  let expanded: Record<string, boolean> = $state({});

  function taskKey(projectId: string, taskId: string): string {
    return `${projectId}:${taskId}`;
  }

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

  function isFailed(action: TaskActionState | undefined): action is { kind: "failed"; message: string } {
    return typeof action === "object" && action.kind === "failed";
  }

  function recentLines(status: ManagedStatus | undefined): string[] {
    if (!status || (status.lifecycle !== "exited" && status.lifecycle !== "crashed")) return [];
    return status.recent_output.slice(-OUTPUT_LIMIT);
  }

  function toggle(projectId: string, taskId: string) {
    const key = taskKey(projectId, taskId);
    expanded = { ...expanded, [key]: !expanded[key] };
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
        {@const action = taskActions[taskKey(project.id, task.id)]}
        {@const output = recentLines(status)}
        {@const key = taskKey(project.id, task.id)}
        <li class="task-row" class:has-output={output.length > 0 || isFailed(action)}>
          <div class="task-main">
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
                {:else if action === "starting"}
                  <span class="sec-dot" aria-hidden="true">·</span>
                  <span class="lifecycle">starting</span>
                {/if}
              </div>
            </div>
            <div class="row-actions">
              {#if status && status.lifecycle !== "exited" && status.lifecycle !== "crashed"}
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
              {#if status && status.lifecycle !== "exited" && status.lifecycle !== "crashed"}
                <button
                  class="act-btn kill"
                  type="button"
                  title={`Stop ${task.name}`}
                  aria-label={`Stop ${task.name}`}
                  disabled={action === "stopping"}
                  onclick={() => onStop(status.pid, project.id, task.id)}
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
            <p class="task-error">{action.message}</p>
          {/if}
          {#if output.length > 0}
            <pre class="recent-output" aria-label={`Recent output for ${task.name}`}>{output.join("\n")}</pre>
          {/if}
          {#if status && expanded[key] && status.lifecycle !== "exited" && status.lifecycle !== "crashed"}
            <LogPeek
              projectId={project.id}
              taskId={task.id}
              readonly={false}
              terminal={false}
            />
          {/if}
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
    min-height: 50px;
    padding: 8px 12px;
    transition: background-color 100ms ease;
  }

  .task-main {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr) auto;
    gap: 8px;
    align-items: center;
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

  .act-btn.expanded :global(svg) {
    transform: rotate(180deg);
  }

  .act-btn.start:hover {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }

  .act-btn.kill:hover {
    border-color: color-mix(in srgb, var(--crashed) 42%, var(--hairline));
    color: var(--crashed);
    background: color-mix(in srgb, var(--crashed) 8%, transparent);
  }

  .act-btn:disabled {
    opacity: 0.52;
    cursor: default;
  }

  .act-btn:disabled:hover {
    background: transparent;
  }

  .task-error,
  .recent-output {
    margin: 8px 0 0 24px;
  }

  .task-error {
    color: var(--crashed);
    font-size: 11px;
    line-height: 1.35;
  }

  .recent-output {
    max-height: 92px;
    overflow: auto;
    padding: 7px 8px;
    border: 1px solid var(--hairline);
    border-radius: 6px;
    color: var(--text-secondary);
    background: color-mix(in srgb, var(--surface) 72%, transparent);
    font-family: var(--font-mono);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    line-height: 1.35;
    white-space: pre-wrap;
    word-break: break-word;
  }

  @media (prefers-reduced-motion: reduce) {
    .task-row,
    .row-actions,
    .act-btn {
      transition: none;
    }
  }
</style>
