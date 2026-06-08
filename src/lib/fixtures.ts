import type { PortRow, Snapshot } from "$lib/bindings";

export const snapshotFixture: Snapshot = {
  ports: {
    error: null,
    data: [
      portRow(3000, 18420, "web", "/Applications/Visual Studio Code.app/web"),
      portRow(8080, 18704, "api", "/Applications/Terminal.app/api"),
      portRow(5432, 612, "postgres", "/opt/homebrew/bin/postgres")
    ]
  },
  processes: {
    error: null,
    data: [
      processFixture(18420, "web", 2.4, 148, "~/code/portus-web", "/Applications/Visual Studio Code.app/web"),
      processFixture(18704, "api", 0.8, 92, "~/code/portus-api", "/Applications/Terminal.app/api"),
      processFixture(612, "postgres", 0.1, 76, "/opt/homebrew/var/postgresql@16", "/opt/homebrew/bin/postgres")
    ]
  },
  docker: {
    error: null,
    data: {
      status: "detected",
      containers: [
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
      ]
    }
  },
  managed: []
};

function portRow(port: number, pid: number, name: string, path: string): PortRow {
  return {
    protocol: "Tcp",
    port,
    scope: "Loopback",
    specific_addr: null,
    families: ["V4"],
    owners: [{ pid, name, path }],
    key: `tcp|loopback|${port}|${pid}`
  };
}

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
