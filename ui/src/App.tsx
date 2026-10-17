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
import type { AdoptDraft, InstanceView, PanelState, PanelTab } from "./types";
import { ActivityView, activitySubtitle } from "./views/ActivityView";
import { AttentionView, attentionSubtitle } from "./views/AttentionView";
import { OrphansView, orphansSubtitle } from "./views/OrphansView";
import { ProjectsView, projectsSubtitle } from "./views/ProjectsView";
import { SharedView, sharedSubtitle } from "./views/SharedView";

type Tab = "attention" | "projects" | "shared" | "orphans" | "activity";
type DialogRequest = { kind: "stop"; view: InstanceView; invoker: HTMLButtonElement | null };

const TABS: { id: Tab; icon: (props: { size?: number }) => ReactElement }[] = [
  { id: "attention", icon: BellIcon },
  { id: "projects", icon: FolderIcon },
  { id: "shared", icon: DatabaseIcon },
  { id: "orphans", icon: OrphanIcon },
  { id: "activity", icon: ListIcon },
];

function initialTab(): Tab {
  const wanted = new URLSearchParams(window.location.search).get("tab");
  return TABS.find((entry) => entry.id === wanted)?.id ?? "attention";
}

export function App(): ReactElement {
  const { api, snapshot, error, panelState: hydratedState, missingDraft } = useConnection();
  const now = useNow();
  const [panelState, setPanelState] = useState<PanelState | undefined>(hydratedState);
  const [previousHydratedState, setPreviousHydratedState] = useState(hydratedState);
  if (hydratedState !== previousHydratedState) {
    setPreviousHydratedState(hydratedState);
    setPanelState(hydratedState);
  }
  const tab = panelState?.tab ?? initialTab();
  const [highlight, setHighlight] = useState<string>();
  const [dialog, setDialog] = useState<DialogRequest>();
  const [draftInvoker, setDraftInvoker] = useState<HTMLButtonElement | null>(null);
  const [logCount, setLogCount] = useState<number>();
  const [toast, showToast] = useToast();
  const [failed, setFailed] = useState(false);

  const updatePanelState = useCallback(
    (change: Partial<PanelState>): void => {
      setPanelState((current) => {
        if (current === undefined) return current;
        const next = { ...current, ...change };
        if (api !== undefined) void api.savePanelState(next);
        return next;
      });
    },
    [api],
  );

  const selectTab = useCallback(
    (next: PanelTab): void => {
      updatePanelState({ tab: next });
    },
    [updatePanelState],
  );

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
      requestStop: (view, invoker) => {
        setDialog({ kind: "stop", view, invoker });
      },
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
        selectTab("projects");
      },
      adopt: (view, invoker) => {
        const draft: AdoptDraft = {
          kind: "adopt",
          reference: view.ref,
          mode: "options",
          name: view.cwd?.split("/").filter(Boolean).pop() ?? "",
          lastStep: null,
        };
        updatePanelState({ draft });
        setDraftInvoker(invoker);
      },
      declareShared: (view) => {
        attempt(api.adopt(view.ref, "shared"), text.shared.declared);
      },
    };
  }, [api, snapshot, now, attempt, selectTab, updatePanelState]);

  if (error !== undefined) {
    return (
      <Shell
        title="DevPorts"
        subtitle={text.status.failed}
        body={<p className="empty">{error}</p>}
      />
    );
  }
  if (
    api === undefined ||
    snapshot === undefined ||
    actions === undefined ||
    panelState === undefined
  ) {
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
      {(missingDraft ||
        (panelState.draft !== null &&
          !snapshot.instances.some((view) => view.ref === panelState.draft?.reference))) && (
        <p className="card blue" role="status">
          {text.status.draftGone}
        </p>
      )}
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
      {tab === "projects" && (
        <ProjectsView
          actions={actions}
          highlight={highlight}
          expandedProject={panelState.expandedProject}
          onExpandedProjectChange={(expandedProject) => {
            updatePanelState({ expandedProject, expandedProjectSet: true });
          }}
        />
      )}
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
        <ActivityView
          api={api}
          revision={snapshot.revision}
          onCount={setLogCount}
          filter={panelState.activityFilter}
          onFilterChange={(activityFilter) => {
            updatePanelState({ activityFilter });
          }}
        />
      )}
    </>
  );

  const confirmed = dialog;
  const adoptionDraft = panelState.draft;
  const adoptionView =
    adoptionDraft === null
      ? undefined
      : snapshot.instances.find((view) => view.ref === adoptionDraft.reference);
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
                  selectTab(id);
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
              view={confirmed.view}
              returnFocusTo={confirmed.invoker}
              affected={dependentsOf(confirmed.view, snapshot)}
              onCancel={() => {
                setDialog(undefined);
              }}
              onConfirm={() => {
                setDialog(undefined);
                attempt(api.stop(confirmed.view.ref));
              }}
            />
          )}
          {adoptionDraft !== null && adoptionView !== undefined && (
            <AdoptSheet
              view={adoptionView}
              draft={adoptionDraft}
              returnFocusTo={draftInvoker}
              projects={snapshot.projects}
              onCancel={() => {
                updatePanelState({ draft: null });
                setDraftInvoker(null);
              }}
              onDraftChange={(draft) => {
                updatePanelState({ draft });
              }}
              onChoose={(choice) => {
                updatePanelState({ draft: null });
                setDraftInvoker(null);
                attempt(adoptWith(api, adoptionView, choice), "Listo");
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
