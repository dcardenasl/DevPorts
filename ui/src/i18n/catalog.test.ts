import { describe, expect, it } from "vitest";

import { en } from "./en";
import { es } from "./es";

type Node = string | ((...args: never[]) => string) | { [key: string]: Node };

/** Every leaf of a catalog with its path: `adoption.save`, `projects.summary`, … */
function leaves(node: Node, path = ""): [string, string | ((...args: never[]) => string)][] {
  if (typeof node === "string" || typeof node === "function") return [[path, node]];
  return Object.entries(node).flatMap(([key, child]) =>
    leaves(child, path === "" ? key : `${path}.${key}`),
  );
}

const spanish = leaves(es);
const english = leaves(en);

// covers: UXR-9.3
describe("the catalogs", () => {
  it("have the very same keys, in the same shape", () => {
    expect(english.map(([path]) => path)).toEqual(spanish.map(([path]) => path));
  });

  it("take the same arguments wherever a phrase is a function", () => {
    const englishByPath = new Map(english);
    for (const [path, value] of spanish) {
      const other = englishByPath.get(path);
      expect(typeof other, path).toBe(typeof value);
      if (typeof value === "function" && typeof other === "function")
        expect(other.length, path).toBe(value.length);
    }
  });

  it("have no empty phrase", () => {
    for (const [path, value] of [...spanish, ...english])
      if (typeof value === "string") expect(value.trim(), path).not.toBe("");
  });

  it("keep Spanish out of the English one, but for the name of Spanish in Spanish", () => {
    for (const [path, value] of english)
      if (typeof value === "string" && path !== "preferences.languages.es")
        expect(value, path).not.toMatch(/[áéíóúñ¿¡«»]/);
  });

  it("count in each language's own way", () => {
    expect(es.projects.summary(1, 1, 1)).toBe("1 proyecto · 1 repo · 1 servicio");
    expect(es.projects.summary(2, 3, 0)).toBe("2 proyectos · 3 repos · 0 servicios");
    expect(en.projects.summary(1, 1, 1)).toBe("1 project · 1 repo · 1 service");
    expect(en.projects.summary(2, 3, 0)).toBe("2 projects · 3 repos · 0 services");
    expect(es.attention.waiting(1)).toBe("1 cosa espera tu decisión");
    expect(en.attention.waiting(1)).toBe("1 thing is waiting for your decision");
    expect(en.attention.waiting(3)).toBe("3 things are waiting for your decision");
    expect(en.orphans.summary(0)).toBe("Nothing loose");
    expect(en.orphans.summary(2)).toBe("2 processes without an owner");
  });
});
