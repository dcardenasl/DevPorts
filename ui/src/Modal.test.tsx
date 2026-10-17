import { StrictMode, useRef, useState } from "react";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { Modal } from "./Modal";

function Example({ disabledControls = false }: { disabledControls?: boolean }) {
  const [invoker, setInvoker] = useState<HTMLButtonElement>();
  const initialRef = useRef<HTMLButtonElement>(null);
  return (
    <>
      <button
        type="button"
        onClick={(event) => {
          setInvoker(event.currentTarget);
        }}
      >
        Abrir diálogo
      </button>
      <button type="button">Acción del fondo</button>
      {invoker !== undefined && (
        <Modal
          title="Ejemplo"
          description="Descripción de la operación"
          returnFocusTo={invoker}
          onCancel={() => {
            setInvoker(undefined);
          }}
          {...(disabledControls ? {} : { initialFocusRef: initialRef })}
        >
          <button type="button" disabled={disabledControls} ref={initialRef}>
            Primera
          </button>
          <button type="button" disabled>
            Deshabilitada
          </button>
          <button type="button" hidden>
            Oculta
          </button>
          <button type="button" disabled={disabledControls}>
            Última
          </button>
          <div style={{ display: "none" }}>
            <button type="button">En grupo oculto</button>
          </div>
          <input type="hidden" tabIndex={0} />
        </Modal>
      )}
    </>
  );
}

describe("panel modal", () => {
  // covers: UI, UXR-1.1
  it("names and describes the dialog, protects the background and cleans up in StrictMode", async () => {
    const user = userEvent.setup();
    const { container } = render(
      <StrictMode>
        <Example />
      </StrictMode>,
    );
    const invoker = screen.getByRole("button", { name: "Abrir diálogo" });
    await user.click(invoker);
    const dialog = screen.getByRole("dialog", { name: "Ejemplo" });
    expect(dialog).toHaveAccessibleDescription("Descripción de la operación");
    expect(container).toHaveAttribute("inert");
    expect(container).toHaveAttribute("aria-hidden", "true");
    expect(screen.queryByRole("button", { name: "Acción del fondo" })).not.toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Primera" })).toHaveFocus();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(container).not.toHaveAttribute("inert");
    expect(container).not.toHaveAttribute("aria-hidden");
    expect(invoker).toHaveFocus();
    await user.click(invoker);
    expect(
      within(screen.getByRole("dialog")).getByRole("button", { name: "Primera" }),
    ).toHaveFocus();
  });

  // covers: UI, UXR-1.1
  it("cycles Tab in both directions, skipping hidden and disabled controls", async () => {
    const user = userEvent.setup();
    render(<Example />);
    await user.click(screen.getByRole("button", { name: "Abrir diálogo" }));
    const dialog = screen.getByRole("dialog");
    const first = within(dialog).getByRole("button", { name: "Primera" });
    const last = within(dialog).getByRole("button", { name: "Última" });
    await user.tab({ shift: true });
    expect(last).toHaveFocus();
    await user.tab();
    expect(first).toHaveFocus();
    await user.tab();
    expect(last).toHaveFocus();
  });

  // covers: UI, UXR-1.1
  it("recovers focus if a background element is focused programmatically", async () => {
    const user = userEvent.setup();
    render(<Example />);
    const background = screen.getByRole("button", { name: "Acción del fondo" });
    await user.click(screen.getByRole("button", { name: "Abrir diálogo" }));
    background.focus();
    expect(
      within(screen.getByRole("dialog")).getByRole("button", { name: "Primera" }),
    ).toHaveFocus();
  });

  // covers: UI, UXR-1.1
  it("keeps focus inside when the dialog has no enabled controls", async () => {
    const user = userEvent.setup();
    render(<Example disabledControls />);
    await user.click(screen.getByRole("button", { name: "Abrir diálogo" }));
    const heading = within(screen.getByRole("dialog")).getByRole("heading", { name: "Ejemplo" });
    expect(heading).toHaveFocus();
    await user.tab();
    expect(heading).toHaveFocus();
    await user.tab({ shift: true });
    expect(heading).toHaveFocus();
    await user.keyboard("{Escape}");
    expect(screen.getByRole("button", { name: "Abrir diálogo" })).toHaveFocus();
  });

  // covers: UI, UXR-1.1
  it("does not cancel while Escape belongs to IME composition", async () => {
    const user = userEvent.setup();
    render(<Example />);
    await user.click(screen.getByRole("button", { name: "Abrir diálogo" }));
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape", isComposing: true });
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  // covers: UI, UXR-1.1
  it("restores pre-existing background attributes without focusing a removed invoker", async () => {
    const user = userEvent.setup();
    const background = document.createElement("aside");
    background.setAttribute("inert", "");
    background.setAttribute("aria-hidden", "false");
    document.body.append(background);
    try {
      const { unmount } = render(<Example />);
      await user.click(screen.getByRole("button", { name: "Abrir diálogo" }));
      unmount();
      expect(background).toHaveAttribute("inert");
      expect(background).toHaveAttribute("aria-hidden", "false");
      expect(document.querySelector('[role="dialog"]')).toBeNull();
    } finally {
      background.remove();
    }
  });
});
