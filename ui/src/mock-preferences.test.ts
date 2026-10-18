import { describe, expect, it } from "vitest";

import { mockPreferences } from "./mock-preferences";

describe("the preferences fixture", () => {
  // covers: UXR-9.1
  it("starts as system and returns what was saved", async () => {
    const fixture = mockPreferences();
    await expect(fixture.preferences()).resolves.toEqual({ theme: "system", language: "system" });
    await fixture.setPreferences({ theme: "light", language: "es" });
    await expect(fixture.preferences()).resolves.toEqual({ theme: "light", language: "es" });
  });
});
