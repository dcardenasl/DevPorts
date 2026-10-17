import { useCallback, useMemo, useState, type ReactElement } from "react";

import type { Actions } from "./actions";
import { AdoptSheet, ConfirmStop, type AdoptChoice } from "./dialogs";
import { useConnection, useNow, useToast } from "./hooks";
import {
  BellIcon,
  CheckIcon,
  DatabaseIcon,
  FolderIcon,
  ListIcon,
  OrphanIcon,
  WarningIcon,
} from "./icons";
import { buildOrphans, dependentsOf } from "./model";
import { text } from "./text";
import type { InstanceView } from "./types";
import { ActivityView, activitySubtitle } from "./views/ActivityView";
import { AttentionView, attentionSubtitle } from "./views/AttentionView";
import { OrphansView, orphansSubtitle } from "./views/OrphansView";
import { ProjectsView, projectsSubtitle } from "./views/ProjectsView";
import { SharedView, sharedSubtitle } from "./views/SharedView";

type Tab = "attention" | "projects" | "shared" | "orphans" | "activity";

const TABS: { id: Tab; icon: (props: { size?: number }) => ReactElement }[] = [
  { id: "attention", icon: BellIcon },
  { id: "projects", icon: FolderIcon },
  { id: "shared", icon: DatabaseIcon },
  { id: "orphans", icon: OrphanIcon },
  { id: "activity", icon: ListIcon },
];

