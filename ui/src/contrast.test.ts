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
  const group = solid("group");
  const sheet = solid("sheet");
  const rgbaOver = (name: string, base: Rgb): Rgb => over(parse(token(theme, name)), base);
  const hoverOnGroup = rgbaOver("hover", group);
  const pressOnGroup = rgbaOver("press", group);
  const hoverOnPanel = rgbaOver("hover", panel);
  const text: [string, Rgb, Rgb][] = [];
  for (const [surface, background] of [
    ["panel", panel],
    ["group", group],
    ["sheet", sheet],
    ["a hovered row", hoverOnGroup],
  ] as const) {
    text.push(
      [`text on ${surface}`, solid("text"), background],
      [`soft text on ${surface}`, solid("text-soft"), background],
      [`dim text on ${surface}`, solid("text-dim"), background],
      [`accent on ${surface}`, solid("accent"), background],
      [`green on ${surface}`, solid("green"), background],
      [`amber on ${surface}`, solid("amber"), background],
      [`red on ${surface}`, solid("red"), background],
    );
  }
  text.push(
    ["text on a pressed row", solid("text"), pressOnGroup],
    ["soft text on a pressed row", solid("text-soft"), pressOnGroup],
    ["soft text on the segmented track", solid("text-soft"), hoverOnPanel],
    ["text on the selected segment", solid("text"), solid("selected")],
    ["text on a field", solid("text"), solid("field")],
    ["soft text on a field", solid("text-soft"), solid("field")],
    ["text on a toast", solid("text"), solid("toast")],
    ["primary button text", solid("on-accent"), solid("accent-fill")],
    ["badge and destructive button text", solid("on-red"), solid("red")],
    ["soft text on the panel's hover", solid("text-soft"), hoverOnPanel],
  );
  return {
    text,
    // Not text: what must still be told apart from the surface it sits on (3:1).
    ui: [
      ["focus ring on panel", solid("accent"), panel],
      ["focus ring on group", solid("accent"), group],
      ["focus ring on sheet", solid("accent"), sheet],
      ["unhealthy ring on group", solid("dot-down"), group],
      ["ownerless ring on group", solid("dot-loose"), group],
      ["unhealthy ring on panel", solid("dot-down"), panel],
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
