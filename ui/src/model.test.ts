import { describe, expect, it } from "vitest";

import {
  buildAttention,
  buildOrphans,
  buildProjects,
  buildShared,
  dependentsOf,
  startCommand,
} from "./model";
import { sampleSnapshot } from "./sample";

describe("model", () => {
  const snapshot = sampleSnapshot();

  it("lays projects out as repos with services, temporals and duplicates inside", () => {
    const shop = buildProjects(snapshot).find((project) => project.summary.name === "tienda-demo");
    expect(shop?.repos.map((repo) => repo.repo.name)).toEqual(["tienda-api", "tienda-admin"]);
    const api = shop?.repos[0]?.services[0];
    expect(api?.principal?.state).toBe("healthy");
    expect(api?.temporals).toHaveLength(1);
    expect(api?.duplicates).toHaveLength(2);
    expect(shop?.serviceCount).toBe(2);
    expect(shop?.runningCount).toBe(2);
  });

  it("groups what needs attention and shows the official instance as context", () => {
    const attention = buildAttention(snapshot);
    expect(attention.expiring.map((view) => view.ref)).toEqual(["c3d4"]);
    expect(attention.groups).toHaveLength(1);
    expect(attention.groups[0]?.duplicates).toHaveLength(2);
    expect(attention.official.map((view) => view.ref)).toEqual(["a1b2"]);
    expect(attention.hung).toHaveLength(0);
  });

  // covers: UXR-2.3
  it("keeps uncertainty separate from observed activity and counts live Temporals", () => {
    const altered = sampleSnapshot();
    altered.instances = altered.instances.map((view) =>
      view.project === "tienda-demo"
        ? { ...view, state: view.role === "temporal" ? "healthy" : "unknown" }
        : view,
    );
    const project = buildProjects(altered).find((node) => node.summary.name === "tienda-demo");
    expect(project?.runningCount).toBe(1);
    expect(project?.unknownCount).toBe(4);
  });

  it("finds shared services with the projects that use them", () => {
    const shared = buildShared(snapshot);
    expect(shared.map((node) => node.name)).toEqual(["mysql"]);
    expect(shared[0]?.users).toEqual(["tienda-demo"]);
  });

  it("lists loose processes and leaves out the ones that were ignored", () => {
    expect(buildOrphans(snapshot).map((view) => view.ref)).toEqual(["z9y8"]);
  });

  it("names the projects hurt by stopping a shared service", () => {
    const mysql = snapshot.instances.find((view) => view.service === "mysql");
    expect(mysql).toBeDefined();
    if (mysql !== undefined) expect(dependentsOf(mysql, snapshot)).toEqual(["tienda-demo"]);
  });

  it("builds a start command from the project folder", () => {
    expect(startCommand(snapshot.projects[0], "api")).toBe(
      "cd /work/tienda-demo && devports run api",
    );
    expect(startCommand(undefined, "api")).toBe("devports run api");
  });
});
