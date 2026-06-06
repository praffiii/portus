import type { DockerItem, PortItem } from "$lib/types";

export const portFixtures: PortItem[] = [
  {
    port: 3000,
    process: "web",
    source: "from IDE",
    status: "running",
    pid: 18420,
    cpuPercent: 2.4,
    memoryMb: 148,
    cwd: "~/code/portus-web"
  },
  {
    port: 8080,
    process: "api",
    source: "from Terminal",
    status: "running",
    pid: 18704,
    cpuPercent: 0.8,
    memoryMb: 92,
    cwd: "~/code/portus-api"
  },
  {
    port: 5432,
    process: "postgres",
    source: "system",
    status: "running",
    pid: 612,
    cpuPercent: 0.1,
    memoryMb: 76,
    cwd: "/opt/homebrew/var/postgresql@16"
  },
  {
    port: null,
    process: "migration-worker",
    source: "orphan?",
    status: "waiting",
    pid: 19117,
    cpuPercent: 0,
    memoryMb: 44,
    cwd: "~/code/portus-api/workers"
  }
];

export const dockerFixtures: DockerItem[] = [
  {
    name: "portus-redis",
    image: "redis:7-alpine",
    status: "running",
    detail: "Up 2 hours",
    ports: [6379]
  },
  {
    name: "portus-postgres",
    image: "postgres:16-alpine",
    status: "running",
    detail: "Up 2 hours (healthy)",
    ports: [5433]
  },
  {
    name: "docs-preview",
    image: "nginx:1.27-alpine",
    status: "stopped",
    detail: "Exited (0) 3 hours ago",
    ports: [9000]
  }
];
