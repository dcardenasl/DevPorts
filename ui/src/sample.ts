// Invented data (projects, ports, times) for tests and for the panel in a
// browser without the application. Never shipped as real content.

import type { InstanceView, LogEntry, Snapshot } from "./types";

const base: InstanceView = {
  ref: "",
  project: null,
  service: null,
  role: "principal",
  state: "healthy",
  kind: "app",
  port: null,
  ports: [],
  url: null,
  pid: null,
  target_identity: null,
  container: null,
  branch: null,
  worktree: null,
  cwd: null,
  started_at: null,
  expires_at: null,
  reason: null,
  registered: true,
  launcher_chain: [],
  duplicate_of: null,
  stale: null,
  shared: false,
  compose: null,
  argv: [],
  image: null,
  ignored: false,
};

const iso = (nowMs: number, offsetSeconds: number): string =>
  new Date(nowMs + offsetSeconds * 1000).toISOString();

export function sampleSnapshot(nowMs: number = Date.now()): Snapshot {
  const api: InstanceView = {
    ...base,
    ref: "a1b2",
    project: "tienda-demo",
    service: "api",
    port: 8080,
    ports: [8080],
    url: "http://localhost:8080",
    pid: 4121,
    target_identity: "api-process-target",
    branch: "main",
    cwd: "/work/tienda-demo/tienda-api",
    started_at: iso(nowMs, -3 * 3600),
    argv: ["php", "spark", "serve", "--port", "8080"],
  };
  const temporal: InstanceView = {
    ...base,
    ref: "c3d4",
    project: "tienda-demo",
    service: "api",
    role: "temporal",
    port: 52814,
    ports: [52814],
    url: "http://localhost:52814",
    pid: 5530,
    target_identity: "temporal-process-target",
    branch: "main",
    cwd: "/work/tienda-demo/tienda-api",
    started_at: iso(nowMs, -29 * 60),
    expires_at: iso(nowMs, 49),
    reason: "QA login",
    launcher_chain: [
      { pid: 4000, name: "zsh", label: null },
      { pid: 3999, name: "claude", label: "Claude Code" },
    ],
  };
  const duplicate = (ref: string, port: number, ageMinutes: number, pid: number): InstanceView => ({
    ...base,
    ref,
    project: "tienda-demo",
    service: "api",
    role: "unregistered",
    port,
    ports: [port],
    url: `http://localhost:${port}`,
    pid,
    target_identity: `duplicate-process-${pid}`,
    registered: false,
    duplicate_of: "a1b2",
    cwd: "/work/tienda-demo/tienda-api",
    started_at: iso(nowMs, -ageMinutes * 60),
    argv: ["php", "spark", "serve", "--port", String(port)],
  });
  const admin: InstanceView = {
    ...base,
    ref: "k1l2",
    project: "tienda-demo",
    service: "admin",
    port: 8081,
    ports: [8081],
    url: "http://localhost:8081",
    pid: 4300,
    target_identity: "admin-process-target",
    branch: "develop",
    cwd: "/work/tienda-demo/tienda-admin",
    started_at: iso(nowMs, -2 * 3600),
  };
  const mysql: InstanceView = {
    ...base,
    ref: "m9n0",
    service: "mysql",
    kind: "data",
    port: 3306,
    ports: [3306],
    url: "http://localhost:3306",
    container: "mysql",
    target_identity: "mysql-container-target",
    shared: true,
    image: "mysql:8.0",
    compose: "docker-services",
    started_at: iso(nowMs, -26 * 3600),
  };
  const loose: InstanceView = {
    ...base,
    ref: "z9y8",
    role: "unregistered",
    port: 7000,
    ports: [7000],
    url: "http://localhost:7000",
    pid: 6012,
    target_identity: "loose-process-target",
    registered: false,
    cwd: "/tmp/scratch",
    started_at: iso(nowMs, -41 * 60),
    argv: ["python3", "-m", "http.server", "7000"],
  };
  const ignored: InstanceView = {
    ...loose,
    ref: "i0j1",
    port: 7001,
    ports: [7001],
    pid: 6013,
    target_identity: "ignored-process-target",
    ignored: true,
    argv: ["node", "scratch.js"],
  };
  const kickstartDown: InstanceView = {
    ...base,
    ref: "-",
    project: "ci4-kickstart",
    service: "web",
    state: "down",
    port: 8090,
    url: "http://localhost:8090",
    registered: false,
    cwd: "/work/ci4-kickstart",
  };
  const instances = [
    api,
    temporal,
    duplicate("e5f6", 8082, 95, 6100),
    duplicate("g7h8", 8083, 22, 6101),
    admin,
    mysql,
    loose,
    ignored,
    kickstartDown,
  ];
  return {
    revision: 1,
    instances,
    docker_reachable: true,
    docker_error: null,
    problems: [],
    projects: [
      {
        name: "tienda-demo",
        root: "/work/tienda-demo",
        uses: ["mysql"],
        repos: [
          { name: "tienda-api", path: "/work/tienda-demo/tienda-api", branch: "main" },
          { name: "tienda-admin", path: "/work/tienda-demo/tienda-admin", branch: "develop" },
        ],
        services: [
          { name: "api", repo: "/work/tienda-demo/tienda-api", port: 8080, kind: "app" },
          { name: "admin", repo: "/work/tienda-demo/tienda-admin", port: 8081, kind: "app" },
        ],
      },
      {
        name: "ci4-kickstart",
        root: "/work/ci4-kickstart",
        uses: [],
        repos: [{ name: "ci4-kickstart", path: "/work/ci4-kickstart", branch: "main" }],
        services: [{ name: "web", repo: "/work/ci4-kickstart", port: 8090, kind: "app" }],
      },
    ],
    shared: [{ name: "mysql", start: "docker start mysql", endpoint: null }],
    attention: [
      { kind: "expiring", instance_ref: "c3d4" },
      { kind: "duplicate", instance_ref: "e5f6" },
      { kind: "duplicate", instance_ref: "g7h8" },
    ],
    extend_minutes: 30,
  };
}

export function sampleLog(nowMs: number = Date.now()): LogEntry[] {
  const at = (offsetMinutes: number): string => iso(nowMs, offsetMinutes * 60);
  return [
    {
      at: at(-210),
      kind: "core_started",
      project: null,
      service: null,
      summary: "DevPorts se inició",
    },
    {
      at: at(-180),
      kind: "run_accepted",
      project: "tienda-demo",
      service: "api",
      summary: "api registrada como Principal :8080",
    },
    {
      at: at(-62),
      kind: "run_rejected",
      project: "tienda-demo",
      service: "api",
      summary:
        "un agente (Cursor > zsh) intentó levantar otra instancia de api; la Principal sigue en :8080",
    },
    {
      at: at(-29),
      kind: "run_accepted",
      project: "tienda-demo",
      service: "api",
      summary: "api registrada como Temporal :52814 para «QA login», vence en 30 min",
    },
    {
      at: at(-22),
      kind: "duplicate_detected",
      project: "tienda-demo",
      service: "api",
      summary: "Apareció un duplicado de api :8083 sin registrar",
    },
  ];
}
