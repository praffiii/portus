<script lang="ts">
  import { isTauri } from "@tauri-apps/api/core";
  import { Anchor, FolderOpen, Play, Plus, Settings } from "@lucide/svelte";
  import { onMount } from "svelte";

  import { commands, events, type Project, type Snapshot } from "$lib/bindings";
  import DockerList from "$lib/components/DockerList.svelte";
  import PortList, { type PortActionState } from "$lib/components/PortList.svelte";
  import ProjectList, { type TaskActionState } from "$lib/components/ProjectList.svelte";
  import QuickRun from "$lib/components/QuickRun.svelte";
  import SettingsPanel from "$lib/components/SettingsPanel.svelte";
  import IconButton from "$lib/components/ui/IconButton.svelte";
  import { snapshotFixture } from "$lib/fixtures";
  import { containersToDockerRows, snapshotToPortRows, type PortRowView } from "$lib/snapshot-adapter";

  const BLUR_IDLE_DELAY_MS = 200;
  const ACTIVE_SAFETY_IDLE_MS = 30_000;

  let snapshot: Snapshot = $state(snapshotFixture);
  let projects: Project[] = $state([]);
  let portActionStates: Record<string, PortActionState> = $state({});
  let taskActions: Record<string, TaskActionState> = $state({});
  let quickRunComponent: QuickRun | undefined = $state();
  let settingsOpen = $state(false);
  const managed = $derived(snapshot.managed);
  const ports = $derived(snapshotToPortRows(snapshot));
  const containers = $derived(containersToDockerRows(snapshot.docker.data.containers));
  const isEmpty = $derived(
    snapshot.ports.data.length === 0 &&
      snapshot.docker.data.containers.length === 0 &&
      projects.length === 0 &&
      snapshot.managed.length === 0
  );
  const runningCount = $derived(
    ports.filter((item) => item.status === "running").length +
      containers.filter((item) => item.status === "running").length
  );
  const waitingCount = $derived(
    ports.filter((item) => item.status === "waiting").length +
      containers.filter((item) => item.status === "waiting").length +
      managed.filter((item) => item.lifecycle === "waiting").length
  );

  $effect(() => {
    const nextActions = { ...taskActions };
    let changed = false;

    for (const [key, action] of Object.entries(taskActions)) {
      if (typeof action === "object") continue;
      const status = managed.find(
        (item) =>
          item.origin.kind === "project" &&
          taskKey(item.origin.project_id, item.origin.task_id) === key
      );
      if (
        (action === "starting" && status) ||
        (action === "stopping" &&
          (!status || status.lifecycle === "exited" || status.lifecycle === "crashed"))
      ) {
        delete nextActions[key];
        changed = true;
      }
    }

    if (changed) taskActions = nextActions;
  });

  onMount(() => {
    if (!isTauri()) return;

    let disposed = false;
    let stopListening: (() => void) | undefined;
    let blurIdleTimer: ReturnType<typeof setTimeout> | undefined;
    let safetyIdleTimer: ReturnType<typeof setTimeout> | undefined;

    const clearBlurIdle = () => {
      if (blurIdleTimer) clearTimeout(blurIdleTimer);
      blurIdleTimer = undefined;
    };

    const clearSafetyIdle = () => {
      if (safetyIdleTimer) clearTimeout(safetyIdleTimer);
      safetyIdleTimer = undefined;
    };

    const setIdle = () => {
      clearBlurIdle();
      clearSafetyIdle();
      void commands.setIdle();
    };

    const armSafetyIdle = () => {
      clearSafetyIdle();
      safetyIdleTimer = setTimeout(setIdle, ACTIVE_SAFETY_IDLE_MS);
    };

    const setActive = () => {
      clearBlurIdle();
      void commands.setActive();
      armSafetyIdle();
    };

    const scheduleIdle = () => {
      clearBlurIdle();
      blurIdleTimer = setTimeout(setIdle, BLUR_IDLE_DELAY_MS);
    };

    const handleVisibilityChange = () => {
      if (document.visibilityState === "visible" && document.hasFocus()) {
        setActive();
      } else {
        scheduleIdle();
      }
    };

    void events.snapshot.listen((event) => {
      snapshot = event.payload;
    }).then((unlisten) => {
      if (disposed) {
        unlisten();
      } else {
        stopListening = unlisten;
      }
    });
    window.addEventListener("focus", setActive);
    window.addEventListener("blur", scheduleIdle);
    document.addEventListener("visibilitychange", handleVisibilityChange);
    setActive();
    void refreshProjects();

    return () => {
      disposed = true;
      window.removeEventListener("focus", setActive);
      window.removeEventListener("blur", scheduleIdle);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      stopListening?.();
      setIdle();
    };
  });

  async function killPortProcess(port: PortRowView) {
    if (!isTauri() || port.pid === 0 || portActionStates[port.key] === "killing") return;

    portActionStates = { ...portActionStates, [port.key]: "killing" };
    const result = await commands.killProcessTree({
      pid: port.pid,
      executable: port.executable,
      start_time: port.startTime,
      expected_port: port.port
    });
    if (result.status === "ok") {
      const { [port.key]: _removed, ...rest } = portActionStates;
      portActionStates = rest;
    } else {
      const actionState =
        result.error.kind === "needs_elevated_privileges" ? "needs_privilege" : "failed";
      portActionStates = { ...portActionStates, [port.key]: actionState };
    }
  }

  async function refreshProjects() {
    if (!isTauri()) return;
    projects = await commands.loadProjects();
  }

  async function addProjectFromFolder() {
    if (!isTauri()) return;
    const result = await commands.pickFolder();
    if (result.status === "error" || result.data === null) return;

    const folder = result.data;
    const tasks = await commands.suggestTasks(folder);
    const name = folder.split("/").filter(Boolean).pop() ?? folder;
    const saveResult = await commands.saveProject({ id: "", name, folder, tasks });
    if (saveResult.status === "ok") {
      projects = saveResult.data;
    }
  }

  async function startTask(projectId: string, taskId: string) {
    if (!isTauri()) return;
    const key = taskKey(projectId, taskId);
    if (taskActions[key] === "starting") return;

    taskActions = { ...taskActions, [key]: "starting" };
    const result = await commands.startTask(projectId, taskId);
    if (result.status === "error") {
      taskActions = { ...taskActions, [key]: { kind: "failed", message: result.error } };
    }
  }

  async function stopTask(runId: string, projectId: string, taskId: string) {
    if (!isTauri()) return;
    const key = taskKey(projectId, taskId);
    if (taskActions[key] === "stopping") return;

    taskActions = { ...taskActions, [key]: "stopping" };
    const result = await commands.stopTask(runId);
    if (result.status === "error") {
      taskActions = { ...taskActions, [key]: { kind: "failed", message: result.error } };
    }
  }

  async function startQuickRun(command: string, cwd: string): Promise<string | undefined> {
    if (!isTauri()) return "Quick-run is available in the app";
    const result = await commands.startQuickRun(command, cwd);
    if (result.status === "error") return result.error;
    return undefined;
  }

  async function stopQuickRun(runId: string): Promise<string | undefined> {
    if (!isTauri()) return "Quick-run is available in the app";
    const result = await commands.stopTask(runId);
    if (result.status === "error") return result.error;
    return undefined;
  }

  async function saveQuickRun(runId: string): Promise<string | undefined> {
    if (!isTauri()) return "Quick-run is available in the app";
    const result = await commands.saveQuickRunAsProject(runId);
    if (result.status === "error") return result.error;
    projects = result.data;
    return undefined;
  }

  async function removeProject(project: Project) {
    if (!isTauri()) return;
    const confirmed = window.confirm(`Remove “${project.name}” from Portus?\n\nThis only unsaves the project here. It does not delete files or stop running processes.`);
    if (!confirmed) return;

    const result = await commands.removeProject(project.id);
    if (result.status === "ok") {
      projects = result.data;
    }
  }

  function focusQuickRun() {
    settingsOpen = false;
    quickRunComponent?.focusCommand();
  }

  function toggleSettings() {
    settingsOpen = !settingsOpen;
  }

  function taskKey(projectId: string, taskId: string): string {
    return `${projectId}:${taskId}`;
  }

  async function saveAsProject(port: PortRowView) {
    if (!isTauri() || !port.cwd) return;
    const candidateResult = await commands.saveAsCandidates(port.pid);
    const candidates = candidateResult.status === "ok" ? candidateResult.data : [];
    const chosen = candidates.find((candidate) => !candidate.is_shell) ?? candidates[0];
    const command = window.prompt("Command for this task:", chosen?.command ?? "");
    if (!command) return;

    const name = port.cwd.split("/").filter(Boolean).pop() ?? port.cwd;
    const result = await commands.saveProject({
      id: "",
      name,
      folder: port.cwd,
      tasks: [{ id: "saved", name: "saved", command }]
    });
    if (result.status === "ok") {
      projects = result.data;
    }
  }
