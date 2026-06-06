// TODO(T6): replace these hand-written types with tauri-specta-generated types from the Rust snapshot model.
export type ServiceStatus = "running" | "waiting" | "stopped" | "crashed";

export type PortSource = "system" | "from IDE" | "from Terminal" | "orphan?";

export interface PortItem {
  port: number | null;
  process: string;
  source: PortSource;
  status: ServiceStatus;
  pid: number;
  cpuPercent: number;
  memoryMb: number;
  cwd: string;
}

export interface DockerItem {
  name: string;
  image: string;
  status: ServiceStatus;
  detail: string;
  ports: number[];
}
