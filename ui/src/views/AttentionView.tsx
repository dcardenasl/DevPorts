import type { ReactElement } from "react";

import type { Actions } from "../actions";
import { buildAttention, whoAsked } from "../model";
import type { Catalog } from "../i18n/es";
import { useI18n } from "../i18n/provider";
import { ago, clock, countdown, secondsUntil } from "../time";
import type { InstanceView } from "../types";
import { InstanceDetails } from "./InstanceDetails";
import { instanceTitle, stateLabel, dotFor } from "../instance";
import { Dot } from "./shared-parts";

export function attentionSubtitle(count: number, text: Catalog): string {
  return count === 0 ? text.attention.allClear : text.attention.waiting(count);
}

export function AttentionView({ actions }: { actions: Actions }): ReactElement {
  const { text, locale } = useI18n();
  const { snapshot, now } = actions;
  const model = buildAttention(snapshot);
  const empty = snapshot.attention.length === 0;

  return (
    <>
      {empty ? (
        <div className="card green">
          <div className="row">
            <Dot kind="green" />
            <div>
              <div className="name">{text.attention.allClear}</div>
              <div className="soft">{text.attention.allClearBody}</div>
            </div>
          </div>
        </div>
      ) : (
        model.official.map((view) => <Official key={view.ref} view={view} now={now} />)
      )}

      {model.hung.map((view) => (
        <Hung key={view.ref} view={view} actions={actions} />
      ))}
      {model.expiring.map((view) => (
        <Expiring key={view.ref} view={view} actions={actions} />
      ))}
      {model.groups.map((group) => (
        <div className="card coral" key={`${group.project}/${group.service}`}>
          <div className="eyebrow coral-text">
            {text.attention.duplicates(group.duplicates.length, group.service)}
          </div>
          <div className="divided">
            {group.duplicates.map((view) => (
              <div key={view.ref}>
                <div className="proc">
                  <span className="row">
                    <Dot kind="coral" />
                    <span className="mono coral-text">:{view.port ?? "—"}</span>
                  </span>
                  <span className="ellipsis soft" title={instanceTitle(view, text)}>
                    {instanceTitle(view, text)}
                  </span>
                  <span className="dim">{ago(view.started_at, now, locale)}</span>
                </div>
                <InstanceDetails view={view} />
              </div>
            ))}
          </div>
          <div className="row" style={{ marginTop: 8 }}>
            <button
              type="button"
              className="btn primary-coral"
              onClick={(event) => {
                actions.requestStop(group.duplicates, event.currentTarget);
              }}
            >
              {group.duplicates.length === 1
                ? text.attention.stop
                : text.attention.stopAll(group.duplicates.length)}
            </button>
            <button
              type="button"
              className="btn"
              onClick={() => {
                for (const view of group.duplicates) actions.keep(view);
              }}
            >
              {text.attention.keep}
            </button>
          </div>
        </div>
      ))}
    </>
  );
}

function Official({ view, now }: { view: InstanceView; now: number }): ReactElement {
  const { text, locale } = useI18n();
  const branch = view.branch === null ? "" : ` · ${view.branch}`;
  return (
    <div
      className={
        view.state === "healthy" ? "card green" : view.state === "unhealthy" ? "card coral" : "card"
      }
    >
      <div className="row spread">
        <span className="row">
          <Dot kind={dotFor(view.state)} />
          <span className="name">{view.service}</span>
          <span className="soft">{view.project}</span>
        </span>
        <span className="eyebrow green-text">{text.attention.official}</span>
      </div>
      <div className="soft mono" style={{ marginTop: 6 }}>
        :{view.port}
        {branch} ·{" "}
        {text.attention.since(clock(view.started_at, locale) || ago(view.started_at, now, locale))}{" "}
        · {stateLabel(view.state, text)}
      </div>
      <InstanceDetails view={view} />
    </div>
  );
}

function Hung({ view, actions }: { view: InstanceView; actions: Actions }): ReactElement {
  const { text } = useI18n();
  return (
    <div className="card coral">
      <div className="eyebrow coral-text">{text.attention.hung}</div>
      <div className="row" style={{ marginTop: 6 }}>
        <Dot kind="coral" />
        <span className="name">{view.service}</span>
        <span className="soft">{view.project}</span>
      </div>
      <div className="soft mono ellipsis" style={{ marginTop: 4 }}>
        :{view.port} · {instanceTitle(view, text)}
      </div>
      <InstanceDetails view={view} />
      <div className="row" style={{ marginTop: 10 }}>
        <button
          type="button"
          className="btn primary-coral"
          onClick={() => {
            actions.showInProjects(view.ref);
          }}
        >
          {text.attention.seeInProjects}
        </button>
      </div>
    </div>
  );
}

function Expiring({ view, actions }: { view: InstanceView; actions: Actions }): ReactElement {
  const { text } = useI18n();
  const left = secondsUntil(view.expires_at, actions.now) ?? 0;
  const total = Math.max(
    1,
    secondsUntil(view.expires_at, Date.parse(view.started_at ?? "") || actions.now) ?? 1,
  );
  const ratio = Math.min(1, left / total);
  const who = whoAsked(view);
  return (
    <div className="card amber">
      <div className="row spread">
        <span className="eyebrow amber-text">{text.attention.expiring}</span>
        <span className="mono amber-text" style={{ fontSize: 16 }}>
          {countdown(left)}
        </span>
      </div>
      <div className="row" style={{ marginTop: 6 }}>
        <span className="name">{view.service}</span>
        <span className="soft">{view.reason}</span>
      </div>
      <div className="soft mono" style={{ marginTop: 4 }}>
        :{view.port}
        {who === null ? "" : ` · ${text.attention.askedBy(who)}`}
      </div>
      <InstanceDetails view={view} />
      <div className="bar" style={{ marginTop: 10 }} aria-hidden="true">
        <div style={{ width: `${Math.round(ratio * 100)}%` }} />
      </div>
      <div className="row" style={{ marginTop: 12 }}>
        <button
          type="button"
          className="btn primary-amber"
          onClick={() => {
            actions.extend(view);
          }}
        >
          {text.attention.extend(actions.snapshot.extend_minutes)}
        </button>
        <button
          type="button"
          className="btn"
          onClick={(event) => {
            actions.requestStop([view], event.currentTarget);
          }}
        >
          {text.attention.terminate}
        </button>
      </div>
    </div>
  );
}
