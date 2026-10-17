import type { ReactElement } from "react";

import type { Actions } from "../actions";
import { buildOrphans } from "../model";
import { text } from "../text";
import { ago } from "../time";
import { FolderIcon } from "../icons";
import type { InstanceView } from "../types";
import { Dot, Empty, Tag, commandLine } from "./shared-parts";

export function orphansSubtitle(actions: Actions): string {
  return text.orphans.summary(buildOrphans(actions.snapshot).length);
}

interface Group {
  key: string;
  title: string;
  branch: string | null;
  views: InstanceView[];
}

/** Loose processes grouped by the repository (or compose project) they came from. */
function groupOrphans(orphans: InstanceView[]): Group[] {
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
  const orphans = buildOrphans(actions.snapshot);
  if (orphans.length === 0) {
    return <Empty title={text.orphans.none} body={text.orphans.noneBody} />;
  }
  return (
    <>
      {groupOrphans(orphans).map((group) => (
        <section key={group.key} className="stack">
          <div className="row" style={{ padding: "2px 2px 0" }}>
            <FolderIcon size={14} />
            <span className="mono grow ellipsis">{group.title}</span>
            {group.branch !== null && <Tag>{group.branch}</Tag>}
            <span className="dim" style={{ fontSize: 12 }}>
              {text.orphans.group(group.views.length)}
            </span>
          </div>
          {group.views.map((view) => (
            <OrphanCard key={view.ref} view={view} actions={actions} />
          ))}
        </section>
      ))}
    </>
  );
}

function OrphanCard({ view, actions }: { view: InstanceView; actions: Actions }): ReactElement {
  return (
    <div className="card">
      <div className="row">
        <Dot kind="loose" />
        <span className="mono grow ellipsis" title={commandLine(view.argv, view.container)}>
          {commandLine(view.argv, view.container)}
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
      <div className="row" style={{ marginTop: 10 }}>
        {view.project !== null && <Tag tone="green">{text.orphans.seems(view.project)}</Tag>}
        <span className="grow" />
        <button
          type="button"
          className={view.project !== null ? "btn primary-green" : "btn"}
          onClick={() => {
            actions.adopt(view);
          }}
        >
          {text.orphans.adopt}
        </button>
      </div>
    </div>
  );
}
