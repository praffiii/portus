<script lang="ts">
  import { isTauri } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { ChevronDown, FolderOpen, Play, Save, Square } from "@lucide/svelte";

  import LogPeek from "$lib/components/LogPeek.svelte";
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import type { Lifecycle, ManagedStatus, Project } from "$lib/bindings";
  import type { ServiceStatus } from "$lib/snapshot-adapter";

  let {
    projects,
    managed,
    onRun,
    onStop,
    onSave
  }: {
    projects: Project[];
    managed: ManagedStatus[];
    onRun: (command: string, cwd: string) => Promise<string | undefined>;
    onStop: (runId: string) => Promise<string | undefined>;
    onSave: (runId: string) => Promise<string | undefined>;
  } = $props();

  const CHOOSE_FOLDER = "__choose_folder__";
  const OUTPUT_LIMIT = 8;

  let command = $state("");
  let cwd = $state("~");
  let customFolders: string[] = $state([]);
  let submitting = $state(false);
  let runError = $state("");
  let expanded: Record<string, boolean> = $state({});
  let rowActions: Record<string, "stopping" | "saving" | { kind: "failed"; message: string }> =
    $state({});

  const quickRuns = $derived(managed.filter((item) => item.origin.kind === "quick_run"));
  const savedFolders = $derived(projects.map((project) => project.folder));
  const folderOptions = $derived([
    "~",
    ...unique([...savedFolders, ...customFolders].filter((folder) => folder !== "~"))
  ]);

  function unique(values: string[]): string[] {
    return Array.from(new Set(values));
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

  function shortFolder(folder: string): string {
    if (folder === "~") return "~";
    return folder.split("/").filter(Boolean).pop() ?? folder;
  }

  function isTerminal(lifecycle: Lifecycle): boolean {
    return lifecycle === "exited" || lifecycle === "crashed";
  }

  function isFailed(
    action: (typeof rowActions)[string] | undefined
  ): action is { kind: "failed"; message: string } {
    return typeof action === "object" && action.kind === "failed";
  }

  function recentLines(status: ManagedStatus): string[] {
    if (!isTerminal(status.lifecycle)) return [];
    return status.recent_output.slice(-OUTPUT_LIMIT);
  }

  function toggle(runId: string) {
    expanded = { ...expanded, [runId]: !expanded[runId] };
  }

  async function chooseFolder() {
    if (!isTauri()) return;
    const folder = await open({ directory: true, multiple: false });
    if (typeof folder !== "string") return;
    customFolders = unique([...customFolders, folder]);
    cwd = folder;
  }

  async function handleFolderChange(event: Event) {
    const value = (event.currentTarget as HTMLSelectElement).value;
    if (value === CHOOSE_FOLDER) {
      await chooseFolder();
      return;
    }
    cwd = value;
  }

  async function submit() {
    if (submitting) return;

    submitting = true;
    runError = "";
    const error = await onRun(command, cwd);
    if (error) {
      runError = error;
    } else {
      command = "";
    }
    submitting = false;
  }

  async function stop(runId: string) {
    if (rowActions[runId] === "stopping") return;
    rowActions = { ...rowActions, [runId]: "stopping" };
    const error = await onStop(runId);
    if (error) {
      rowActions = { ...rowActions, [runId]: { kind: "failed", message: error } };
    } else {
      const { [runId]: _removed, ...rest } = rowActions;
      rowActions = rest;
    }
  }

  async function save(runId: string) {
    if (rowActions[runId] === "saving") return;
    rowActions = { ...rowActions, [runId]: "saving" };
    const error = await onSave(runId);
    if (error) {
      rowActions = { ...rowActions, [runId]: { kind: "failed", message: error } };
    } else {
      const { [runId]: _removed, ...rest } = rowActions;
      rowActions = rest;
    }
  }
</script>

<section aria-labelledby="quick-run-heading">
  <div class="section-heading">
    <h2 id="quick-run-heading">Quick-run</h2>
    <span>{quickRuns.length}</span>
  </div>

  <form
    class="quick-run-box"
    onsubmit={(event) => {
      event.preventDefault();
      void submit();
    }}
  >
    <input
      bind:value={command}
      class="command-input"
      type="text"
      placeholder="Command"
      spellcheck="false"
      aria-label="Quick-run command"
    />
    <div class="folder-select">
      <FolderOpen size={13} strokeWidth={1.9} aria-hidden="true" />
      <select value={cwd} aria-label="Run in" onchange={handleFolderChange}>
        {#each folderOptions as folder (folder)}
          <option value={folder}>{shortFolder(folder)}</option>
        {/each}
        <option value={CHOOSE_FOLDER}>Choose folder...</option>
      </select>
    </div>
    <button class="run-btn" type="submit" disabled={submitting}>
      <Play size={12} strokeWidth={2.2} fill="currentColor" aria-hidden="true" />
      <span>Run</span>
    </button>
  </form>
  {#if runError}
    <p class="run-error">{runError}</p>
  {/if}

  {#if quickRuns.length > 0}
    <ul>
      {#each quickRuns as status (status.run_id)}
        {@const action = rowActions[status.run_id]}
        {@const output = recentLines(status)}
        <li class="quick-run-row">
          <div class="run-main">
            <StatusBadge status={badgeFor(status.lifecycle)} />
            <div class="details">
              <div class="primary">
                <span class="command" title={status.launch_spec.command}>{status.launch_spec.command}</span>
              </div>
              <div class="secondary" title={status.launch_spec.cwd}>
                <span>{shortFolder(status.launch_spec.cwd)}</span>
                <span class="sec-dot" aria-hidden="true">·</span>
                <span class="lifecycle">{labelFor(status.lifecycle)}</span>
              </div>
            </div>
            <div class="row-actions">
              {#if !isTerminal(status.lifecycle)}
                <button
                  class="act-btn"
                  class:expanded={expanded[status.run_id]}
                  type="button"
                  title={`${expanded[status.run_id] ? "Hide" : "Show"} output`}
                  aria-label={`${expanded[status.run_id] ? "Hide" : "Show"} quick-run output`}
                  onclick={() => toggle(status.run_id)}
                >
                  <ChevronDown size={13} strokeWidth={2.1} aria-hidden="true" />
                </button>
                <button
                  class="act-btn kill"
                  type="button"
                  title="Stop quick-run"
                  aria-label="Stop quick-run"
                  disabled={action === "stopping"}
                  onclick={() => void stop(status.run_id)}
                >
                  <Square size={10} strokeWidth={2.4} fill="currentColor" aria-hidden="true" />
                </button>
              {/if}
              <button
                class="act-btn save"
                type="button"
                title="Save as project"
                aria-label="Save quick-run as project"
                disabled={action === "saving"}
                onclick={() => void save(status.run_id)}
              >
                <Save size={13} strokeWidth={2} aria-hidden="true" />
              </button>
            </div>
          </div>
          {#if isFailed(action)}
            <p class="run-error row-error">{action.message}</p>
          {/if}
          {#if output.length > 0}
            <pre class="recent-output" aria-label="Recent quick-run output">{output.join("\n")}</pre>
          {/if}
          {#if expanded[status.run_id] && !isTerminal(status.lifecycle)}
            <LogPeek runId={status.run_id} readonly={false} terminal={false} />
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
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

  .quick-run-box {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 112px auto;
    gap: 6px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--hairline);
  }

  .command-input,
  select {
    min-width: 0;
    height: 28px;
    border: 1px solid var(--hairline);
    border-radius: 6px;
    color: var(--text-primary);
    background: color-mix(in srgb, var(--surface) 72%, transparent);
    font-family: var(--font-ui);
    font-size: 12px;
    outline: none;
  }

  .command-input {
    padding: 0 8px;
  }

  .command-input::placeholder {
    color: var(--text-muted);
  }

  .command-input:focus,
  select:focus {
    border-color: color-mix(in srgb, var(--accent) 52%, var(--hairline));
  }

  .folder-select {
    position: relative;
    min-width: 0;
  }

  .folder-select :global(svg) {
    position: absolute;
    top: 7px;
    left: 7px;
    color: var(--text-muted);
    pointer-events: none;
  }

  select {
    width: 100%;
    padding: 0 22px 0 24px;
    appearance: none;
  }

  .run-btn {
    display: flex;
    height: 28px;
    align-items: center;
    gap: 5px;
    padding: 0 9px;
    border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
    border-radius: 6px;
    color: var(--accent);
    background: transparent;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
  }

  .run-btn:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }

  .run-btn:disabled {
    opacity: 0.52;
    cursor: default;
  }

  .run-error {
    margin: 7px 12px 8px;
    color: var(--crashed);
    font-size: 11px;
    line-height: 1.35;
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

  .quick-run-row {
    min-height: 50px;
    padding: 8px 12px;
    transition: background-color 100ms ease;
  }

  .quick-run-row:hover {
    background: var(--surface-hi);
  }

  .run-main {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr) auto;
    gap: 8px;
    align-items: center;
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

  .command {
    min-width: 0;
    overflow: hidden;
    color: var(--text-primary);
    font-size: 13px;
    font-weight: 500;
    text-overflow: ellipsis;
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

  .secondary span:first-child {
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
    flex-shrink: 0;
    align-items: center;
    gap: 4px;
    opacity: 0;
    transition: opacity 100ms ease;
  }

  .quick-run-row:hover .row-actions,
  .quick-run-row:focus-within .row-actions {
    opacity: 1;
  }

  .act-btn {
    display: grid;
    width: 26px;
    height: 26px;
    place-items: center;
    border: 1px solid var(--hairline);
    border-radius: 6px;
    color: var(--text-muted);
    background: transparent;
    cursor: pointer;
    transition:
      color 100ms ease,
      background 100ms ease,
      border-color 100ms ease;
  }

  .act-btn.expanded :global(svg) {
    transform: rotate(180deg);
  }

  .act-btn.save:hover {
    border-color: color-mix(in srgb, var(--accent) 30%, var(--hairline));
    color: var(--accent);
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

  .row-error,
  .recent-output {
    margin: 8px 0 0 24px;
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
    .quick-run-row,
    .row-actions,
    .act-btn {
      transition: none;
    }
  }
</style>
