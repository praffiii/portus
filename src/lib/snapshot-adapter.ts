import type { DockerContainer, PortListener, ProcessInfo, Snapshot } from "$lib/bindings";

const bytesPerMegabyte = 1024 * 1024;

export const serviceStatuses = ["running", "waiting", "stopped", "crashed"] as const;
export type ServiceStatus = (typeof serviceStatuses)[number];

export function snapshotToPortRows(snapshot: Snapshot) {
  const listenersByPid = new Map(
    snapshot.ports.data.map((listener) => [listener.process.pid, listener])
  );
  const processesByPid = new Map(
    snapshot.processes.data.map((process) => [process.pid, process])
  );

  for (const listener of snapshot.ports.data) {
    if (!processesByPid.has(listener.process.pid)) {
      processesByPid.set(listener.process.pid, listenerFallback(listener));
    }
  }

  return [...processesByPid.values()].map((process) => {
    const listener = listenersByPid.get(process.pid);

    return {
      port: listener ? portFromSocket(listener.socket) : null,
      process: process.name,
      source: sourceFromProcess(process),
      status: (listener ? "running" : "waiting") as ServiceStatus,
      pid: process.pid,
      cpuPercent: process.cpu_usage ?? 0,
      memoryMb: Math.round(process.memory_bytes / bytesPerMegabyte),
      cwd: process.cwd ?? process.executable ?? listener?.process.path ?? "Unknown"
    };
  });
}

export function containersToDockerRows(containers: DockerContainer[]) {
  return containers.map((container) => ({
    name: container.names[0]?.replace(/^\//, "") ?? container.id.slice(0, 12),
    image: container.image,
    status: dockerStatus(container.state),
    detail: container.status.replace(/,\s*(?:[\d.]+:)?\d+->\d+\/(?:tcp|udp).*$/, ""),
    ports: publishedPorts(container.status)
  }));
}

export type PortRow = ReturnType<typeof snapshotToPortRows>[number];
export type DockerRow = ReturnType<typeof containersToDockerRows>[number];
export type PortSource = PortRow["source"];

function listenerFallback(listener: PortListener): ProcessInfo {
  return {
    pid: listener.process.pid,
    parent_pid: null,
    name: listener.process.name,
    command: [],
    executable: listener.process.path,
    cwd: null,
    start_time: 0,
    cpu_usage: 0,
    memory_bytes: 0
  };
}

function portFromSocket(socket: string): number | null {
  const port = Number(socket.slice(socket.lastIndexOf(":") + 1));
  return Number.isInteger(port) ? port : null;
}

function sourceFromProcess(process: ProcessInfo) {
  const context = [process.executable, process.cwd, ...process.command]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();

  if (context.includes("terminal") || context.includes("iterm") || context.includes("warp")) {
    return "from Terminal" as const;
  }
  if (context.includes("visual studio code") || context.includes("/code") || context.includes("cursor")) {
    return "from IDE" as const;
  }
  if (context.includes("orphan?")) return "orphan?" as const;
  return process.cwd ? ("system" as const) : ("orphan?" as const);
}

function dockerStatus(state: string): ServiceStatus {
  if (state === "running") return "running";
  if (state === "created" || state === "restarting" || state === "paused") return "waiting";
  if (state === "dead") return "crashed";
  return "stopped";
}

function publishedPorts(status: string): number[] {
  return [...status.matchAll(/(?:^|[\s,])(?:[\d.]+:)?(\d+)->\d+\/(?:tcp|udp)/g)].map(
    (match) => Number(match[1])
  );
}
