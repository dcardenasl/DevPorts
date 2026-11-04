import { open, resetBackend, tab } from "./test-utils";
import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { sampleSnapshot } from "./sample";

// covers: UXR-9.3
describe("the panel in English", () => {
  beforeEach(resetBackend);

  it("opens on Attention with the counter, the official instance and every alert", async () => {
    await open(sampleSnapshot(), "en");
    expect(screen.getByRole("heading", { level: 1, name: "Attention" })).toBeInTheDocument();
    expect(screen.getByText("3 things are waiting for your decision")).toBeInTheDocument();
    expect(screen.getByText("OFFICIAL")).toBeInTheDocument();
    expect(screen.getByText("TEMPORARY, ABOUT TO EXPIRE")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Stop all 2" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Extend 30 min" })).toBeInTheDocument();
    expect(document.documentElement.lang).toBe("en");
  });

  it("names every section and its subtitle in English", async () => {
    const { user } = await open(sampleSnapshot(), "en");
    expect(screen.getByRole("tablist", { name: "Sections" })).toBeInTheDocument();
    for (const [name, heading] of [
      [/Projects/, "Projects"],
      [/Shared/, "Shared"],
      [/No project/, "No project"],
      [/Activity/, "Activity"],
    ] as const) {
      await user.click(tab(name));
      expect(screen.getByRole("heading", { level: 1, name: heading })).toBeInTheDocument();
    }
  });

  it("writes the activity from the log with English labels and filters", async () => {
    const { user } = await open(sampleSnapshot(), "en");
    await user.click(tab(/Activity/));
    await screen.findByRole("group", { name: "Filter" });
    expect(screen.getByRole("button", { name: "Rejections" })).toBeInTheDocument();
    expect(screen.getAllByText("REJECTED").length).toBeGreaterThan(0);
  });

  it("opens the settings sheet in English", async () => {
    const { user } = await open(sampleSnapshot(), "en");
    await user.click(screen.getByRole("button", { name: "Settings" }));
    expect(await screen.findByText("Scan folders")).toBeInTheDocument();
  });
});
