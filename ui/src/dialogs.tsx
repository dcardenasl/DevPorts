import { useRef, type ReactElement } from "react";

import { Modal } from "./Modal";
import { WarningIcon } from "./icons";
import { text } from "./text";
import type { InstanceView } from "./types";

export function ConfirmStop({
  views,
  project,
  affected,
  onCancel,
  onConfirm,
  returnFocusTo,
}: {
  views: InstanceView[];
  project?: string | undefined;
  affected: string[];
  onCancel: () => void;
  onConfirm: () => void;
  returnFocusTo: HTMLElement | null;
}): ReactElement {
  const cancelRef = useRef<HTMLButtonElement>(null);
  const name = views[0]?.service ?? views[0]?.container ?? views[0]?.ref ?? "";
  const data = views.some((view) => view.kind === "data");
  const affectedNames = new Set(affected);
  return (
    <Modal
      title={
        project !== undefined
          ? text.confirm.stopProjectTitle(project)
          : views.length === 1
            ? text.confirm.stopTitle(name)
            : text.confirm.stopGroupTitle(views.length)
      }
      description={
        project !== undefined
          ? text.confirm.projectStopBody
          : data
            ? text.confirm.dataStopBody
            : text.confirm.stopBody
      }
      role="alertdialog"
      onCancel={onCancel}
      returnFocusTo={returnFocusTo}
      initialFocusRef={cancelRef}
      icon={
        <span className="coral-text">
          <WarningIcon size={22} />
        </span>
      }
    >
      <div className="stack stop-targets" style={{ marginTop: 8 }}>
        {views.map((view) => {
          const targetName = view.service ?? view.container ?? view.ref;
          const consumers = affectedNames.size > 0 && (view.shared || view.kind === "data");
          return (
            <section className="card coral" key={view.ref}>
              <div className="name">{targetName}</div>
              <div className="soft">
                {text.confirm.identity(view)} · {text.confirm.port(view.port)}
              </div>
              <div className="soft">{text.confirm.origin(view)}</div>
              {consumers && (
                <div style={{ marginTop: 6 }}>
                  <div className="eyebrow coral-text">{text.confirm.affects}</div>
                  <ul style={{ margin: "4px 0 0", paddingLeft: 18 }}>
                    {affected.map((entry) => (
                      <li key={entry}>{entry}</li>
                    ))}
                  </ul>
                </div>
              )}
            </section>
          );
        })}
      </div>
      <div className="row" style={{ justifyContent: "flex-end", marginTop: 12 }}>
        <button type="button" className="btn" onClick={onCancel} ref={cancelRef}>
          {text.confirm.cancel}
        </button>
        <button type="button" className="btn primary-coral" onClick={onConfirm}>
          {project !== undefined
            ? text.confirm.stopGroup
            : views.length === 1
              ? text.confirm.stop(name)
              : text.confirm.stopGroup}
        </button>
      </div>
    </Modal>
  );
}
