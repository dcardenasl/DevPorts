import type { ReactElement } from "react";

import type { Actions } from "../actions";
import { StopIcon } from "../icons";
import { buildShared, buildUndeclaredData } from "../model";
import { text } from "../text";
import { ago } from "../time";
import { Dot, Empty, Tag, dotFor } from "./shared-parts";
import { IconButton } from "./ProjectsView";

export function sharedSubtitle(actions: Actions): string {
  const declared = buildShared(actions.snapshot).length;
  const undeclared = buildUndeclaredData(actions.snapshot).length;
  return declared === 0 && undeclared > 0
    ? text.shared.onlyUndeclared
    : text.shared.summary(declared);
}

export function SharedView({ actions }: { actions: Actions }): ReactElement {
  const nodes = buildShared(actions.snapshot);
  const undeclared = buildUndeclaredData(actions.snapshot);
  if (nodes.length === 0 && undeclared.length === 0) {
    return <Empty title={text.shared.none} body={text.shared.noneBody} />;
  }
  return (
    <>
      {nodes.map((node) => {
        const view = node.principal;
        const running = view.state !== "down" && view.ref !== "-";
        return (
          <section className="card" key={node.name}>
            <div className="row">
              <Dot kind={dotFor(view.state)} />
              <span className="name">{node.name}</span>
              {view.kind === "data" && <Tag tone="blue">{text.shared.data}</Tag>}
              <span className="grow" />
              <span className="mono soft">{view.port === null ? "" : `:${view.port}`}</span>
              {running && (
                <IconButton
                  label={`${text.projects.stop} ${node.name}`}
                  danger
                  onClick={(event) => {
                    actions.requestStop([view], event.currentTarget);
                  }}
                >
                  <StopIcon size={14} />
                </IconButton>
              )}
            </div>
            <div className="soft mono" style={{ marginTop: 6, fontSize: 11 }}>
              {text.shared.docker} · {view.container ?? view.image ?? "—"}
              {running
                ? ` · ${ago(view.started_at, actions.now)}`
                : ` · ${text.projects.down.toLowerCase()}`}
            </div>
            <div className="row" style={{ marginTop: 8, flexWrap: "wrap" }}>
              <span className="dim" style={{ fontSize: 12 }}>
                {text.shared.usedBy}
              </span>
              {node.users.length === 0 ? (
                <span className="dim" style={{ fontSize: 12 }}>
                  {text.shared.nobody}
                </span>
              ) : (
                node.users.map((name) => <Tag key={name}>{name}</Tag>)
              )}
            </div>
          </section>
        );
      })}
      {undeclared.length > 0 && (
        <>
          <div className="eyebrow dim" style={{ marginTop: 4 }}>
            {text.shared.undeclared}
          </div>
          <div className="soft" style={{ fontSize: 12 }}>
            {text.shared.undeclaredBody}
          </div>
          {undeclared.map((view) => (
            <section className="card" key={view.ref}>
              <div className="row">
                <Dot kind="loose" />
                <span className="name">{view.container}</span>
                <Tag tone="blue">{text.shared.data}</Tag>
                <span className="grow" />
                <span className="mono soft">{view.port === null ? "" : `:${view.port}`}</span>
              </div>
              <div className="soft mono" style={{ marginTop: 6, fontSize: 11 }}>
                {text.shared.docker} · {view.image ?? "—"}
                {view.compose === null ? "" : ` · ${view.compose}`}
              </div>
              <div className="row" style={{ marginTop: 10, justifyContent: "flex-end" }}>
                <button
                  type="button"
                  className="btn"
                  onClick={(event) => {
                    actions.requestStop([view], event.currentTarget);
                  }}
                >
                  {text.projects.stop}
                </button>
                <button
                  type="button"
                  className="btn primary-green"
                  onClick={() => {
                    actions.declareShared(view);
                  }}
                >
                  {text.shared.declare}
                </button>
              </div>
            </section>
          ))}
        </>
      )}
    </>
  );
}
