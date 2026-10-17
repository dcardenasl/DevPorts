import { useState, type ReactElement } from "react";

import { WarningIcon } from "./icons";
import { text } from "./text";
import type { InstanceView, ProjectSummary } from "./types";

export function ConfirmStop({
  view,
  affected,
  onCancel,
  onConfirm,
}: {
  view: InstanceView;
  affected: string[];
  onCancel: () => void;
  onConfirm: () => void;
}): ReactElement {
  const name = view.service ?? view.container ?? view.ref;
  return (
    <div className="veil" role="presentation">
      <div
        className="sheet"
        role="alertdialog"
        aria-modal="true"
        aria-label={text.confirm.stopTitle(name)}
      >
        <div className="row">
          <span className="coral-text">
            <WarningIcon size={22} />
          </span>
          <span className="name">{text.confirm.stopTitle(name)}</span>
        </div>
        <p className="soft" style={{ margin: "8px 0" }}>
          {text.confirm.stopBody}
        </p>
        {affected.length > 0 && (
          <div className="card coral" style={{ margin: "8px 0" }}>
            <div className="eyebrow coral-text">{text.confirm.affects}</div>
            <ul style={{ margin: "6px 0 0", paddingLeft: 18 }}>
              {affected.map((entry) => (
                <li key={entry}>{entry}</li>
              ))}
            </ul>
          </div>
        )}
        <div className="row" style={{ justifyContent: "flex-end", marginTop: 12 }}>
          <button type="button" className="btn" onClick={onCancel} autoFocus>
            {text.confirm.cancel}
          </button>
          <button type="button" className="btn primary-coral" onClick={onConfirm}>
            {text.confirm.stop(name)}
          </button>
        </div>
      </div>
    </div>
  );
}

export type AdoptChoice =
  { kind: "project"; project: string } | { kind: "new"; name: string | null } | { kind: "ignore" };

export function AdoptSheet({
  view,
  projects,
  onCancel,
  onChoose,
}: {
  view: InstanceView;
  projects: ProjectSummary[];
  onCancel: () => void;
  onChoose: (choice: AdoptChoice) => void;
}): ReactElement {
  const [mode, setMode] = useState<"options" | "others" | "new">("options");
  const folder = view.cwd?.split("/").filter(Boolean).pop() ?? "";
  const [name, setName] = useState(folder);
  const suggested = view.project;

  return (
    <div className="veil bottom" role="presentation">
      <div className="sheet" role="dialog" aria-modal="true" aria-label={text.orphans.sheetTitle}>
        <div className="name" style={{ marginBottom: 10 }}>
          {text.orphans.sheetTitle}
        </div>
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
                onClick={() => {
                  setMode("others");
                }}
              >
                <span className="grow">
                  <div>{text.orphans.another}</div>
                  <div className="soft">{text.orphans.anotherBody}</div>
                </span>
              </button>
            )}
            <button
              type="button"
              className="option"
              onClick={() => {
                setMode("new");
              }}
            >
              <span className="grow">
                <div>{text.orphans.newProject}</div>
                <div className="soft ellipsis">
                  {text.orphans.newProjectBody(folder || "la carpeta")}
                </div>
              </span>
            </button>
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
            {projects.map((project) => (
              <button
                key={project.name}
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
              autoFocus
              onChange={(event) => {
                setName(event.target.value);
              }}
            />
            <div className="row" style={{ justifyContent: "flex-end" }}>
              <button type="submit" className="btn primary-green">
                {text.orphans.create}
              </button>
            </div>
          </form>
        )}
        <div className="row" style={{ justifyContent: "flex-end", marginTop: 12 }}>
          <button type="button" className="link-btn" onClick={onCancel}>
            {text.orphans.cancel}
          </button>
        </div>
      </div>
    </div>
  );
}