</script>

<main class="popover glass-regular">
  <header class="glance glass-chrome">
    <div class="brand">
      <Anchor class="brand-mark" size={18} strokeWidth={1.75} aria-hidden="true" />
      <p class="summary">
        <span class="running">{runningCount} running</span>
        <span class="sep" aria-hidden="true">·</span>
        <span class="neutral">{waitingCount} waiting?</span>
      </p>
    </div>
    <div class="glance-actions">
      <IconButton label="Add folder" title="Add folder" onclick={addProjectFromFolder}>
        <Plus size={15} strokeWidth={1.9} aria-hidden="true" />
      </IconButton>
      <IconButton
        label={settingsOpen ? "Close settings" : "Settings"}
        title={settingsOpen ? "Close settings" : "Settings"}
        active={settingsOpen}
        onclick={toggleSettings}
      >
        <Settings size={15} strokeWidth={1.8} aria-hidden="true" />
      </IconButton>
    </div>
  </header>

  <div class="scroll-body">
    {#if settingsOpen}
      <SettingsPanel />
    {:else}
      {#if isEmpty}
        <section class="empty-state" aria-labelledby="empty-heading">
          <div class="empty-icon" aria-hidden="true">
            <Anchor size={18} strokeWidth={1.75} />
          </div>
          <div class="empty-copy">
            <h2 id="empty-heading">Nothing running</h2>
            <p>Open a folder or run a one-off command.</p>
          </div>
          <div class="empty-actions">
            <button class="empty-action primary" type="button" onclick={addProjectFromFolder}>
              <FolderOpen size={13} strokeWidth={1.9} aria-hidden="true" />
              <span>Open folder</span>
            </button>
            <button class="empty-action" type="button" onclick={focusQuickRun}>
              <Play size={12} strokeWidth={2.2} fill="currentColor" aria-hidden="true" />
              <span>Quick-run</span>
            </button>
          </div>
        </section>
      {/if}
      <QuickRun
        bind:this={quickRunComponent}
        {projects}
        {managed}
        onRun={startQuickRun}
        onStop={stopQuickRun}
        onSave={saveQuickRun}
      />
      {#if !isEmpty}
        <ProjectList
          {projects}
          {managed}
          {taskActions}
          onStart={startTask}
          onStop={stopTask}
          onRemove={removeProject}
        />
        <PortList
          {ports}
          {projects}
          {managed}
          actionStates={portActionStates}
          onKill={killPortProcess}
          onSaveAs={saveAsProject}
        />
        <DockerList {containers} />
      {/if}
    {/if}
  </div>

  <footer class="footer glass-chrome">
    <kbd class="kbd">⌥⌘P</kbd>
  </footer>
</main>

<style>
  .popover {
    display: flex;
    width: 380px;
    height: 520px;
    flex-direction: column;
    overflow: hidden;
    isolation: isolate;
    border: 1px solid var(--glass-border);
    border-radius: var(--glass-radius);
    color: var(--text-primary);
    background: var(--app-bg);
    box-shadow: var(--glass-shadow);
  }

  .glance {
    position: relative;
    z-index: 3;
    display: flex;
    height: 48px;
    flex: 0 0 48px;
    align-items: center;
    justify-content: space-between;
    padding: 0 var(--row-pad-x);
    border-bottom: 1px solid var(--hairline);
  }

  .brand {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 9px;
  }

  :global(.brand-mark) {
    flex-shrink: 0;
    color: var(--text-primary);
    opacity: 0.5;
  }

  .summary {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 13px;
    font-weight: 600;
    line-height: 1;
    white-space: nowrap;
  }

  .running {
    color: var(--running);
  }

  .summary .sep {
    color: var(--text-muted);
  }

  .summary .neutral {
    color: var(--text-primary);
  }

  .glance-actions {
    display: flex;
    flex: 0 0 auto;
    align-items: center;
    gap: 4px;
  }

  .scroll-body {
    flex: 1 1 0;
    overflow-x: hidden;
    overflow-y: auto;
    overscroll-behavior: contain;
  }

  .scroll-body::-webkit-scrollbar {
    width: 5px;
  }

  .scroll-body::-webkit-scrollbar-track {
    background: transparent;
  }

  .scroll-body::-webkit-scrollbar-thumb {
    border-radius: 3px;
    background: rgb(120 120 130 / 35%);
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 28px 24px 24px;
    border-bottom: 1px solid var(--hairline);
    text-align: center;
  }

  .empty-icon {
    display: grid;
    width: 32px;
    height: 32px;
    place-items: center;
    border: 1px solid var(--glass-border);
    border-radius: 8px;
    color: var(--text-muted);
    background: var(--surface);
    backdrop-filter: var(--glass-blur-chrome);
    -webkit-backdrop-filter: var(--glass-blur-chrome);
    box-shadow: inset 0 1px 0 var(--glass-specular);
  }

  .empty-copy {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .empty-copy h2,
  .empty-copy p {
    margin: 0;
  }

  .empty-copy h2 {
    color: var(--text-primary);
    font-size: 13px;
    font-weight: 600;
    line-height: 1.2;
  }

  .empty-copy p {
    color: var(--text-secondary);
    font-size: 11px;
    line-height: 1.35;
  }

  .empty-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 2px;
  }

  .empty-action {
    display: flex;
    height: 28px;
    align-items: center;
    gap: 5px;
    padding: 0 9px;
    border: 1px solid var(--glass-border);
    border-radius: 8px;
    color: var(--text-muted);
    background: var(--surface);
    backdrop-filter: var(--glass-blur-chrome);
    -webkit-backdrop-filter: var(--glass-blur-chrome);
    box-shadow: inset 0 1px 0 var(--glass-specular);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition:
      color var(--motion-fast),
      background var(--motion-fast),
      border-color var(--motion-fast);
  }

  .empty-action.primary,
  .empty-action:hover {
    border-color: var(--hairline-strong);
    color: var(--text-primary);
  }

  .empty-action:hover {
    background: var(--surface-pressed);
  }

  .footer {
    display: flex;
    height: 28px;
    flex: 0 0 28px;
    align-items: center;
    justify-content: flex-end;
    padding: 0 var(--row-pad-x);
    border-top: 1px solid var(--hairline);
  }

  .kbd {
    padding: 2px 6px;
    border: 1px solid var(--glass-border);
    border-radius: 6px;
    background: var(--surface);
    backdrop-filter: var(--glass-blur-chrome);
    -webkit-backdrop-filter: var(--glass-blur-chrome);
    box-shadow: inset 0 1px 0 var(--glass-specular);
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: 10px;
    user-select: none;
  }

  @media (prefers-reduced-motion: reduce) {
    .empty-action {
      transition: none;
    }
  }
</style>
