import { useRef, type ReactElement } from "react";

import { instanceTitle } from "./instance";
import { Modal } from "./Modal";
import { WarningIcon } from "./icons";
import { useI18n } from "./i18n/provider";
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
  const { text } = useI18n();
  const cancelRef = useRef<HTMLButtonElement>(null);
  const first = views[0];
  const name = first === undefined ? "" : instanceTitle(first, text);
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
        <span className="is-red">
          <WarningIcon size={22} />
        </span>
      }
    >
      <div className="stack stop-targets">
        {views.map((view) => {
          const targetName = instanceTitle(view, text);
          const consumers = affectedNames.size > 0 && (view.shared || view.kind === "data");
          return (
            <section className="well" key={view.ref}>
              <div className="name">{targetName}</div>
              <div className="soft">
                {text.confirm.identity(view)} · {text.confirm.port(view.port)}
              </div>
              <div className="soft">{text.confirm.origin(view)}</div>
              {consumers && (
                <div className="affected">
                  <div className="label is-red">{text.confirm.affects}</div>
                  <ul>
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
      <div className="row end sheet-actions">
        <button type="button" className="btn" onClick={onCancel} ref={cancelRef}>
          {text.confirm.cancel}
        </button>
        <button type="button" className="btn danger-fill" onClick={onConfirm}>
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
