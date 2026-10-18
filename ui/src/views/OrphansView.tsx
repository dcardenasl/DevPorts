import { useState, type ReactElement } from "react";

import type { Actions } from "../actions";
import { buildOrphans, groupOrphans as groupByClass } from "../model";
import type { Catalog } from "../i18n/es";
import { useI18n } from "../i18n/provider";
import { ago } from "../time";
import type { Classification, InstanceView } from "../types";
import { InstanceDetails } from "./InstanceDetails";
import { instanceTitle } from "../instance";
import { Dot, Empty, Section, Tag } from "./shared-parts";

export function orphansSubtitle(actions: Actions, text: Catalog): string {
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
function groupByRepo(orphans: InstanceView[], text: Catalog): Group[] {
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
  const { text } = useI18n();
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
  const sectioned = groups.unclassified.length + groups.auxiliary.length > 0;
  return (
    <>
      {groups.project.length > 0 && (
        <>
          {sectioned && (
            <h2 className="section-title">
              {text.classify.groupProject} · {groups.project.length}
            </h2>
          )}
          {groupByRepo(groups.project, text).map((group) => (
            <Section
              key={group.key}
              title={`${group.title} · ${text.orphans.group(group.views.length)}`}
            >
              {group.views.map(card)}
            </Section>
          ))}
        </>
      )}
      {groups.unclassified.length > 0 && (
        <section className="section">
          <h2 className="section-title">
            {text.classify.groupUnclassified} · {groups.unclassified.length}
          </h2>
          <p className="section-note">{text.classify.unclassifiedBody}</p>
          <div className="group">{groups.unclassified.map(card)}</div>
        </section>
      )}
      {groups.auxiliary.length > 0 && (
        <section className="section">
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
          <div id="auxiliary-group" className="section" hidden={!auxiliaryOpen}>
            {auxiliaryOpen && (
              <>
                <p className="section-note">{text.classify.auxiliaryBody}</p>
                <div className="group">{groups.auxiliary.map(card)}</div>
              </>
            )}
          </div>
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
  const { text, locale } = useI18n();
  const notes = [
    view.project !== null ? text.orphans.seems(view.project) : undefined,
    classification?.origin ?? undefined,
    classification?.program && classification.class === "auxiliary"
      ? classification.program
      : undefined,
    classification?.corrected === true ? text.classify.corrected : undefined,
  ].filter((note): note is string => note !== undefined);
  return (
    <div>
      <div className="item">
        <Dot kind="loose" />
        <div className="item-main">
          <div className="item-title mono ellipsis" title={instanceTitle(view, text)}>
            {instanceTitle(view, text)}
          </div>
          <div className="item-sub mono ellipsis" title={view.cwd ?? ""}>
            {view.cwd ?? view.image ?? "—"} · {ago(view.started_at, actions.now, locale)}
          </div>
        </div>
        <span className="data soft">{view.port === null ? "" : `:${view.port}`}</span>
      </div>
      <InstanceDetails view={view} />
      {notes.length > 0 && (
        <div className="item-note row wrap">
          {notes.map((note) => (
            <Tag key={note}>{note}</Tag>
          ))}
        </div>
      )}
      <div className="item-actions">
        <button
          type="button"
          className="link-btn"
          onClick={(event) => {
            actions.classify(view, event.currentTarget);
          }}
        >
          {text.classify.action}
        </button>
        <span className="grow" />
        <button
          type="button"
          className="btn danger"
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
          className={view.project !== null ? "btn primary" : "btn"}
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
