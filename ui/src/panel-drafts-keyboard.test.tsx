import { fakeBackend, open, resetBackend, state, tab } from "./test-utils";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";
import { sampleSnapshot } from "./sample";

describe("the panel: keyboard and recovered drafts", () => {
  beforeEach(resetBackend);

  // covers: UI, UXR-1.1
  it("cancels a shared stop with Escape, traps Tab and restores its invoker", async () => {
    const { backend, user } = await open();
    await user.click(tab(/Compartidos/));
    const invoker = screen.getByRole("button", { name: "Detener mysql" });
    await user.click(invoker);
    const dialog = screen.getByRole("alertdialog");
    const cancel = within(dialog).getByRole("button", { name: "Cancelar" });
    const confirm = within(dialog).getByRole("button", { name: "Detener mysql" });
    expect(cancel).toHaveFocus();
    await user.tab({ shift: true });
    expect(confirm).toHaveFocus();
    await user.tab();
    expect(cancel).toHaveFocus();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(invoker).toHaveFocus();
    expect(backend.calls.operation).toBeUndefined();
  });

  // covers: UI, UXR-1.1
  it("returns from both adoption steps without losing the draft or writing anything", async () => {
    const { backend, user } = await open();
    await user.click(tab(/Sin proyecto/));
    const invoker = screen.getByRole("button", { name: "Adoptar" });
    await user.click(invoker);
    const dialog = screen.getByRole("dialog");
    expect(dialog.contains(document.activeElement)).toBe(true);
    const create = within(dialog).getByRole("button", { name: /Crear un proyecto nuevo/ });
    await user.click(create);
    const input = screen.getByLabelText("Nombre del proyecto");
    expect(input).toHaveFocus();
    await user.clear(input);
    await user.type(input, "mi borrador");
    await user.click(screen.getByRole("button", { name: "Volver" }));
    expect(screen.getByRole("button", { name: /Crear un proyecto nuevo/ })).toHaveFocus();
    await user.click(screen.getByRole("button", { name: /Otro proyecto/ }));
    expect(screen.getByRole("button", { name: "Como servicio de tienda-demo" })).toBeDisabled();
    expect(screen.getAllByText(/fuera de la raíz/).length).toBeGreaterThan(0);
    await user.click(screen.getByRole("button", { name: "Volver" }));
    expect(screen.getByRole("button", { name: /Otro proyecto/ })).toHaveFocus();
    await user.click(screen.getByRole("button", { name: /Crear un proyecto nuevo/ }));
    expect(screen.getByLabelText("Nombre del proyecto")).toHaveValue("mi borrador");
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(invoker).toHaveFocus();
    expect(backend.calls["adopt"]).toBeUndefined();
    expect(backend.calls["keep"]).toBeUndefined();
    expect(backend.calls.operation).toBeUndefined();
  });

  // covers: UI, UXR-1.1
  it("keeps keyboard navigation inside the adoption form and cancels without submitting", async () => {
    const { backend, user } = await open();
    await user.click(tab(/Sin proyecto/));
    const invoker = screen.getByRole("button", { name: "Adoptar" });
    await user.click(invoker);
    await user.click(screen.getByRole("button", { name: /Crear un proyecto nuevo/ }));
    const input = screen.getByLabelText("Nombre del proyecto");
    await user.tab({ shift: true });
    expect(screen.getByRole("button", { name: "Cancelar" })).toHaveFocus();
    await user.tab();
    expect(input).toHaveFocus();
    await user.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(invoker).toHaveFocus();
    expect(backend.calls["adopt"]).toBeUndefined();
    expect(backend.calls["keep"]).toBeUndefined();
  });

  // covers: UI, UXR-1.2
  it("restores navigation and the activity filter after the panel is recreated", async () => {
    const backend = fakeBackend(sampleSnapshot());
    state.current = backend;
    const user = userEvent.setup();
    const first = render(<App />);
    await screen.findByRole("tab", { name: /Atención/ });
    await user.click(tab(/Proyectos/));
    await user.click(screen.getByRole("button", { name: /ci4-kickstart/ }));
    await user.click(screen.getByRole("button", { name: /tienda-demo/ }));
    await user.click(tab(/Actividad/));
    await user.click(screen.getByRole("button", { name: "Alertas" }));
    await waitFor(() => {
      expect(backend.getPersisted().activityFilter).toBe("alerts");
    });
    first.unmount();

    render(<App />);
    await waitFor(() => expect(tab(/Actividad/)).toHaveAttribute("aria-selected", "true"));
    expect(screen.getByRole("button", { name: "Alertas" })).toHaveAttribute("aria-pressed", "true");
    await user.click(tab(/Proyectos/));
    expect(screen.getByRole("button", { name: /ci4-kickstart/ })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(screen.getByRole("button", { name: /tienda-demo/ })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
    cleanup();
  });

  // covers: UI, UXR-1.2
  it("restores an adoption draft without restoring a destructive confirmation", async () => {
    const backend = fakeBackend(sampleSnapshot(), {
      version: 3,
      tab: "orphans",
      openProjects: [],
      closedProjects: [],
      activityFilter: "all",
      draft: {
        kind: "adopt",
        reference: "z9y8",
        mode: "new",
        name: "unfinished-project",
        lastStep: "new",
      },
    });
    state.current = backend;
    const user = userEvent.setup();
    render(<App />);
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByLabelText("Nombre del proyecto")).toHaveValue("unfinished-project");
    expect(backend.calls.operation).toBeUndefined();
    expect(backend.calls["adopt"]).toBeUndefined();
    await user.keyboard("{Escape}");
    await waitFor(() => {
      expect(backend.getPersisted().draft).toBeNull();
    });
  });

  // covers: UI, UXR-1.2
  it("discards and explains a draft whose process no longer exists", async () => {
    const backend = fakeBackend(sampleSnapshot(), {
      version: 3,
      tab: "orphans",
      openProjects: [],
      closedProjects: [],
      activityFilter: "all",
      draft: {
        kind: "adopt",
        reference: "gone",
        mode: "new",
        name: "unfinished-project",
        lastStep: null,
      },
    });
    state.current = backend;
    render(<App />);
    expect(await screen.findByText(/ya no está disponible/)).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await waitFor(() => {
      expect(backend.getPersisted().draft).toBeNull();
    });
    expect(backend.calls.operation).toBeUndefined();
    expect(backend.calls["adopt"]).toBeUndefined();
  });
});
