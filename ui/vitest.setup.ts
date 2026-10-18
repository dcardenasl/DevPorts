import "@testing-library/jest-dom/vitest";

// The suite runs in Spanish, the source language: the system languages the panel would
// follow are Spanish ones, as on the machine it was written on.
Object.defineProperty(window.navigator, "languages", {
  configurable: true,
  get: () => ["es-CL", "es"],
});
