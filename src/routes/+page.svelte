<script lang="ts">
  import { isTauri } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { Anchor, Plus, Settings } from "@lucide/svelte";
  import { onMount } from "svelte";

  import { commands, events, type Project, type Snapshot } from "$lib/bindings";
  import DockerList from "$lib/components/DockerList.svelte";
  import PortList, { type PortActionState } from "$lib/components/PortList.svelte";
  import ProjectList, { type TaskActionState } from "$lib/components/ProjectList.svelte";
  import QuickRun from "$lib/components/QuickRun.svelte";
  import { snapshotFixture } from "$lib/fixtures";
  import { containersToDockerRows, snapshotToPortRows, type PortRowView } from "$lib/snapshot-adapter";

  const BLUR_IDLE_DELAY_MS = 200;
  const ACTIVE_SAFETY_IDLE_MS = 30_000;

  let snapshot: Snapshot = $state(snapshotFixture);
  let projects: Project[] = $state([]);
  let portActionStates: Record<string, PortActionState> = $state({});
  let taskActions: Record<string, TaskActionState> = $state({});
  const managed = $derived(snapshot.managed);
  const ports = $derived(snapshotToPortRows(snapshot));
  const containers = $derived(containersToDockerRows(snapshot.docker.data.containers));
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
    const folder = await open({ directory: true, multiple: false });
    if (typeof folder !== "string") return;

    const tasks = await commands.suggestTasks(folder);
    const name = folder.split("/").filter(Boolean).pop() ?? folder;
    const result = await commands.saveProject({ id: "", name, folder, tasks });
    if (result.status === "ok") {
      projects = result.data;
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

<main class="popover">
  <header class="glance">
    <div class="brand">
      <Anchor class="brand-mark" size={18} strokeWidth={1.75} aria-hidden="true" />
      <div class="identity">
        <p class="product">Portus</p>
        <p class="summary">
          <span class="running">{runningCount} running</span>
          <span class="sep" aria-hidden="true">·</span>
          <span class="neutral">{waitingCount} waiting?</span>
        </p>
      </div>
    </div>
    <div class="glance-actions">
      <button class="icon-button" type="button" aria-label="Add folder" title="Add folder" onclick={addProjectFromFolder}>
        <Plus size={15} strokeWidth={1.9} aria-hidden="true" />
      </button>
      <button class="icon-button muted" type="button" aria-label="Settings (unavailable)" title="Settings unavailable" disabled>
        <Settings size={15} strokeWidth={1.8} aria-hidden="true" />
      </button>
    </div>
  </header>

  <div class="scroll-body">
    <QuickRun
      {projects}
      {managed}
      onRun={startQuickRun}
      onStop={stopQuickRun}
      onSave={saveQuickRun}
    />
    <ProjectList {projects} {managed} {taskActions} onStart={startTask} onStop={stopTask} />
    <PortList {ports} actionStates={portActionStates} onKill={killPortProcess} onSaveAs={saveAsProject} />
    <DockerList {containers} />
  </div>

  <footer class="footer">
    <div class="footer-left">
      <button class="footer-btn" type="button" title="Add folder" onclick={addProjectFromFolder}>
        <Plus size={12} strokeWidth={2} aria-hidden="true" />
        <span>Add folder</span>
      </button>
      <button class="footer-btn" type="button" title="Settings (unavailable)" disabled>
        <Settings size={12} strokeWidth={1.8} aria-hidden="true" />
        <span>Settings</span>
      </button>
    </div>
    <kbd class="kbd">⌥⌘P</kbd>
  </footer>
</main>

<style>
  @font-face {
    font-family: "Geist";
    src: url("/fonts/Geist-Variable.woff2") format("woff2");
    font-style: normal;
    font-weight: 100 900;
    font-display: swap;
  }

  @font-face {
    font-family: "Geist Mono";
    src: url("/fonts/GeistMono-Variable.woff2") format("woff2");
    font-style: normal;
    font-weight: 100 900;
    font-display: swap;
  }

  :global(html),
  :global(body) {
    margin: 0;
    width: 100%;
    min-width: 380px;
    height: 100%;
    min-height: 520px;
    overflow: hidden;
    color: var(--text-primary);
    background: transparent;
    font-family: var(--font-ui);
    font-synthesis: none;
    text-rendering: optimizeLegibility;
    -webkit-font-smoothing: antialiased;
  }

  :global(:root) {
    --font-ui: "Geist", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    --font-mono: "Geist Mono", "SFMono-Regular", Consolas, monospace;
    --app-bg: rgb(251 251 253 / 72%);
    --surface: rgb(255 255 255 / 70%);
    --surface-hi: rgb(255 255 255 / 92%);
    --hairline: #e7e7ea;
    --text-primary: #1a1a1e;
    --text-secondary: #5c5c63;
    --text-muted: #8a8a92;
    --accent: #0f9d63;
    --running: #12a266;
    --waiting: #b07a2e;
    --stopped: #a3a3ab;
    --crashed: #cf5a4c;
    --popover-shadow: 0 0 0 0.5px rgb(0 0 0 / 4%) inset, 0 16px 44px rgb(0 0 0 / 14%);
  }

  :global(*) {
    box-sizing: border-box;
  }

  :global(button) {
    font: inherit;
  }

  .popover {
    display: flex;
    width: 380px;
    height: 520px;
    flex-direction: column;
    overflow: hidden;
    border: 1px solid var(--hairline);
    border-radius: 12px;
    color: var(--text-primary);
    background: var(--app-bg);
    box-shadow: var(--popover-shadow);
  }

  .glance {
    position: relative;
    z-index: 3;
    display: flex;
    height: 56px;
    flex: 0 0 56px;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px;
    border-bottom: 1px solid var(--hairline);
    background: var(--surface);
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

  .product,
  .summary {
    margin: 0;
  }

  .identity {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: 2px;
  }

  .product {
    font-size: 11px;
    font-weight: 600;
    line-height: 1;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .summary {
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

  .icon-button {
    display: grid;
    width: 28px;
    height: 28px;
    flex: 0 0 28px;
    place-items: center;
    padding: 0;
    border: 1px solid transparent;
    border-radius: 6px;
    color: var(--text-muted);
    background: transparent;
    cursor: pointer;
    opacity: 0.85;
    transition:
      opacity 100ms ease,
      background 100ms ease;
  }

  .glance-actions {
    display: flex;
    flex: 0 0 auto;
    align-items: center;
    gap: 4px;
  }

  .icon-button.muted {
    cursor: default;
    opacity: 0.5;
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

  .icon-button:hover:not(:disabled) {
    opacity: 1;
    background: rgb(127 127 127 / 12%);
  }

  .footer {
    display: flex;
    height: 36px;
    flex: 0 0 36px;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px;
    border-top: 1px solid var(--hairline);
    background: var(--surface);
  }

  .footer-left {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .footer-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 0;
    border: none;
    background: transparent;
    color: var(--text-muted);
    font-family: var(--font-ui);
    font-size: 12px;
    cursor: default;
  }

  .footer-btn:disabled {
    opacity: 0.65;
  }

  .kbd {
    padding: 2px 6px;
    border: 1px solid var(--hairline);
    border-radius: 4px;
    background: rgb(127 127 127 / 10%);
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: 10px;
    user-select: none;
  }

  @media (prefers-color-scheme: dark) {
    :global(:root) {
      --app-bg: rgb(22 22 24 / 72%);
      --surface: rgb(28 28 31 / 70%);
      --surface-hi: #212125;
      --hairline: #2a2a2e;
      --text-primary: #ededee;
      --text-secondary: #8c8c93;
      --text-muted: #5c5c63;
      --accent: #45ce93;
      --running: #45ce93;
      --waiting: #d2a24c;
      --stopped: #56565c;
      --crashed: #d97066;
      --popover-shadow: 0 0 0 0.5px rgb(255 255 255 / 3.5%) inset,
        0 16px 44px rgb(0 0 0 / 55%);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .icon-button {
      transition: none;
    }
  }
</style>
