import type { ReactElement } from "react";

import type { Actions } from "../actions";
import { buildOrphans } from "../model";
import { text } from "../text";
import { ago } from "../time";
import { Dot, Empty, Tag, commandLine } from "./shared-parts";

export function orphansSubtitle(actions: Actions): string {
  return text.orphans.summary(buildOrphans(actions.snapshot).length);
}

export function OrphansView({ actions }: { actions: Actions }): ReactElement {
  const orphans = buildOrphans(actions.snapshot);
  if (orphans.length === 0) {
    return <Empty title={text.orphans.none} body={text.orphans.noneBody} />;
  }
  return (
    <>
      {orphans.map((view) => (
        <section className="card" key={view.ref}>
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
        </section>
      ))}
    </>
  );
}
