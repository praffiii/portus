<script lang="ts">
  import { isTauri } from "@tauri-apps/api/core";
  import { ChevronDown, FolderOpen, Play, Save, Square } from "@lucide/svelte";

  import LogPeek from "$lib/components/LogPeek.svelte";
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import SectionHeader from "$lib/components/ui/SectionHeader.svelte";
  import { commands, type Lifecycle, type ManagedStatus, type Project } from "$lib/bindings";
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

  let command = $state("");
  let cwd = $state("~");
  let customFolders: string[] = $state([]);
  let submitting = $state(false);
  let runError = $state("");
  let formExpanded = $state(false);
  let expanded: Record<string, boolean> = $state({});
  let rowActions: Record<string, "stopping" | "saving" | { kind: "failed"; message: string }> =
    $state({});
  let commandInput: HTMLInputElement | undefined = $state();

  const quickRuns = $derived(managed.filter((item) => item.origin.kind === "quick_run"));
  const savedFolders = $derived(projects.map((project) => project.folder));
  const folderOptions = $derived([
    "~",
    ...unique([...savedFolders, ...customFolders].filter((folder) => folder !== "~"))
  ]);

  $effect(() => {
    if (quickRuns.length > 0) return;
    if (!submitting && !command.trim() && formExpanded) {
      formExpanded = false;
    }
  });

  function unique(values: string[]): string[] {
    return Array.from(new Set(values));
  }

  export function focusCommand() {
    formExpanded = true;
    queueMicrotask(() => {
      commandInput?.focus();
      commandInput?.select();
    });
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

  function toggle(runId: string) {
    expanded = { ...expanded, [runId]: !expanded[runId] };
  }

  async function chooseFolder(previousCwd: string) {
    if (!isTauri()) return;
    const result = await commands.pickFolder();
    if (result.status === "error" || result.data === null) {
      cwd = previousCwd;
      return;
    }
    const folder = result.data;
    customFolders = unique([...customFolders, folder]);
    cwd = folder;
  }

  async function handleFolderChange(event: Event) {
    const value = (event.currentTarget as HTMLSelectElement).value;
    if (value === CHOOSE_FOLDER) {
      await chooseFolder(cwd);
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

<section class="list-section" aria-labelledby="quick-run-heading">
  <SectionHeader id="quick-run-heading" label="Quick-run" count={quickRuns.length} />

  {#if !formExpanded}
    <button class="collapsed-trigger list-row" type="button" onclick={() => (formExpanded = true)}>
      <Play size={12} strokeWidth={2.2} fill="currentColor" aria-hidden="true" />
      <span>Run a command…</span>
    </button>
  {:else}
    <form
      class="quick-run-box"
      onsubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <input
        bind:this={commandInput}
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
  {/if}

  {#if quickRuns.length > 0}
    <ul>
      {#each quickRuns as status (status.run_id)}
        {@const action = rowActions[status.run_id]}
        {@const terminal = isTerminal(status.lifecycle)}
        <li class="list-row quick-run-row">
          <div class="row-grid">
            <StatusBadge status={badgeFor(status.lifecycle)} />
            <div class="details">
              <div class="row-primary">
                <span class="row-title" title={status.launch_spec.command}>{status.launch_spec.command}</span>
              </div>
              <div class="row-secondary" title={status.launch_spec.cwd}>
                <span>{shortFolder(status.launch_spec.cwd)}</span>
                <span class="sec-dot" aria-hidden="true">·</span>
                <span class="row-metric">{labelFor(status.lifecycle)}</span>
              </div>
            </div>
            <div class="row-actions">
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
              {#if !terminal}
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
            <p class="row-error">{action.message}</p>
          {/if}
          {#if expanded[status.run_id]}
            <LogPeek runId={status.run_id} readonly={terminal} terminal={terminal} />
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .collapsed-trigger {
    display: flex;
    width: 100%;
    align-items: center;
    gap: 8px;
    padding: var(--row-pad-y) var(--row-pad-x);
    border: none;
    border-bottom: 1px solid var(--hairline);
    color: var(--text-muted);
    background: transparent;
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  .collapsed-trigger:hover {
    color: var(--text-primary);
  }

  .collapsed-trigger::before {
    display: none;
  }

  .quick-run-box {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 112px auto;
    gap: 6px;
    padding: var(--row-pad-y) var(--row-pad-x);
    border-bottom: 1px solid var(--hairline);
  }

  .command-input,
  select {
    min-width: 0;
    height: 28px;
    border: 1px solid var(--glass-border);
    border-radius: 8px;
    color: var(--text-primary);
    background: var(--surface);
    backdrop-filter: var(--glass-blur-chrome);
    -webkit-backdrop-filter: var(--glass-blur-chrome);
    box-shadow: inset 0 1px 0 var(--glass-specular);
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
    border-color: var(--hairline-strong);
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
    border: 1px solid var(--glass-border);
    border-radius: 8px;
    color: var(--text-primary);
    background: var(--surface);
    backdrop-filter: var(--glass-blur-chrome);
    -webkit-backdrop-filter: var(--glass-blur-chrome);
    box-shadow: inset 0 1px 0 var(--glass-specular);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
  }

  .run-btn:hover:not(:disabled) {
    background: var(--surface-pressed);
  }

  .run-btn:disabled {
    opacity: 0.52;
    cursor: default;
  }

  .run-error {
    margin: 7px var(--row-pad-x) 8px;
    color: var(--crashed);
    font-size: 11px;
    line-height: 1.35;
  }
</style>
