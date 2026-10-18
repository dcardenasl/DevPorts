import { describe, expect, it } from "vitest";

import css from "./styles.css?raw";

type Rgb = [number, number, number];
type Rgba = [number, number, number, number];

function token(name: string): string {
  const match = new RegExp(`--${name}:\\s*([^;]+);`).exec(css);
  if (!match?.[1]) throw new Error(`missing token --${name}`);
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

const solid = (name: string): Rgb => over(parse(token(name)), [0, 0, 0]);
const panel = solid("panel");
const sheet = solid("sheet");
const tabbar = solid("tabbar");
const tint = (name: string, alpha: number, base: Rgb): Rgb => {
  const [r, g, b] = parse(token(name));
  return over([r, g, b, alpha], base);
};

// covers: UXR-7.1
describe("contrast of the colour pairs the panel uses (WCAG AA, 4.5:1 for text)", () => {
  const pairs: [string, Rgb, Rgb][] = [
    ["text on panel", solid("text"), panel],
    ["text on sheet", solid("text"), sheet],
    ["soft text on panel", solid("text-soft"), panel],
    ["soft text on sheet", solid("text-soft"), sheet],
    ["soft text on a card", solid("text-soft"), over(parse(token("surface")), panel)],
    ["dim text on panel", solid("text-dim"), panel],
    ["dim text on the tab bar", solid("text-dim"), tabbar],
    ["selected tab on the tab bar", [255, 255, 255], tabbar],
    ["green on panel", solid("green"), panel],
    ["amber on panel", solid("amber"), panel],
    ["coral on panel", solid("coral"), panel],
    ["blue on panel", solid("blue"), panel],
    ["blue on sheet", solid("blue"), sheet],
    ["coral on sheet (errors)", solid("coral"), sheet],
    ["primary button text on green", parse("#06241a").slice(0, 3) as Rgb, solid("green")],
    ["badge text on coral", parse("#1a0d0a").slice(0, 3) as Rgb, solid("coral")],
    ["blue tag", solid("blue"), tint("blue", 0.14, panel)],
    ["green tag", solid("green"), tint("green", 0.14, panel)],
    ["amber tag", solid("amber"), tint("amber", 0.14, panel)],
    ["coral tag", solid("coral"), tint("coral", 0.14, panel)],
    ["soft tag", solid("text-soft"), over([255, 255, 255, 0.08], panel)],
  ];
  it.each(pairs)("%s", (_name, foreground, background) => {
    expect(ratio(foreground, background)).toBeGreaterThanOrEqual(4.5);
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

  it("keeps a visible focus ring", () => {
    expect(css).toMatch(/:focus-visible\s*\{[^}]*outline:\s*2px/);
  });
});
