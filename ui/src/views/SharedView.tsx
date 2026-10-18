import type { ReactElement } from "react";

import type { Actions } from "../actions";
import type { SharedNode } from "../model";
import type { SharedSummary } from "../types";
import { StopIcon } from "../icons";
import { buildShared, buildUndeclaredData } from "../model";
import type { Catalog } from "../i18n/es";
import { useI18n } from "../i18n/provider";
import { ago } from "../time";
import { canRequestStop, stateLabel } from "../instance";
import { InstanceDetails } from "./InstanceDetails";
import { Dot, Empty, Tag, dotFor } from "./shared-parts";
import { IconButton } from "./ProjectsView";

export function sharedSubtitle(actions: Actions, text: Catalog): string {
  const declared = buildShared(actions.snapshot).length;
  const undeclared = buildUndeclaredData(actions.snapshot).length;
  return declared === 0 && undeclared > 0
    ? text.shared.onlyUndeclared
    : text.shared.summary(declared);
}

export function SharedView({ actions }: { actions: Actions }): ReactElement {
  const { text, locale } = useI18n();
  const nodes = buildShared(actions.snapshot);
  const undeclared = buildUndeclaredData(actions.snapshot);
  if (nodes.length === 0 && undeclared.length === 0) {
    return <Empty title={text.shared.none} body={text.shared.noneBody} />;
  }
  return (
    <>
      {nodes.map((node) => {
        const view = node.principal;
        const running = canRequestStop(view);
        const summary = actions.snapshot.shared.find((item) => item.name === node.name);
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
              {` · ${stateLabel(view.state, text)}`}
              {running ? ` · ${ago(view.started_at, actions.now, locale)}` : ""}
            </div>
            <InstanceDetails view={view} />
            <div className="row" style={{ marginTop: 8, flexWrap: "wrap" }}>
              <span className="dim" style={{ fontSize: 12 }}>
                {text.consumers.associated}
              </span>
              {node.users.length === 0 ? (
                <span className="dim" style={{ fontSize: 12 }}>
                  {text.consumers.nobody}
                </span>
              ) : (
                node.users.map((name) => <Tag key={name}>{name}</Tag>)
              )}
            </div>
            <div className="soft" style={{ marginTop: 4, fontSize: 11 }}>
              {text.consumers.declaredOnly}
            </div>
            <div className="row" style={{ marginTop: 8, justifyContent: "flex-end" }}>
              <ResourceButtons node={node} summary={summary} actions={actions} />
              <button
                type="button"
                className="btn"
                aria-label={text.consumers.manageFor(node.name)}
                onClick={(event) => {
                  actions.manageConsumers(node.name, event.currentTarget);
                }}
              >
                {text.consumers.manage}
              </button>
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
              <InstanceDetails view={view} />
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
                  onClick={(event) => {
                    actions.declareShared(view, event.currentTarget);
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

/**
 * What can be done with the resource itself. A start command or an address is
 * offered only when `shared.yml` declares it; nothing is derived from a port.
 */
function ResourceButtons({
  node,
  summary,
  actions,
}: {
  node: SharedNode;
  summary: SharedSummary | undefined;
  actions: Actions;
}): ReactElement | null {
  const { text } = useI18n();
  const state = node.principal.state;
  if (state === "down") {
    const start = summary?.start ?? null;
    return start === null ? (
      <span className="soft grow" style={{ fontSize: 11 }}>
        {text.consumers.noStart}
      </span>
    ) : (
      <button
        type="button"
        className="btn"
        onClick={() => {
          actions.copy(start);
        }}
      >
        {text.consumers.copyStart}
      </button>
    );
  }
  const endpoint = summary?.endpoint ?? null;
  return endpoint !== null && canRequestStop(node.principal) ? (
    <button
      type="button"
      className="btn"
      onClick={() => {
        actions.openUrl(endpoint);
      }}
    >
      {text.consumers.open}
    </button>
  ) : null;
}
