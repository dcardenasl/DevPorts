import { describe, expect, it } from "vitest";

import { mockConsumers } from "./mock-consumers";
import { sampleSnapshot } from "./sample";
import type { Snapshot, UseRequest } from "./types";

function setup() {
  let state: Snapshot = sampleSnapshot();
  const mock = mockConsumers(
    () => state,
    (next) => {
      state = { ...next, revision: state.revision + 1 };
    },
  );
  return { mock, snapshot: () => state };
}

const shared = "mysql";

// covers: UXR-4.1
describe("the consumer fixture mirrors the core contract", () => {
  it("lists every project with its declaration and a version to send back", async () => {
    const { mock, snapshot } = setup();
    const options = await mock.sharedConsumers(shared);
    expect(options.defined).toBe(true);
    expect(options.consumers.map((choice) => choice.project)).toEqual(
      snapshot()
        .projects.map((project) => project.name)
        .sort(),
    );
    expect(options.consumers.every((choice) => choice.eligible && choice.base !== null)).toBe(true);
  });

  it("saves several projects together, repeats safely and accepts zero consumers", async () => {
    const { mock, snapshot } = setup();
    const options = await mock.sharedConsumers(shared);
    const requests: UseRequest[] = options.consumers.map((choice) => ({
      project: choice.project,
      include: true,
      base: choice.base ?? "",
    }));
    expect(mock.apply(shared, requests).every((result) => result.ok)).toBe(true);
    expect(snapshot().projects.every((project) => project.uses.includes(shared))).toBe(true);
    const revision = snapshot().revision;
    expect(mock.apply(shared, requests).every((result) => result.ok)).toBe(true);
    for (const project of snapshot().projects)
      expect(project.uses.filter((name) => name === shared)).toHaveLength(1);
    expect(snapshot().revision).toBeGreaterThanOrEqual(revision);
    expect(mock.apply(shared, [])).toEqual([]);
  });

  it("writes nothing when any project fails validation", async () => {
    const { mock, snapshot } = setup();
    const options = await mock.sharedConsumers(shared);
    const first = options.consumers.find((choice) => !choice.declared);
    expect(first).toBeDefined();
    const before = JSON.stringify(snapshot().projects);
    const results = mock.apply(shared, [
      { project: first?.project ?? "", include: true, base: first?.base ?? "" },
      { project: "ghost", include: true, base: "x" },
    ]);
    expect(results[0]).toEqual({ ok: false, error: { code: "blocked" } });
    expect(results[1]).toMatchObject({ ok: false, error: { code: "unknownProject" } });
    expect(JSON.stringify(snapshot().projects)).toBe(before);
  });

  it("refuses a stale version and an undefined shared service", async () => {
    const { mock } = setup();
    const options = await mock.sharedConsumers(shared);
    const target = options.consumers.find((choice) => !choice.declared);
    const stale = mock.apply(shared, [
      { project: target?.project ?? "", include: true, base: "stale" },
    ]);
    expect(stale[0]).toMatchObject({ ok: false, error: { code: "documentChanged" } });
    const undefinedShared = mock.apply("nope", [
      { project: target?.project ?? "", include: true, base: target?.base ?? "" },
    ]);
    expect(undefinedShared[0]).toMatchObject({ ok: false, error: { code: "unknownShared" } });
  });
});
