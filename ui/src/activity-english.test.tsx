import { open, resetBackend, tab } from "./test-utils";
import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { sampleSnapshot } from "./sample";

// covers: UXR-9.4
describe("the activity in English", () => {
  beforeEach(resetBackend);

  it("writes every entry from its facts, not from the core's Spanish sentence", async () => {
    const { user } = await open(sampleSnapshot(), "en");
    await user.click(tab(/Activity/));
    expect(await screen.findByText("Test of api :52814 for “QA login”")).toBeInTheDocument();
    expect(screen.getByText("An unregistered duplicate of api :8083 appeared")).toBeInTheDocument();
    expect(screen.queryByText(/Prueba de api/)).not.toBeInTheDocument();
  });
});
