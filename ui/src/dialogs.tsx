import { useRef, type ReactElement } from "react";

import { Modal } from "./Modal";
import { WarningIcon } from "./icons";
import { text } from "./text";
import type { AdoptDraft, InstanceView, ProjectSummary } from "./types";

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

export type AdoptChoice =
  | { kind: "project"; project: string }
  | { kind: "new"; name: string | null }
  | { kind: "shared" }
  | { kind: "ignore" };

export function AdoptSheet({
  view,
  projects,
  draft,
  onCancel,
  onChoose,
  onDraftChange,
  returnFocusTo,
}: {
  view: InstanceView;
  projects: ProjectSummary[];
  draft: AdoptDraft;
  onCancel: () => void;
  onChoose: (choice: AdoptChoice) => void;
  onDraftChange: (draft: AdoptDraft) => void;
  returnFocusTo: HTMLElement | null;
}): ReactElement {
  const { mode, name, lastStep } = draft;
  const folder = view.cwd?.split("/").filter(Boolean).pop() ?? "";
  const suggested = view.project;
  const nameRef = useRef<HTMLInputElement>(null);
  const firstProjectRef = useRef<HTMLButtonElement>(null);
  const optionsFocusRef = useRef<HTMLButtonElement>(null);

  const enterStep = (step: "others" | "new"): void => {
    onDraftChange({ ...draft, lastStep: step, mode: step });
  };
  const goBack = (): void => {
    onDraftChange({ ...draft, mode: "options" });
  };

  return (
    <Modal
      title={text.orphans.sheetTitle}
      bottom
      onCancel={onCancel}
      returnFocusTo={returnFocusTo}
      initialFocusRef={
        mode === "new" ? nameRef : mode === "others" ? firstProjectRef : optionsFocusRef
      }
    >
      {mode === "options" && (
        <div className="stack">
          {suggested !== null && (
            <button
              type="button"
              className="option suggested"
              onClick={() => {
                onChoose({ kind: "project", project: suggested });
              }}
            >
              <span className="grow">
                <div>{text.orphans.asService(suggested)}</div>
              </span>
              <span className="tag green">{text.orphans.suggested}</span>
            </button>
          )}
          {projects.length > (suggested === null ? 0 : 1) && (
            <button
              type="button"
              className="option"
              ref={lastStep === "others" ? optionsFocusRef : undefined}
              onClick={() => {
                enterStep("others");
              }}
            >
              <span className="grow">
                <div>{text.orphans.another}</div>
                <div className="soft">{text.orphans.anotherBody}</div>
              </span>
            </button>
          )}
          {view.container !== null && (
            <button
              type="button"
              className="option"
              onClick={() => {
                onChoose({ kind: "shared" });
              }}
            >
              <span className="grow">
                <div>{text.orphans.shared}</div>
                <div className="soft">{text.orphans.sharedBody}</div>
              </span>
            </button>
          )}
          {view.container === null && (
            <button
              type="button"
              className="option"
              ref={lastStep === "new" ? optionsFocusRef : undefined}
              onClick={() => {
                enterStep("new");
              }}
            >
              <span className="grow">
                <div>{text.orphans.newProject}</div>
                <div className="soft ellipsis">
                  {text.orphans.newProjectBody(folder || "la carpeta")}
                </div>
              </span>
            </button>
          )}
          <button
            type="button"
            className="option"
            onClick={() => {
              onChoose({ kind: "ignore" });
            }}
          >
            <span className="grow">
              <div>{text.orphans.ignore}</div>
              <div className="soft">{text.orphans.ignoreBody}</div>
            </span>
          </button>
        </div>
      )}
      {mode === "others" && (
        <div className="stack">
          {projects.map((project, index) => (
            <button
              key={project.name}
              ref={index === 0 ? firstProjectRef : undefined}
              type="button"
              className="option"
              onClick={() => {
                onChoose({ kind: "project", project: project.name });
              }}
            >
              {text.orphans.asService(project.name)}
            </button>
          ))}
        </div>
      )}
      {mode === "new" && (
        <form
          className="stack"
          onSubmit={(event) => {
            event.preventDefault();
            onChoose({ kind: "new", name: name.trim() === "" ? null : name.trim() });
          }}
        >
          <label className="soft" htmlFor="project-name">
            {text.orphans.projectName}
          </label>
          <input
            id="project-name"
            className="input"
            value={name}
            ref={nameRef}
            onChange={(event) => {
              onDraftChange({ ...draft, name: event.target.value });
            }}
          />
          <div className="row" style={{ justifyContent: "flex-end" }}>
            <button type="submit" className="btn primary-green">
              {text.orphans.create}
            </button>
          </div>
        </form>
      )}
      <div className="row" style={{ justifyContent: "space-between", marginTop: 12 }}>
        {mode !== "options" ? (
          <button type="button" className="btn" onClick={goBack}>
            {text.orphans.back}
          </button>
        ) : (
          <span />
        )}
        <button type="button" className="link-btn" onClick={onCancel}>
          {text.orphans.cancel}
        </button>
      </div>
    </Modal>
  );
}
