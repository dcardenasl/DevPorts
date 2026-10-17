import {
  useId,
  useLayoutEffect,
  useRef,
  type KeyboardEvent,
  type ReactElement,
  type ReactNode,
  type RefObject,
} from "react";
import { createPortal } from "react-dom";

const FOCUSABLE =
  'button, [href], input:not([type="hidden"]), select, textarea, [tabindex], [contenteditable="true"]';

function focusableWithin(element: HTMLElement): HTMLElement[] {
  return [...element.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((candidate) => {
    const style = getComputedStyle(candidate);
    if (
      candidate.tabIndex < 0 ||
      candidate.matches(':disabled, input[type="hidden"]') ||
      candidate.closest("[hidden], [inert]") !== null ||
      style.visibility === "hidden" ||
      style.visibility === "collapse"
    )
      return false;
    let ancestor: HTMLElement | null = candidate;
    while (ancestor !== null && ancestor !== element) {
      if (getComputedStyle(ancestor).display === "none") return false;
      ancestor = ancestor.parentElement;
    }
    return true;
  });
}

/** A panel-local modal. The portal leaves the application root inert until unmount. */
export function Modal({
  title,
  children,
  onCancel,
  returnFocusTo,
  initialFocusRef,
  role = "dialog",
  bottom = false,
  description,
  icon,
}: {
  title: string;
  children: ReactNode;
  onCancel: () => void;
  returnFocusTo: HTMLElement | null;
  initialFocusRef?: RefObject<HTMLElement | null>;
  role?: "dialog" | "alertdialog";
  bottom?: boolean;
  description?: string;
  icon?: ReactNode;
}): ReactElement {
  const titleId = useId();
  const descriptionId = useId();
  const overlayRef = useRef<HTMLDivElement>(null);
  const dialogRef = useRef<HTMLDivElement>(null);
  const titleRef = useRef<HTMLHeadingElement>(null);

  useLayoutEffect(() => {
    const overlay = overlayRef.current;
    const dialog = dialogRef.current;
    if (overlay === null || dialog === null) return;
    const invoker = returnFocusTo ?? document.activeElement;
    const background = [...document.body.children]
      .filter((element) => element !== overlay)
      .map((element) => ({
        element,
        inert: element.getAttribute("inert"),
        hidden: element.getAttribute("aria-hidden"),
      }));

    // Capture focus too: older WebKit versions do not implement `inert`.
    const containFocus = (event: FocusEvent): void => {
      if (event.target instanceof Node && !dialog.contains(event.target)) {
        (focusableWithin(dialog).at(0) ?? titleRef.current)?.focus();
      }
    };
    titleRef.current?.focus();
    document.addEventListener("focusin", containFocus);
    for (const { element } of background) {
      element.setAttribute("inert", "");
      element.setAttribute("aria-hidden", "true");
    }
    return () => {
      document.removeEventListener("focusin", containFocus);
      for (const { element, inert, hidden } of background) {
        if (inert === null) element.removeAttribute("inert");
        else element.setAttribute("inert", inert);
        if (hidden === null) element.removeAttribute("aria-hidden");
        else element.setAttribute("aria-hidden", hidden);
      }
      if (invoker instanceof HTMLElement && invoker.isConnected) invoker.focus();
    };
  }, [returnFocusTo]);

  // Switching steps can replace the controls, without tearing down the modal.
  useLayoutEffect(() => {
    (initialFocusRef?.current ?? titleRef.current)?.focus();
    if (!dialogRef.current?.contains(document.activeElement)) titleRef.current?.focus();
  }, [initialFocusRef]);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    if (event.key === "Escape" && !event.nativeEvent.isComposing) {
      event.preventDefault();
      event.stopPropagation();
      onCancel();
    } else if (event.key === "Tab" && dialogRef.current !== null) {
      const controls = focusableWithin(dialogRef.current);
      const first = controls.at(0);
      const last = controls.at(-1);
      const active = document.activeElement;
      if (!controls.some((control) => control === active)) {
        event.preventDefault();
        (event.shiftKey ? last : first)?.focus();
      } else if (event.shiftKey ? active === first : active === last) {
        event.preventDefault();
        (event.shiftKey ? last : first)?.focus();
      }
    }
  };

  return createPortal(
    <div className={bottom ? "veil bottom" : "veil"} ref={overlayRef}>
      <div
        className="sheet"
        ref={dialogRef}
        role={role}
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={description === undefined ? undefined : descriptionId}
        onKeyDown={onKeyDown}
      >
        <div className="row sheet-heading">
          {icon}
          <h2 id={titleId} className="name sheet-title" tabIndex={-1} ref={titleRef}>
            {title}
          </h2>
        </div>
        {description !== undefined && (
          <p id={descriptionId} className="soft sheet-description">
            {description}
          </p>
        )}
        {children}
      </div>
    </div>,
    document.body,
  );
}
