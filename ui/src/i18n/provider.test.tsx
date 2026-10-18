import { act, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { I18nProvider, useI18n } from "./provider";

function Probe() {
  const { locale, text } = useI18n();
  return (
    <p>
      {locale}:{text.tabs.projects}
    </p>
  );
}

function setSystemLanguages(tags: string[]): void {
  Object.defineProperty(window.navigator, "languages", { configurable: true, get: () => tags });
}

// covers: UXR-9.3
describe("the i18n provider", () => {
  afterEach(() => {
    setSystemLanguages(["es-CL", "es"]);
    document.documentElement.removeAttribute("lang");
  });

  it("speaks Spanish, the source language, when no provider is mounted", () => {
    render(<Probe />);
    expect(screen.getByText("es:Proyectos")).toBeInTheDocument();
  });

  it("gives the catalog of the chosen language and writes it on the document", () => {
    render(
      <I18nProvider preference="en">
        <Probe />
      </I18nProvider>,
    );
    expect(screen.getByText("en:Projects")).toBeInTheDocument();
    expect(document.documentElement.lang).toBe("en");
  });

  it("follows the system languages for system, live, and not for a chosen language", () => {
    setSystemLanguages(["en-GB"]);
    const system = render(
      <I18nProvider preference="system">
        <Probe />
      </I18nProvider>,
    );
    expect(screen.getByText("en:Projects")).toBeInTheDocument();

    act(() => {
      setSystemLanguages(["es-MX"]);
      window.dispatchEvent(new Event("languagechange"));
    });
    expect(screen.getByText("es:Proyectos")).toBeInTheDocument();
    expect(document.documentElement.lang).toBe("es");
    system.unmount();

    setSystemLanguages(["en-GB"]);
    render(
      <I18nProvider preference="es">
        <Probe />
      </I18nProvider>,
    );
    act(() => {
      window.dispatchEvent(new Event("languagechange"));
    });
    expect(screen.getByText("es:Proyectos")).toBeInTheDocument();
  });
});
