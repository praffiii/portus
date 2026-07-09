import type { DockerContainer, ManagedStatus, ProcessInfo, Project, Snapshot } from "$lib/bindings";

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

/** Default Ports filter: project-matched first. Legacy `relevant` maps to `projects`. */
export type PortFilterMode = "projects" | "all";

export type PortSourceBucketId = "ide" | "terminal" | "system" | "other";

export type PortProjectGroup = {
  projectId: string;
  projectName: string;
  folder: string;
  ports: PortRowView[];
};

export type PortSourceBucket = {
  id: PortSourceBucketId;
  label: string;
  ports: PortRowView[];
};

export type PortsProjectsView = {
  projectGroups: PortProjectGroup[];
  buckets: PortSourceBucket[];
};

const sourcePriority: Record<PortSource, number> = {
  "from IDE": 0,
  "from Terminal": 1,
  "orphan?": 2,
  system: 3
};

const bucketOrder: PortSourceBucketId[] = ["ide", "terminal", "system", "other"];

const bucketMeta: Record<PortSourceBucketId, string> = {
  ide: "IDE",
  terminal: "Terminal",
  system: "System",
  other: "Other"
};

export function sortPortRows(ports: PortRowView[]): PortRowView[] {
  return [...ports].sort((left, right) => {
    const bySource = sourcePriority[left.source] - sourcePriority[right.source];
    if (bySource !== 0) return bySource;
    return left.port - right.port;
  });
}

/** @deprecated Prefer organizePortsByProject — kept for any leftover Relevant callers. */
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

export type PortProcessGroup = {
  key: string;
  pid: number;
  process: string;
  source: PortSource;
  status: ServiceStatus;
  executable: string | null;
  startTime: number;
  cpuPercent: number;
  memoryMb: number;
  cwd: string;
  ports: PortRowView[];
};

/** Collapse listeners that share a PID into one process group. PID 0 stays one-per-port. */
export function groupPortsByPid(ports: PortRowView[]): PortProcessGroup[] {
  const buckets = new Map<string, PortRowView[]>();

  for (const port of sortPortRows(ports)) {
    const key = port.pid === 0 ? `orphan-port:${port.key}` : `pid:${port.pid}`;
    const bucket = buckets.get(key);
    if (bucket) {
      bucket.push(port);
    } else {
      buckets.set(key, [port]);
    }
  }

  return [...buckets.entries()].map(([key, members]) => {
    const portsSorted = [...members].sort((left, right) => left.port - right.port);
    const primary = portsSorted[0];

    return {
      key,
      pid: primary.pid,
      process: primary.process,
      source: primary.source,
      status: primary.status,
      executable: primary.executable,
      startTime: primary.startTime,
      cpuPercent: primary.cpuPercent,
      memoryMb: primary.memoryMb,
      cwd: primary.cwd,
      ports: portsSorted
    };
  });
}

/**
 * Project-first Ports view: match managed/path ports into project groups;
 * leftover listeners go into collapsed source buckets.
 */
export function organizePortsByProject(
  ports: PortRowView[],
  projects: Project[],
  managed: ManagedStatus[] = []
): PortsProjectsView {
  const managedProjectByPid = new Map<number, string>();
  for (const item of managed) {
    if (item.origin.kind !== "project") continue;
    if (item.pid > 0) managedProjectByPid.set(item.pid, item.origin.project_id);
  }

  const projectsById = new Map(projects.map((project) => [project.id, project]));
  const matchedKeys = new Set<string>();
  const portsByProject = new Map<string, PortRowView[]>();

  for (const port of ports) {
    const projectId = matchPortToProject(port, projects, managedProjectByPid);
    if (!projectId) continue;
    matchedKeys.add(port.key);
    const bucket = portsByProject.get(projectId);
    if (bucket) bucket.push(port);
    else portsByProject.set(projectId, [port]);
  }

  const projectGroups: PortProjectGroup[] = [...portsByProject.entries()]
    .map(([projectId, members]) => {
      const project = projectsById.get(projectId);
      return {
        projectId,
        projectName: project?.name ?? projectId,
        folder: project?.folder ?? "",
        ports: sortPortRows(members)
      };
    })
    .sort((left, right) => left.projectName.localeCompare(right.projectName));

  const unmatched = ports.filter((port) => !matchedKeys.has(port.key));
  const bucketPorts = new Map<PortSourceBucketId, PortRowView[]>();

  for (const port of unmatched) {
    const id = sourceBucketId(port.source);
    const bucket = bucketPorts.get(id);
    if (bucket) bucket.push(port);
    else bucketPorts.set(id, [port]);
  }

  const buckets: PortSourceBucket[] = bucketOrder
    .filter((id) => (bucketPorts.get(id)?.length ?? 0) > 0)
    .map((id) => ({
      id,
      label: bucketMeta[id],
      ports: sortPortRows(bucketPorts.get(id) ?? [])
    }));

  return { projectGroups, buckets };
}

export function matchPortToProject(
  port: PortRowView,
  projects: Project[],
  managedProjectByPid: Map<number, string>
): string | null {
  if (port.pid > 0) {
    const managedId = managedProjectByPid.get(port.pid);
    if (managedId && projects.some((project) => project.id === managedId)) {
      return managedId;
    }
  }

  if (isIdeChromeProcess(port)) return null;

  let best: { id: string; folderLen: number } | null = null;
  for (const project of projects) {
    if (!pathIsInsideFolder(port.cwd, project.folder)) continue;
    const folderLen = normalizePath(project.folder).length;
    if (!best || folderLen > best.folderLen) {
      best = { id: project.id, folderLen };
    }
  }
  return best?.id ?? null;
}

export function sourceBucketId(source: PortSource): PortSourceBucketId {
  if (source === "from IDE") return "ide";
  if (source === "from Terminal") return "terminal";
  if (source === "system") return "system";
  return "other";
}

export function normalizePath(path: string): string {
  const trimmed = path.trim();
  if (!trimmed || trimmed === "Unknown") return "";
  return trimmed.replace(/\/+$/, "").toLowerCase();
}

export function pathIsInsideFolder(path: string, folder: string): boolean {
  const cwd = normalizePath(path);
  const root = normalizePath(folder);
  if (!cwd || !root) return false;
  return cwd === root || cwd.startsWith(`${root}/`);
}

function isIdeChromeProcess(port: PortRowView): boolean {
  if (port.source !== "from IDE") return false;
  return /helper|language server|tsserver|eslint|copilot/i.test(port.process);
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
