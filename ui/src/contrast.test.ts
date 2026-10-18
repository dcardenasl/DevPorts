import { describe, expect, it } from "vitest";

import css from "./styles.css?raw";
import themeCss from "./theme.css?raw";

type Rgb = [number, number, number];
type Rgba = [number, number, number, number];

type Theme = "dark" | "light";

const THEME_BLOCKS: Record<Theme, RegExp> = {
  dark: /:root,\s*:root\[data-theme="dark"\]\s*\{([^}]*)\}/,
  light: /:root\[data-theme="light"\]\s*\{([^}]*)\}/,
};

function token(theme: Theme, name: string): string {
  const block = THEME_BLOCKS[theme].exec(themeCss)?.[1] ?? "";
  const match = new RegExp(`--${name}:\\s*([^;]+);`).exec(block);
  if (!match?.[1]) throw new Error(`missing token --${name} in the ${theme} theme`);
  return match[1].trim();
}

function parse(color: string): Rgba {
  const hex = /^#([0-9a-f]{6})$/i.exec(color);
  if (hex?.[1]) {
    const value = parseInt(hex[1], 16);
    return [(value >> 16) & 255, (value >> 8) & 255, value & 255, 1];
  }
  const rgba = /^rgba?\(([^)]+)\)$/.exec(color);
  if (rgba?.[1]) {
    const [r = 0, g = 0, b = 0, a = 1] = rgba[1].split(",").map((part) => Number(part.trim()));
    return [r, g, b, a];
  }
  throw new Error(`unsupported color ${color}`);
}

/** `top` painted over an opaque `base`. */
function over(top: Rgba, base: Rgb): Rgb {
  const [r, g, b, a] = top;
  return [
    Math.round(r * a + base[0] * (1 - a)),
    Math.round(g * a + base[1] * (1 - a)),
    Math.round(b * a + base[2] * (1 - a)),
  ];
}

function luminance([r, g, b]: Rgb): number {
  const channel = (value: number): number => {
    const unit = value / 255;
    return unit <= 0.03928 ? unit / 12.92 : ((unit + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

function ratio(foreground: Rgb, background: Rgb): number {
  const [light, dark] = [luminance(foreground), luminance(background)].sort((a, b) => b - a) as [
    number,
    number,
  ];
  return (light + 0.05) / (dark + 0.05);
}

const THEMES: Theme[] = ["dark", "light"];

/** Every colour pair the panel paints, resolved against one theme's tokens. */
function pairs(theme: Theme): { text: [string, Rgb, Rgb][]; ui: [string, Rgb, Rgb][] } {
  const solid = (name: string): Rgb => over(parse(token(theme, name)), [0, 0, 0]);
  const panel = solid("panel");
  const sheet = solid("sheet");
  const tabbar = solid("tabbar");
  const rgbaOver = (name: string, base: Rgb): Rgb => over(parse(token(theme, name)), base);
  return {
    text: [
      ["text on panel", solid("text"), panel],
      ["text on sheet", solid("text"), sheet],
      ["soft text on panel", solid("text-soft"), panel],
      ["soft text on sheet", solid("text-soft"), sheet],
      ["soft text on a card", solid("text-soft"), rgbaOver("surface", panel)],
      ["dim text on panel", solid("text-dim"), panel],
      ["dim text on the tab bar", solid("text-dim"), tabbar],
      ["selected tab on the tab bar", solid("text-strong"), tabbar],
      ["green on panel", solid("green"), panel],
      ["amber on panel", solid("amber"), panel],
      ["coral on panel", solid("coral"), panel],
      ["blue on panel", solid("blue"), panel],
      ["blue on sheet", solid("blue"), sheet],
      ["coral on sheet (errors)", solid("coral"), sheet],
      ["primary green button text", solid("on-green"), solid("green")],
      ["primary amber button text", solid("on-amber"), solid("amber")],
      ["primary coral button text", solid("on-coral"), solid("coral")],
      ["badge text on coral", solid("on-badge"), solid("coral")],
      ["blue tag", solid("blue"), rgbaOver("tint-blue", panel)],
      ["green tag", solid("green"), rgbaOver("tint-green", panel)],
      ["amber tag", solid("amber"), rgbaOver("tint-amber", panel)],
      ["coral tag", solid("coral"), rgbaOver("tint-coral", panel)],
      ["soft tag", solid("text-soft"), rgbaOver("ov-8", panel)],
      ["green on its card", solid("green"), rgbaOver("wash-green", panel)],
      ["amber on its card", solid("amber"), rgbaOver("wash-amber", panel)],
      ["coral on its card", solid("coral"), rgbaOver("wash-coral", panel)],
      ["blue on its card", solid("blue"), rgbaOver("wash-blue", panel)],
      ["active filter pill", solid("pill-fg"), solid("pill-bg")],
      ["text in a field", solid("text"), solid("field")],
      ["text in a toast", solid("text"), solid("toast")],
    ],
    // Not text: what must still be told apart from the surface it sits on (3:1).
    ui: [
      ["focus ring on panel", solid("blue"), panel],
      ["focus ring on sheet", solid("blue"), sheet],
      ["unhealthy ring", solid("dot-down"), panel],
      ["ownerless ring", solid("dot-loose"), panel],
      ["active filter pill on panel", solid("pill-bg"), panel],
    ],
  };
}

describe.each(THEMES)("contrast of the colour pairs the panel uses in the %s theme", (theme) => {
  const { text, ui } = pairs(theme);

  // covers: UXR-7.1, UXR-9.2
  it.each(text)("text pair: %s (WCAG AA, 4.5:1)", (_name, foreground, background) => {
    expect(ratio(foreground, background)).toBeGreaterThanOrEqual(4.5);
  });

  // covers: UXR-9.2
  it.each(ui)("non-text pair: %s (3:1)", (_name, foreground, background) => {
    expect(ratio(foreground, background)).toBeGreaterThanOrEqual(3);
  });
});

// covers: UXR-7.1
describe("layout rules that keep the panel usable at 380×520 and with long text", () => {
  it("honours reduced motion for animations and transitions", () => {
    const block =
      /@media \(prefers-reduced-motion: reduce\)\s*\{([\s\S]*?)\n\}/.exec(css)?.[1] ?? "";
    expect(block).toContain("animation-duration");
    expect(block).toContain("transition-duration");
  });

  it("lets sheets scroll and break long paths, keeps their actions reachable and wraps buttons", () => {
    const sheet = /\.sheet\s*\{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(sheet).toContain("overflow-y: auto");
    expect(sheet).toContain("overflow-wrap: anywhere");
    expect(sheet).toContain("max-height: 100%");
    const footer = /\.adoption-footer\s*\{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(footer).toContain("position: sticky");
    const button = /\.btn\s*\{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(button).toContain("white-space: normal");
    expect(button).toContain("min-height");
  });

  it("keeps list items at their size so a long list scrolls instead of collapsing cards", () => {
    const rule = /\.body > \*\s*\{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(rule).toContain("flex-shrink: 0");
  });

  it("keeps a visible focus ring", () => {
    expect(css).toMatch(/:focus-visible\s*\{[^}]*outline:\s*2px/);
  });
});
