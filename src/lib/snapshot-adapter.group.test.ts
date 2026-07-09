import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { ManagedStatus, Project } from "./bindings.ts";
import {
  groupPortsByPid,
  organizePortsByProject,
  pathIsInsideFolder,
  type PortRowView
} from "./snapshot-adapter.ts";

function port(partial: Partial<PortRowView> & Pick<PortRowView, "key" | "port" | "pid" | "process">): PortRowView {
  return {
    source: "from IDE",
    status: "running",
    executable: "/Applications/Cursor.app/Contents/Frameworks/Cursor Helper",
    startTime: 1,
    cpuPercent: 0.2,
    memoryMb: 130,
    cwd: "/",
    ...partial
  };
}

function project(partial: Partial<Project> & Pick<Project, "id" | "name" | "folder">): Project {
  return {
    tasks: [],
    ...partial
  };
}

describe("groupPortsByPid", () => {
  it("keeps a single-port process as a one-member group", () => {
    const groups = groupPortsByPid([
      port({ key: "tcp|3000|1", port: 3000, pid: 100, process: "node", source: "from Terminal" })
    ]);

    assert.equal(groups.length, 1);
    assert.equal(groups[0].key, "pid:100");
    assert.deepEqual(
      groups[0].ports.map((row) => row.port),
      [3000]
    );
  });

  it("collapses multiple ports owned by the same pid", () => {
    const groups = groupPortsByPid([
      port({ key: "a", port: 57922, pid: 23437, process: "Cursor Helper" }),
      port({ key: "b", port: 57921, pid: 23437, process: "Cursor Helper" }),
      port({ key: "c", port: 57923, pid: 23437, process: "Cursor Helper" })
    ]);

    assert.equal(groups.length, 1);
    assert.equal(groups[0].key, "pid:23437");
    assert.equal(groups[0].process, "Cursor Helper");
    assert.deepEqual(
      groups[0].ports.map((row) => row.port),
      [57921, 57922, 57923]
    );
  });

  it("does not merge ports with distinct pids", () => {
    const groups = groupPortsByPid([
      port({ key: "a", port: 3000, pid: 10, process: "node", source: "from Terminal" }),
      port({ key: "b", port: 3001, pid: 11, process: "node", source: "from Terminal" })
    ]);

    assert.equal(groups.length, 2);
    assert.deepEqual(
      groups.map((group) => group.pid),
      [10, 11]
    );
  });

  it("keeps pid 0 listeners unmerged", () => {
    const groups = groupPortsByPid([
      port({ key: "orphan-a", port: 80, pid: 0, process: "unknown", source: "orphan?" }),
      port({ key: "orphan-b", port: 443, pid: 0, process: "unknown", source: "orphan?" })
    ]);

    assert.equal(groups.length, 2);
    assert.ok(groups.every((group) => group.key.startsWith("orphan-port:")));
  });
});

describe("pathIsInsideFolder", () => {
  it("matches cwd inside project folder", () => {
    assert.equal(
      pathIsInsideFolder("/Users/me/Developments/listify/apps/web", "/Users/me/Developments/listify"),
      true
    );
  });

  it("rejects sibling folders", () => {
    assert.equal(
      pathIsInsideFolder("/Users/me/Developments/listify-old", "/Users/me/Developments/listify"),
      false
    );
  });
});

describe("organizePortsByProject", () => {
  const listify = project({
    id: "p-listify",
    name: "listify",
    folder: "/Users/me/Developments/listify"
  });
  const admin = project({
    id: "p-admin",
    name: "admin",
    folder: "/Users/me/Developments/admin"
  });

  it("groups path-matched ports under their project and buckets the rest", () => {
    const view = organizePortsByProject(
      [
        port({
          key: "app",
          port: 3000,
          pid: 10,
          process: "node",
          source: "from Terminal",
          cwd: "/Users/me/Developments/listify"
        }),
        port({
          key: "helper",
          port: 57921,
          pid: 99,
          process: "Cursor Helper (Plugin)",
          source: "from IDE",
          cwd: "/Users/me/Developments/listify"
        }),
        port({
          key: "sys",
          port: 53,
          pid: 1,
          process: "mDNSResponder",
          source: "system",
          cwd: "/"
        })
      ],
      [listify, admin]
    );

    assert.equal(view.projectGroups.length, 1);
    assert.equal(view.projectGroups[0].projectName, "listify");
    assert.deepEqual(
      view.projectGroups[0].ports.map((row) => row.port),
      [3000]
    );
    assert.deepEqual(
      view.buckets.map((bucket) => [bucket.id, bucket.ports.length]),
      [
        ["ide", 1],
        ["system", 1]
      ]
    );
  });

  it("prefers managed project origin over path", () => {
    const managed: ManagedStatus[] = [
      {
        run_id: "run-1",
        origin: { kind: "project", project_id: "p-admin", task_id: "dev" },
        launch_spec: { command: "pnpm dev", cwd: "/Users/me/Developments/admin" },
        pid: 42,
        lifecycle: "running",
        recent_output: []
      }
    ];

    const view = organizePortsByProject(
      [
        port({
          key: "managed",
          port: 5173,
          pid: 42,
          process: "node",
          source: "from Terminal",
          cwd: "/Users/me/Developments/listify"
        })
      ],
      [listify, admin],
      managed
    );

    assert.equal(view.projectGroups.length, 1);
    assert.equal(view.projectGroups[0].projectId, "p-admin");
  });

  it("picks the longest matching project folder", () => {
    const nested = project({
      id: "p-web",
      name: "web",
      folder: "/Users/me/Developments/listify/apps/web"
    });

    const view = organizePortsByProject(
      [
        port({
          key: "nested",
          port: 3000,
          pid: 10,
          process: "node",
          source: "from Terminal",
          cwd: "/Users/me/Developments/listify/apps/web"
        })
      ],
      [listify, nested]
    );

    assert.equal(view.projectGroups[0].projectId, "p-web");
  });
});
