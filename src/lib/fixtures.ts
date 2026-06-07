import type { DockerContainer, Snapshot } from "$lib/bindings";

export const snapshotFixture: Snapshot = {
  ports: {
    error: null,
    data: [
      { protocol: "Tcp", socket: "127.0.0.1:3000", process: { pid: 18420, name: "web", path: "/Applications/Visual Studio Code.app/web" } },
      { protocol: "Tcp", socket: "127.0.0.1:8080", process: { pid: 18704, name: "api", path: "/Applications/Terminal.app/api" } },
      { protocol: "Tcp", socket: "127.0.0.1:5432", process: { pid: 612, name: "postgres", path: "/opt/homebrew/bin/postgres" } }
    ]
  },
  processes: {
    error: null,
    data: [
      processFixture(18420, "web", 2.4, 148, "~/code/portus-web", "/Applications/Visual Studio Code.app/web"),
      processFixture(18704, "api", 0.8, 92, "~/code/portus-api", "/Applications/Terminal.app/api"),
      processFixture(612, "postgres", 0.1, 76, "/opt/homebrew/var/postgresql@16", "/opt/homebrew/bin/postgres"),
      {
        ...processFixture(19117, "migration-worker", 0, 44, "~/code/portus-api/workers", null),
        command: ["orphan?"]
      }
    ]
  }
};

export const dockerFixtures: DockerContainer[] = [
  {
    id: "redis",
    names: ["/portus-redis"],
    image: "redis:7-alpine",
    state: "running",
    status: "Up 2 hours, 0.0.0.0:6379->6379/tcp"
  },
  {
    id: "postgres",
    names: ["/portus-postgres"],
    image: "postgres:16-alpine",
    state: "running",
    status: "Up 2 hours (healthy), 0.0.0.0:5433->5432/tcp"
  },
  {
    id: "docs",
    names: ["/docs-preview"],
    image: "nginx:1.27-alpine",
    state: "exited",
    status: "Exited (0) 3 hours ago, 0.0.0.0:9000->80/tcp"
  }
];

function processFixture(
  pid: number,
  name: string,
  cpuUsage: number,
  memoryMb: number,
  cwd: string | null,
  executable: string | null
) {
  return {
    pid,
    parent_pid: null,
    name,
    command: [],
    executable,
    cwd,
    start_time: 0,
    cpu_usage: cpuUsage,
    memory_bytes: memoryMb * 1024 * 1024
  };
}
