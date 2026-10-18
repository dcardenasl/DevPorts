import type { ReactElement } from "react";

import type { Actions } from "../actions";
import { buildAttention, whoAsked } from "../model";
import type { Catalog } from "../i18n/es";
import { useI18n } from "../i18n/provider";
import { ago, clock, countdown, secondsUntil } from "../time";
import type { InstanceView } from "../types";
import { InstanceDetails } from "./InstanceDetails";
import { instanceTitle, stateLabel, dotFor } from "../instance";
import { Dot, Section } from "./shared-parts";

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
        <Section>
          <div className="item">
            <Dot kind="green" />
            <div className="item-main">
              <div className="item-title">{text.attention.allClear}</div>
              <div className="item-sub">{text.attention.allClearBody}</div>
            </div>
          </div>
        </Section>
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
        <Section
          key={`${group.project}/${group.service}`}
          title={text.attention.duplicates(group.duplicates.length, group.service)}
          tone="red"
        >
          {group.duplicates.map((view) => (
            <div key={view.ref}>
              <div className="item">
                <Dot kind="red" />
                <div className="item-main">
                  <div className="item-title mono">:{view.port ?? "—"}</div>
                  <div className="item-sub ellipsis" title={instanceTitle(view, text)}>
                    {instanceTitle(view, text)}
                  </div>
                </div>
                <span className="dim">{ago(view.started_at, now, locale)}</span>
              </div>
              <InstanceDetails view={view} />
            </div>
          ))}
          <div className="item-actions">
            <button
              type="button"
              className="btn danger"
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
        </Section>
      ))}
    </>
  );
}

function Official({ view, now }: { view: InstanceView; now: number }): ReactElement {
  const { text, locale } = useI18n();
  const branch = view.branch === null ? "" : ` · ${view.branch}`;
  return (
    <Section title={text.attention.official}>
      <div className="item">
        <Dot kind={dotFor(view.state)} />
        <div className="item-main">
          <div className="item-title">
            <span>{view.service}</span>
            <span className="soft">{view.project}</span>
          </div>
          <div className="item-sub mono">
            :{view.port}
            {branch} ·{" "}
            {text.attention.since(
              clock(view.started_at, locale) || ago(view.started_at, now, locale),
            )}{" "}
            · {stateLabel(view.state, text)}
          </div>
        </div>
      </div>
      <InstanceDetails view={view} />
    </Section>
  );
}

function Hung({ view, actions }: { view: InstanceView; actions: Actions }): ReactElement {
  const { text } = useI18n();
  return (
    <Section title={text.attention.hung} tone="red">
      <div className="item">
        <Dot kind="red" />
        <div className="item-main">
          <div className="item-title">
            <span>{view.service}</span>
            <span className="soft">{view.project}</span>
          </div>
          <div className="item-sub mono ellipsis">
            :{view.port} · {instanceTitle(view, text)}
          </div>
        </div>
      </div>
      <InstanceDetails view={view} />
      <div className="item-actions">
        <button
          type="button"
          className="btn primary"
          onClick={() => {
            actions.showInProjects(view.ref);
          }}
        >
          {text.attention.seeInProjects}
        </button>
      </div>
    </Section>
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
    <Section title={text.attention.expiring} tone="amber">
      <div className="item">
        <Dot kind="amber" />
        <div className="item-main">
          <div className="item-title">
            <span>{view.service}</span>
            <span className="soft">{view.reason}</span>
          </div>
          <div className="item-sub mono">
            :{view.port}
            {who === null ? "" : ` · ${text.attention.askedBy(who)}`}
          </div>
        </div>
        <span className="data is-amber">{countdown(left)}</span>
      </div>
      <InstanceDetails view={view} />
      <div className="item compact">
        <div className="bar grow" aria-hidden="true">
          <div style={{ width: `${Math.round(ratio * 100)}%` }} />
        </div>
      </div>
      <div className="item-actions">
        <button
          type="button"
          className="btn primary"
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
    </Section>
  );
}
