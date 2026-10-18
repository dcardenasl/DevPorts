import { useState, type ReactElement } from "react";

import type { Actions } from "../actions";
import { buildOrphans, groupOrphans as groupByClass } from "../model";
import { text } from "../text";
import { ago } from "../time";
import { FolderIcon } from "../icons";
import type { Classification, InstanceView } from "../types";
import { InstanceDetails } from "./InstanceDetails";
import { instanceTitle } from "../instance";
import { Dot, Empty, Tag } from "./shared-parts";

export function orphansSubtitle(actions: Actions): string {
  const total = text.orphans.summary(buildOrphans(actions.snapshot).length);
  const auxiliary = groupByClass(actions.snapshot).auxiliary.length;
  return auxiliary === 0 ? total : `${total} · ${text.classify.auxiliaries(auxiliary)}`;
}

interface Group {
  key: string;
  title: string;
  branch: string | null;
  views: InstanceView[];
}

/** Loose processes grouped by the repository (or compose project) they came from. */
function groupByRepo(orphans: InstanceView[]): Group[] {
  const groups = new Map<string, Group>();
  for (const view of orphans) {
    const place = view.worktree ?? view.compose;
    const key = place ?? "";
    const title =
      place === null ? text.orphans.otherGroup : (place.split("/").filter(Boolean).pop() ?? place);
    const group = groups.get(key) ?? { key, title, branch: view.branch, views: [] };
    group.views.push(view);
    groups.set(key, group);
  }
  return [...groups.values()].sort(
    (a, b) => Number(a.key === "") - Number(b.key === "") || b.views.length - a.views.length,
  );
}

export function OrphansView({ actions }: { actions: Actions }): ReactElement {
  const [auxiliaryOpen, setAuxiliaryOpen] = useState(false);
  const groups = groupByClass(actions.snapshot);
  const byRef = new Map(actions.snapshot.classifications.map((item) => [item.instanceRef, item]));
  const ignored = (
    <button
      type="button"
      className="link-btn"
      onClick={(event) => {
        actions.showIgnored(event.currentTarget);
      }}
    >
      {text.ignore.view}
    </button>
  );
  const total = groups.project.length + groups.unclassified.length + groups.auxiliary.length;
  if (total === 0) {
    return (
      <>
        <Empty title={text.orphans.none} body={text.orphans.noneBody} />
        {ignored}
      </>
    );
  }
  const card = (view: InstanceView): ReactElement => (
    <OrphanCard key={view.ref} view={view} classification={byRef.get(view.ref)} actions={actions} />
  );
  const heading = (title: string, count: number): ReactElement => (
    <div className="eyebrow dim" style={{ marginTop: 4 }}>
      {title} · {count}
    </div>
  );
  const sectioned = groups.unclassified.length + groups.auxiliary.length > 0;
  return (
    <>
      {groups.project.length > 0 && (
        <>
          {sectioned && heading(text.classify.groupProject, groups.project.length)}
          {groupByRepo(groups.project).map((group) => (
            <section key={group.key} className="stack">
              <div className="row" style={{ padding: "2px 2px 0" }}>
                <FolderIcon size={14} />
                <span className="mono grow ellipsis">{group.title}</span>
                {group.branch !== null && <Tag>{group.branch}</Tag>}
                <span className="dim" style={{ fontSize: 12 }}>
                  {text.orphans.group(group.views.length)}
                </span>
              </div>
              {group.views.map(card)}
            </section>
          ))}
        </>
      )}
      {groups.unclassified.length > 0 && (
        <section className="stack">
          {heading(text.classify.groupUnclassified, groups.unclassified.length)}
          <div className="soft" style={{ fontSize: 12 }}>
            {text.classify.unclassifiedBody}
          </div>
          {groups.unclassified.map(card)}
        </section>
      )}
      {groups.auxiliary.length > 0 && (
        <section className="stack">
          <button
            type="button"
            className="disclosure"
            aria-expanded={auxiliaryOpen}
            aria-controls="auxiliary-group"
            onClick={() => {
              setAuxiliaryOpen((open) => !open);
            }}
          >
            {text.classify.groupAuxiliary} · {groups.auxiliary.length}
          </button>
          {auxiliaryOpen && (
            <div id="auxiliary-group" className="stack">
              <div className="soft" style={{ fontSize: 12 }}>
                {text.classify.auxiliaryBody}
              </div>
              {groups.auxiliary.map(card)}
            </div>
          )}
        </section>
      )}
      {ignored}
    </>
  );
}

function OrphanCard({
  view,
  classification,
  actions,
}: {
  view: InstanceView;
  classification: Classification | undefined;
  actions: Actions;
}): ReactElement {
  return (
    <div className="card">
      <div className="row">
        <Dot kind="loose" />
        <span className="mono grow ellipsis" title={instanceTitle(view)}>
          {instanceTitle(view)}
        </span>
        <span className="mono soft">{view.port === null ? "" : `:${view.port}`}</span>
      </div>
      <div
        className="dim mono ellipsis"
        style={{ marginTop: 6, fontSize: 11 }}
        title={view.cwd ?? ""}
      >
        {view.cwd ?? view.image ?? "—"} · {ago(view.started_at, actions.now)}
      </div>
      <InstanceDetails view={view} />
      <div className="row" style={{ marginTop: 6 }}>
        <button
          type="button"
          className="link-btn"
          onClick={(event) => {
            actions.classify(view, event.currentTarget);
          }}
        >
          {text.classify.action}
        </button>
      </div>
      <div className="row" style={{ marginTop: 10 }}>
        {view.project !== null && <Tag tone="green">{text.orphans.seems(view.project)}</Tag>}
        {classification?.origin && <Tag tone="blue">{classification.origin}</Tag>}
        {classification?.program && classification.class === "auxiliary" && (
          <Tag>{classification.program}</Tag>
        )}
        {classification?.corrected === true && <Tag tone="amber">{text.classify.corrected}</Tag>}
        <span className="grow" />
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
          className="btn"
          onClick={(event) => {
            actions.ignore(view, event.currentTarget);
          }}
        >
          {text.ignore.action}
        </button>
        <button
          type="button"
          className={view.project !== null ? "btn primary-green" : "btn"}
          onClick={(event) => {
            actions.adopt(view, event.currentTarget);
          }}
        >
          {text.orphans.adopt}
        </button>
      </div>
    </div>
  );
}
