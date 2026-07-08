import type { DockerContainer, ProcessInfo, Snapshot } from "$lib/bindings";

const bytesPerMegabyte = 1024 * 1024;

export const serviceStatuses = ["running", "waiting", "stopped", "crashed"] as const;
export type ServiceStatus = (typeof serviceStatuses)[number];

export function snapshotToPortRows(snapshot: Snapshot) {
  const processesByPid = new Map(
    snapshot.processes.data.map((process) => [process.pid, process])
  );

  return snapshot.ports.data.map((row) => {
    const owner = row.owners[0] ?? null;
    const process = owner ? processesByPid.get(owner.pid) : undefined;

    return {
      key: row.key,
      port: row.port,
      process: process?.name ?? owner?.name ?? "unknown owner",
      source: process ? sourceFromProcess(process) : ("orphan?" as const),
      status: "running" as ServiceStatus,
      pid: owner?.pid ?? 0,
      executable: process?.executable ?? null,
      startTime: process?.start_time ?? 0,
      cpuPercent: process?.cpu_usage ?? 0,
      memoryMb: process ? Math.round(process.memory_bytes / bytesPerMegabyte) : 0,
      cwd: process?.cwd ?? process?.executable ?? owner?.path ?? "Unknown"
    };
  });
}

export function containersToDockerRows(containers: DockerContainer[]) {
  return containers.map((container) => ({
    id: container.id,
    name: container.names[0]?.replace(/^\//, "") ?? container.id.slice(0, 12),
    image: container.image,
    status: dockerStatus(container.state),
    detail: container.status.replace(/,\s*(?:[\d.]+:)?\d+->\d+\/(?:tcp|udp).*$/, ""),
    ports: publishedPorts(container.status)
  }));
}

export type PortRowView = ReturnType<typeof snapshotToPortRows>[number];
export type DockerRowView = ReturnType<typeof containersToDockerRows>[number];
export type PortSource = PortRowView["source"];

export type PortFilterMode = "relevant" | "all";

const sourcePriority: Record<PortSource, number> = {
  "from IDE": 0,
  "from Terminal": 1,
  "orphan?": 2,
  system: 3
};

export function sortPortRows(ports: PortRowView[]): PortRowView[] {
  return [...ports].sort((left, right) => {
    const bySource = sourcePriority[left.source] - sourcePriority[right.source];
    if (bySource !== 0) return bySource;
    return left.port - right.port;
  });
}

export function partitionPorts(ports: PortRowView[]) {
  const relevant: PortRowView[] = [];
  const system: PortRowView[] = [];

  for (const port of sortPortRows(ports)) {
    if (port.source === "system") {
      system.push(port);
    } else {
      relevant.push(port);
    }
  }

  return { relevant, system };
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
