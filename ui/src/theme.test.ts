import { afterEach, describe, expect, it, vi } from "vitest";

import styles from "./styles.css?raw";
import {
  createThemeController,
  parseThemePreference,
  resolveTheme,
  type SystemAppearance,
} from "./theme";
import themeCss from "./theme.css?raw";

/** A stand-in for `matchMedia("(prefers-color-scheme: dark)")` that tests can flip. */
function fakeSystem(dark: boolean) {
  const listeners = new Set<() => void>();
  const system: SystemAppearance & { matches: boolean } = {
    matches: dark,
    addEventListener: (_type: string, listener: unknown) => {
      listeners.add(listener as () => void);
    },
    removeEventListener: (_type: string, listener: unknown) => {
      listeners.delete(listener as () => void);
    },
  };
  return {
    system,
    listeners,
    flip: (next: boolean) => {
      system.matches = next;
      listeners.forEach((listener) => {
        listener();
      });
    },
  };
}

// covers: UXR-9.2
describe("which theme a preference asks for", () => {
  it.each([
    ["light", "light"],
    ["dark", "dark"],
    ["system", "system"],
    ["sepia", "system"],
    ["", "system"],
    [null, "system"],
  ] as const)("reads %j from the URL as %s", (value, expected) => {
    expect(parseThemePreference(value)).toBe(expected);
  });

  it("follows the system only for system", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
  });
});

// covers: UXR-9.2
describe("the theme controller", () => {
  it("wears an explicit theme whatever the system does", () => {
    const { system, flip, listeners } = fakeSystem(true);
    const root = document.createElement("html");
    const controller = createThemeController(root, system);

    controller.set("light");
    flip(false);
    flip(true);

    expect(root.dataset.theme).toBe("light");
    expect(listeners.size).toBe(0);
  });

  it("follows the system while the preference is system, live", () => {
    const { system, flip, listeners } = fakeSystem(false);
    const root = document.createElement("html");
    const controller = createThemeController(root, system);

    controller.set("system");
    expect(root.dataset.theme).toBe("light");
    flip(true);
    expect(root.dataset.theme).toBe("dark");
    expect(controller.resolved).toBe("dark");
    expect(controller.preference).toBe("system");
    expect(listeners.size).toBe(1);
  });

  it("never listens twice and stops listening when an explicit theme is chosen", () => {
    const { system, listeners } = fakeSystem(true);
    const controller = createThemeController(document.createElement("html"), system);

    controller.set("system");
    controller.set("system");
    expect(listeners.size).toBe(1);
    controller.set("dark");
    expect(listeners.size).toBe(0);
  });

  it("counts the system as dark when it cannot be asked", () => {
    const root = document.createElement("html");
    createThemeController(root, null).set("system");
    expect(root.dataset.theme).toBe("dark");
  });
});

// covers: UXR-9.2
describe("the boot module", () => {
  afterEach(() => {
    vi.resetModules();
    vi.unstubAllGlobals();
    window.history.replaceState(null, "", "/");
    delete document.documentElement.dataset.theme;
  });

  async function boot(search: string, systemIsDark: boolean): Promise<string | undefined> {
    vi.resetModules();
    window.history.replaceState(null, "", `/index.html${search}`);
    vi.stubGlobal("matchMedia", () => fakeSystem(systemIsDark).system);
    await import("./boot");
    return document.documentElement.dataset.theme;
  }

  it("puts the theme from the URL on the page before anything renders", async () => {
    await expect(boot("?tab=projects&theme=light", true)).resolves.toBe("light");
    await expect(boot("?theme=dark", false)).resolves.toBe("dark");
  });

  it("resolves system, a missing parameter and a junk one with the system appearance", async () => {
    await expect(boot("?theme=system", false)).resolves.toBe("light");
    await expect(boot("", true)).resolves.toBe("dark");
    await expect(boot("?theme=sepia", false)).resolves.toBe("light");
  });
});

// covers: UXR-9.2
describe("the stylesheets", () => {
  const block = (selector: RegExp): string => selector.exec(themeCss)?.[1] ?? "";
  const dark = block(/:root,\s*:root\[data-theme="dark"\]\s*\{([^}]*)\}/);
  const light = block(/:root\[data-theme="light"\]\s*\{([^}]*)\}/);
  const names = (css: string): string[] =>
    [...css.matchAll(/(--[a-z0-9-]+):/g)].map((match) => match[1] ?? "").sort();

  it("define the very same tokens in both themes", () => {
    expect(names(dark).length).toBeGreaterThan(20);
    expect(names(light)).toEqual(names(dark));
  });

  it("declare a colour scheme per theme", () => {
    expect(dark).toContain("color-scheme: dark");
    expect(light).toContain("color-scheme: light");
  });

  it("keep every colour literal in theme.css", () => {
    expect(styles).not.toMatch(/#[0-9a-fA-F]{3,8}\b/);
    expect(styles).not.toMatch(/\brgba?\(|\bhsla?\(|\boklch\(|\bcolor-mix\(|\blight-dark\(/);
  });

  it("only use tokens that exist", () => {
    const defined = new Set([...names(dark), "--font", "--mono", "--inset"]);
    const used = [...styles.matchAll(/var\((--[a-z0-9-]+)/g)].map((match) => match[1] ?? "");
    expect(used.filter((name) => !defined.has(name))).toEqual([]);
  });
});

// covers: UXR-10.1
describe("the visual system stays whole (ADR-042)", () => {
  const sources = import.meta.glob("./**/*.tsx", { query: "?raw", import: "default", eager: true });
  const files = Object.entries(sources).filter(([name]) => !name.includes(".test."));

  it("has no card, state-coloured button or capital label left in a component", () => {
    const retired = [
      /className="card/,
      /primary-(green|coral|amber)/,
      /(green|coral|amber|blue)-text/,
      /className="[^"]*\beyebrow\b/,
      /\bkind="coral"/,
      /<Tag\s+tone=/,
    ];
    for (const [name, source] of files)
      for (const pattern of retired)
        expect(source, `${name} ${String(pattern)}`).not.toMatch(pattern);
  });

  it("styles nothing inline but the width of a progress bar", () => {
    for (const [name, source] of files) {
      const inline = [...source.matchAll(/style=\{\{([^}]*)\}\}/g)].map((m) => m[1]);
      const unexpected = inline.filter((value) => !/^\s*width:\s*`\$\{/.test(value ?? ""));
      expect(unexpected, name).toEqual([]);
    }
  });

  it("keeps status colour out of surfaces: no wash, edge, tint or halo, in either file", () => {
    expect(themeCss).not.toMatch(/--(wash|edge|tint|halo)-/);
    expect(styles).not.toMatch(/--(wash|edge|tint|halo)-/);
  });
});
