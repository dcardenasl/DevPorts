import { open, resetBackend } from "./test-utils";
import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { sampleSnapshot } from "./sample";

// covers: UXR-9.7
describe("the core's reasons in English", () => {
  beforeEach(resetBackend);

  it("writes the reason a typed folder is refused beside it", async () => {
    const { user } = await open(sampleSnapshot(), "en");
    await user.click(screen.getByRole("button", { name: "Settings" }));
    const dialog = screen.getByRole("dialog");
    await user.click(await within(dialog).findByRole("button", { name: "Add folder" }));
    await user.type(within(dialog).getByLabelText("Folder 2"), "/Users/demo/missing");
    await user.click(within(dialog).getByRole("button", { name: "Review the effect" }));
    expect(await within(dialog).findByText("the folder does not exist")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Remove /Users/demo/missing" }));
    await user.click(within(dialog).getByRole("button", { name: "Remove ~/developer" }));
    await user.click(within(dialog).getByRole("button", { name: "Add folder" }));
    await user.click(within(dialog).getByRole("button", { name: "Review the effect" }));
    expect(await within(dialog).findByText("type a folder")).toBeInTheDocument();
  });
});