export function App(): ReactElement {
  const { api, snapshot, error } = useConnection();
  const now = useNow();
  const [tab, setTab] = useState<Tab>("attention");
  const [highlight, setHighlight] = useState<string>();
  const [confirming, setConfirming] = useState<InstanceView>();
  const [adopting, setAdopting] = useState<InstanceView>();
  const [logCount, setLogCount] = useState<number>();
  const [toast, showToast] = useToast();
  const [failed, setFailed] = useState(false);

  const attempt = useCallback(
    (work: Promise<unknown>, success?: string): void => {
      work.then(
        () => {
          setFailed(false);
          if (success !== undefined) showToast(success);
        },
        (failure: unknown) => {
          setFailed(true);
          showToast(failure instanceof Error ? failure.message : String(failure));
        },
      );
    },
    [showToast],
  );

  const actions = useMemo<Actions | undefined>(() => {
    if (api === undefined || snapshot === undefined) return undefined;
    return {
      snapshot,
      now,
      requestStop: setConfirming,
      stopNow: (views) => {
        for (const view of views) attempt(api.stop(view.ref));
      },
      extend: (view) => {
        attempt(api.extend(view.ref));
      },
      keep: (view) => {
        attempt(api.keep(view.ref));
      },
      openUrl: (url) => {
        attempt(api.openUrl(url));
      },
      copy: (value) => {
        attempt(copyText(value), text.projects.copied);
      },
      showInProjects: (ref) => {
        setHighlight(ref);
        setTab("projects");
      },
      adopt: setAdopting,
    };
  }, [api, snapshot, now, attempt]);

  if (error !== undefined) {
    return (
      <Shell
        title="DevPorts"
        subtitle={text.status.failed}
        body={<p className="empty">{error}</p>}
      />
    );
  }
  if (api === undefined || snapshot === undefined || actions === undefined) {
    return <Shell title="DevPorts" subtitle={text.status.connecting} body={null} />;
  }

  const orphanCount = buildOrphans(snapshot).length;
  const subtitle: Record<Tab, string> = {
    attention: attentionSubtitle(snapshot.attention.length),
    projects: projectsSubtitle(actions),
    shared: sharedSubtitle(actions),
    orphans: orphansSubtitle(actions),
    activity: activitySubtitle(logCount),
  };

  const body = (
    <>
      {tab === "attention" && (
        <>
          <AttentionView actions={actions} />
          {snapshot.problems.length > 0 && (
            <div className="card blue">
              <div className="eyebrow blue-text">{text.status.problems}</div>
              <ul className="soft" style={{ margin: "6px 0 0", paddingLeft: 18 }}>
                {snapshot.problems.map((problem) => (
                  <li key={problem}>{problem}</li>
                ))}
              </ul>
            </div>
          )}
        </>
      )}
      {tab === "projects" && <ProjectsView actions={actions} highlight={highlight} />}
      {tab === "shared" && (
        <>
          {!snapshot.docker_reachable && snapshot.docker_error !== null && (
            <div className="card blue">
              <div className="eyebrow blue-text">{text.status.dockerDown}</div>
              <div className="soft">{text.status.dockerDownBody}</div>
            </div>
          )}
          <SharedView actions={actions} />
        </>
      )}
      {tab === "orphans" && <OrphansView actions={actions} />}
      {tab === "activity" && (
        <ActivityView api={api} revision={snapshot.revision} onCount={setLogCount} />
      )}
    </>
  );

  const confirmed = confirming;
  return (
    <Shell
      title={text.tabs[tab]}
      subtitle={subtitle[tab]}
      body={body}
      tabKey={tab}
      footer={
        <nav className="tabs" role="tablist" aria-label="Secciones">
          {TABS.map(({ id, icon: Icon }) => {
            const count =
              id === "attention" ? snapshot.attention.length : id === "orphans" ? orphanCount : 0;
            return (
              <button
                key={id}
                type="button"
                role="tab"
                className="tab"
                aria-selected={tab === id}
                onClick={() => {
                  setTab(id);
                }}
              >
                <Icon size={18} />
                <span>{text.tabs[id]}</span>
                {count > 0 && (
                  <span className="badge" aria-label={`${count} pendientes`}>
                    {count}
                  </span>
                )}
              </button>
            );
          })}
        </nav>
      }
      overlay={
        <>
          {confirmed !== undefined && (
            <ConfirmStop
              view={confirmed}
              affected={dependentsOf(confirmed, snapshot)}
              onCancel={() => {
                setConfirming(undefined);
              }}
              onConfirm={() => {
                setConfirming(undefined);
                attempt(api.stop(confirmed.ref));
              }}
            />
          )}
          {adopting !== undefined && (
            <AdoptSheet
              view={adopting}
              projects={snapshot.projects}
              onCancel={() => {
                setAdopting(undefined);
              }}
              onChoose={(choice) => {
                const view = adopting;
                setAdopting(undefined);
                attempt(adoptWith(api, view, choice), "Listo");
              }}
            />
          )}
          {toast !== undefined && (
            <div className={failed ? "toast error" : "toast"} role="status">
              {failed ? <WarningIcon size={14} /> : <CheckIcon size={14} />}
              <span>{toast}</span>
            </div>
          )}
        </>
      }
    />
  );
}

function Shell({
  title,
  subtitle,
  body,
  footer,
  overlay,
  tabKey,
}: {
  title: string;
  subtitle: string;
  body: ReactElement | null;
  footer?: ReactElement;
  overlay?: ReactElement;
  tabKey?: string;
}): ReactElement {
  return (
    <div className="panel">
      <header className="head">
        <h1>{title}</h1>
        <p>{subtitle}</p>
      </header>
      <main className="body" key={tabKey} role="tabpanel">
        {body}
      </main>
      {footer ?? <div />}
      {overlay}
    </div>
  );
}

async function adoptWith(
  api: NonNullable<ReturnType<typeof useConnection>["api"]>,
  view: InstanceView,
  choice: AdoptChoice,
): Promise<void> {
  switch (choice.kind) {
    case "project":
      await api.adopt(view.ref, { project: choice.project });
      return;
    case "new":
      await api.adopt(view.ref, { new: choice.name });
      return;
    case "shared":
      await api.adopt(view.ref, "shared");
      return;
    case "ignore":
      await api.keep(view.ref);
  }
}

async function copyText(value: string): Promise<void> {
  await navigator.clipboard.writeText(value);
}
