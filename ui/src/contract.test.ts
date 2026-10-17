import { describe, expect, it } from "vitest";

import fixture from "./fixtures/snapshot.json";
import { sampleSnapshot } from "./sample";

const keys = (value: object): string[] => Object.keys(value).sort();

// The fixture is what the Rust core serialises (checked by its own test); the
// sample is typed as `Snapshot`, so TypeScript guarantees it has every field of
// `types.ts`. Equal key sets mean the two sides describe the same shape.
describe("contract with the core", () => {
  const sample = sampleSnapshot();

  it("has the same snapshot fields", () => {
    expect(keys(fixture)).toEqual(keys(sample));
  });

  it("has the same instance, project, repo, service and attention fields", () => {
    expect(keys(fixture.instances[0] ?? {})).toEqual(keys(sample.instances[0] ?? {}));
    expect(keys(fixture.projects[0] ?? {})).toEqual(keys(sample.projects[0] ?? {}));
    expect(keys(fixture.projects[0]?.repos[0] ?? {})).toEqual(
      keys(sample.projects[0]?.repos[0] ?? {}),
    );
    expect(keys(fixture.projects[0]?.services[0] ?? {})).toEqual(
      keys(sample.projects[0]?.services[0] ?? {}),
    );
    expect(keys(fixture.attention[0] ?? {})).toEqual(keys(sample.attention[0] ?? {}));
  });
});
