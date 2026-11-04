import { fakeBackend, open, state } from "./test-utils";
import { render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { sampleSnapshot } from "./sample";

// covers: UXR-9.6
describe("the appearance and language in Configuración", () => {
  beforeEach(() => {
    state.current = undefined;
    Element.prototype.scrollIntoView = vi.fn();
    delete document.documentElement.dataset.theme;
  });

  it("changes the theme and the language at once, and saves them", async () => {
    const { backend, user } = await open();
    await user.click(screen.getByRole("button", { name: "Configuración" }));
    const dialog = screen.getByRole("dialog");
    const theme = within(dialog).getByRole("group", { name: "Tema" });
    expect(within(theme).getByRole("radio", { name: "Sistema" })).toBeChecked();

    await user.click(within(theme).getByRole("radio", { name: "Oscuro" }));
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(backend.calls.setPreferences?.at(-1)).toEqual([{ theme: "dark", language: "system" }]);

    await user.click(
      within(within(dialog).getByRole("group", { name: "Idioma" })).getByRole("radio", {
        name: "English",
      }),
    );
    // The whole panel changes language in place, the open sheet included.
    expect(await within(dialog).findByRole("group", { name: "Language" })).toBeInTheDocument();
    expect(within(dialog).getByRole("heading", { name: "Settings" })).toBeInTheDocument();
    expect(document.documentElement.lang).toBe("en");
    expect(backend.calls.setPreferences?.at(-1)).toEqual([{ theme: "dark", language: "en" }]);
    await expect(backend.preferences()).resolves.toEqual({ theme: "dark", language: "en" });
  });

  it("wears what the core has saved, whatever the page started with", async () => {
    const backend = fakeBackend(sampleSnapshot(), undefined, [], {
      theme: "light",
      language: "en",
    });
    state.current = backend;
    render(<App />);
    expect(await screen.findByRole("heading", { level: 1, name: "Attention" })).toBeInTheDocument();
    expect(document.documentElement.dataset.theme).toBe("light");
  });

  it("goes back to what was there, and says so, when saving fails", async () => {
    const { backend, user } = await open();
    backend.setPreferences = () => Promise.reject(new Error("disk full"));
    await user.click(screen.getByRole("button", { name: "Configuración" }));
    const dialog = screen.getByRole("dialog");
    const theme = within(dialog).getByRole("group", { name: "Tema" });

    await user.click(within(theme).getByRole("radio", { name: "Oscuro" }));

    expect(await within(dialog).findByRole("alert")).toHaveTextContent("No se pudo guardar");
    expect(within(theme).getByRole("radio", { name: "Sistema" })).toBeChecked();
    expect(document.documentElement.dataset.theme).toBe("dark"); // the system's, when no media is known
  });

  it("uses native radio buttons, so the browser gives the arrow keys and the semantics", async () => {
    const { user } = await open();
    await user.click(screen.getByRole("button", { name: "Configuración" }));
    const dialog = screen.getByRole("dialog");
    for (const [legend, count] of [
      ["Tema", 3],
      ["Idioma", 3],
    ] as const) {
      const radios = within(within(dialog).getByRole("group", { name: legend })).getAllByRole(
        "radio",
      );
      expect(radios).toHaveLength(count);
      expect(new Set(radios.map((radio) => radio.getAttribute("name"))).size).toBe(1);
      expect(radios.every((radio) => radio instanceof HTMLInputElement)).toBe(true);
    }
  });
});
