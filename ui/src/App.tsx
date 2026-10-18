import { useCallback, useMemo, useState, type ReactElement } from "react";

import type { Actions } from "./actions";
import { ConfirmStop } from "./dialogs";
import { AdoptSheet } from "./AdoptSheet";
import { StatusStrip, shortReason } from "./StatusStrip";
import { ConsumersSheet } from "./ConsumersSheet";
import { ClassifySheet } from "./ClassifySheet";
import { SettingsSheet } from "./SettingsSheet";
import { IgnoreConfirm, IgnoredList } from "./IgnoreSheets";
import { consumersDraft } from "./consumers";
import { revealed, type ProjectFolds } from "./projectFolds";
import { useConnection, useNow, useOperations, usePanelStore, useToast } from "./hooks";
import {
  BellIcon,
  CheckIcon,
  DatabaseIcon,
  FolderIcon,
  GearIcon,
  ListIcon,
  OrphanIcon,
  WarningIcon,
} from "./icons";
import { buildOrphans, dependentsOf, draftTargetExists } from "./model";
import { text } from "./text";
import { targetLabel } from "./instance";
import type { AdoptDraft, InstanceView, PanelState, PanelTab, StopExpectation } from "./types";
import { ActivityView, activitySubtitle, type ActivityStatus } from "./views/ActivityView";
import { AttentionView, attentionSubtitle } from "./views/AttentionView";
import { OrphansView, orphansSubtitle } from "./views/OrphansView";
import { ProjectsView, projectsSubtitle } from "./views/ProjectsView";
import { SharedView, sharedSubtitle } from "./views/SharedView";

type Tab = "attention" | "projects" | "shared" | "orphans" | "activity";
type IgnoreRequest = { view: InstanceView; invoker: HTMLButtonElement };
type DialogRequest = {
  kind: "stop" | "stopProject";
  views: InstanceView[];
  invoker: HTMLButtonElement | null;
  project?: string;
};

const TABS: { id: Tab; icon: (props: { size?: number }) => ReactElement }[] = [
  { id: "attention", icon: BellIcon },
  { id: "projects", icon: FolderIcon },
  { id: "shared", icon: DatabaseIcon },
  { id: "orphans", icon: OrphanIcon },
  { id: "activity", icon: ListIcon },
];

const tabId = (id: Tab): string => `tab-${id}`;
const panelId = (id: Tab): string => `panel-${id}`;

/** The tab an arrow, Home or End key moves to; roving focus wraps at the ends. */
function nextTab(current: Tab, key: string): Tab | undefined {
  const index = TABS.findIndex((entry) => entry.id === current);
  const last = TABS.length - 1;
  const at =
    key === "ArrowRight"
      ? (index + 1) % TABS.length
      : key === "ArrowLeft"
        ? (index + last) % TABS.length
        : key === "Home"
          ? 0
          : key === "End"
            ? last
            : undefined;
  return at === undefined ? undefined : TABS[at]?.id;
}

function initialTab(): Tab {
  const wanted = new URLSearchParams(window.location.search).get("tab");
  return TABS.find((entry) => entry.id === wanted)?.id ?? "attention";
}

export function App(): ReactElement {
  const {
    api,
    snapshot,
    link,
    retry: retryConnection,
    panelState: hydratedState,
    missingDraft,
  } = useConnection();
  const now = useNow();
  const [highlight, setHighlight] = useState<string>();
  const [dialog, setDialog] = useState<DialogRequest>();
  const [ignoreRequest, setIgnoreRequest] = useState<IgnoreRequest>();
  const [classifyRequest, setClassifyRequest] = useState<IgnoreRequest>();
  const [listInvoker, setListInvoker] = useState<HTMLButtonElement | null>();
  const [preparingProjectStop, setPreparingProjectStop] = useState(false);
  const [draftInvoker, setDraftInvoker] = useState<HTMLButtonElement | null>(null);
  const [activity, setActivity] = useState<ActivityStatus>({ kind: "loading" });
  const [toast, showToast] = useToast();
  const [failed, setFailed] = useState(false);
  const { panelState, persist: persistPanelState } = usePanelStore(api, hydratedState);
  const tab = panelState?.tab ?? initialTab();
  const operationState = useOperations(api, link.epoch);
  const { start: startOperation, retry: retryOperation, operations } = operationState;
  const runOperation = useCallback(
    (kind: "extend" | "keep", reference: string): void => {
      void startOperation({
        kind,
        targets: [{ reference }],
      }).catch((failure: unknown) => {
        setFailed(true);
        showToast(failure instanceof Error ? failure.message : String(failure));
      });
    },
    [startOperation, showToast],
  );
  const stopBatch = useCallback(
    (views: InstanceView[], kind: "stop" | "stopProject"): void => {
      void startOperation({
        kind,
        targets: views.map((view) => ({
          reference: view.ref,
          stopExpectation: stopExpectation(view),
        })),
      }).catch((failure: unknown) => {
        setFailed(true);
        showToast(failure instanceof Error ? failure.message : String(failure));
      });
    },
    [startOperation, showToast],
  );

  const updatePanelState = useCallback(
    (change: Partial<PanelState>): void => {
      void persistPanelState(change).catch((failure: unknown) => {
        setFailed(true);
        showToast(failure instanceof Error ? failure.message : String(failure));
      });
    },
    [persistPanelState, showToast],
  );

  const selectTab = useCallback(
    (next: PanelTab): void => {
      updatePanelState({ tab: next });
    },
    [updatePanelState],
  );

  const changeFolds = useCallback(
    (folds: ProjectFolds): void => {
      updatePanelState({ openProjects: [...folds.open], closedProjects: [...folds.closed] });
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
      preparingProjectStop,
      requestProjectStop: (project, invoker) => {
        if (preparingProjectStop) return;
        setPreparingProjectStop(true);
        void api
          .previewProjectStop(project)
          .then(
            (views) => {
              if (views.length === 0) showToast(text.projects.nothingToStop);
              else setDialog({ kind: "stopProject", project, views, invoker });
            },
            (failure: unknown) => {
              setFailed(true);
              showToast(failure instanceof Error ? failure.message : String(failure));
            },
          )
          .finally(() => {
            setPreparingProjectStop(false);
          });
      },
      requestStop: (views, invoker) => {
        setDialog({ kind: "stop", views, invoker });
      },
      extend: (view) => {
        runOperation("extend", view.ref);
      },
      keep: (view) => {
        runOperation("keep", view.ref);
      },
      ignore: (view, invoker) => {
        setIgnoreRequest({ view, invoker });
      },
      classify: (view, invoker) => {
        setClassifyRequest({ view, invoker });
      },
      openSettings: (invoker) => {
        updatePanelState({ draft: { kind: "settings", roots: [], reviewing: false } });
        setDraftInvoker(invoker);
      },
      showIgnored: (invoker) => {
        setListInvoker(invoker);
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
      manageConsumers: (shared, invoker, focus) => {
        updatePanelState({ draft: consumersDraft(snapshot, shared, focus ?? null) });
        setDraftInvoker(invoker);
      },
      declareShared: (view, invoker) => {
        updatePanelState({
          draft: { kind: "adopt", reference: view.ref, mode: "options", name: "", lastStep: null },
        });
        setDraftInvoker(invoker);
      },
    };
  }, [
    api,
    snapshot,
    now,
    attempt,
    runOperation,
    selectTab,
    updatePanelState,
    preparingProjectStop,
    showToast,
  ]);

  if (
    api === undefined ||
    snapshot === undefined ||
    actions === undefined ||
    panelState === undefined
  ) {
    return link.phase === "failed" ? (
      <Shell
        title="DevPorts"
        subtitle={text.link.cannotConnect}
        body={
          <div className="card coral" role="alert">
            <p className="soft">{text.link.cannotConnectBody}</p>
            {link.error !== null && (
              <p className="soft mono" style={{ fontSize: 11 }}>
                {shortReason(link.error)}
              </p>
            )}
            <div className="row" style={{ justifyContent: "flex-end" }}>
              <button type="button" className="btn primary-green" onClick={retryConnection}>
                {text.link.retry}
              </button>
            </div>
          </div>
        }
      />
    ) : (
      <Shell
        title="DevPorts"
        subtitle={
          link.attempt > 0
            ? text.link.connectingAttempt(link.attempt, link.attempts)
            : text.status.connecting
        }
        body={null}
      />
    );
  }

  const orphanCount = buildOrphans(snapshot).length;
  const subtitle: Record<Tab, string> = {
    attention: attentionSubtitle(snapshot.attention.length),
    projects: projectsSubtitle(actions),
    shared: sharedSubtitle(actions),
    orphans: orphansSubtitle(actions),
    activity: activitySubtitle(activity),
  };

  const body = (
    <>
      <StatusStrip
        link={link}
        snapshot={snapshot}
        now={now}
        onRetry={retryConnection}
        onDetails={actions.openSettings}
      />
      {operations.length > 0 && (
        <section className="card" aria-label="Operaciones">
          <div className="eyebrow">{text.operations.title}</div>
          {operations.slice(0, 5).map((operation) => (
            <div key={operation.id} className="soft" style={{ marginTop: 8 }}>
              <strong>{text.operations.kind[operation.kind]}</strong>
              {operation.targets.map((target) => (
                <div key={target.reference} role={target.status === "failed" ? "alert" : "status"}>
                  {targetLabel(target, snapshot.instances)}:{" "}
                  {target.status === "queued"
                    ? text.operations.queued
                    : target.status === "running"
                      ? text.operations.running
                      : target.status === "succeeded"
                        ? target.result
                          ? text.operations.adopted(target.result.project, target.result.service)
                          : text.operations.succeeded
                        : (target.error ?? "Falló")}
                </div>
              ))}
              {(operation.status === "failed" || operation.status === "partial") && (
                <button
                  type="button"
                  onClick={() => {
                    void retryOperation(operation.id);
                  }}
                >
                  {text.operations.retry}
                </button>
              )}
            </div>
          ))}
        </section>
      )}
      {(missingDraft ||
        (panelState.draft !== null && !draftTargetExists(panelState.draft, snapshot))) && (
        <p className="card blue" role="status">
          {text.status.draftGone}
        </p>
      )}
      {tab === "attention" && <AttentionView actions={actions} />}
      {tab === "projects" && (
        <ProjectsView
          actions={actions}
          highlight={highlight}
          folds={{ open: panelState.openProjects, closed: panelState.closedProjects }}
          onFoldsChange={changeFolds}
        />
      )}
      {tab === "shared" && <SharedView actions={actions} />}
      {tab === "orphans" && <OrphansView actions={actions} />}
      {tab === "activity" && (
        <ActivityView
          api={api}
          revision={snapshot.revision}
          onStatus={setActivity}
          filter={panelState.activityFilter}
          onFilterChange={(activityFilter) => {
            updatePanelState({ activityFilter });
          }}
        />
      )}
    </>
  );

  const confirmed = dialog;
  const adoptionDraft = panelState.draft?.kind === "adopt" ? panelState.draft : null;
  const sharedDraft = panelState.draft?.kind === "consumers" ? panelState.draft : null;
  const settingsDraft = panelState.draft?.kind === "settings" ? panelState.draft : null;
  const adoptionView =
    adoptionDraft === null
      ? undefined
      : snapshot.instances.find((view) => view.ref === adoptionDraft.reference);
  return (
    <Shell
      title={text.tabs[tab]}
      subtitle={subtitle[tab]}
      onSettings={actions.openSettings}
      body={body}
      tabKey={tab}
      footer={
        <div
          className="tabs"
          role="tablist"
          aria-label="Secciones"
          onKeyDown={(event) => {
            const next = nextTab(tab, event.key);
            if (next === undefined) return;
            event.preventDefault();
            selectTab(next);
            document.getElementById(tabId(next))?.focus();
          }}
        >
          {TABS.map(({ id, icon: Icon }) => {
            const count =
              id === "attention" ? snapshot.attention.length : id === "orphans" ? orphanCount : 0;
            return (
              <button
                key={id}
                type="button"
                role="tab"
                id={tabId(id)}
                className="tab"
                aria-selected={tab === id}
                aria-controls={panelId(id)}
                tabIndex={tab === id ? 0 : -1}
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
        </div>
      }
      overlay={
        <>
          {confirmed !== undefined && (
            <ConfirmStop
              views={confirmed.views}
              project={confirmed.project}
              returnFocusTo={confirmed.invoker}
              affected={[
                ...new Set(confirmed.views.flatMap((view) => dependentsOf(view, snapshot))),
              ]}
              onCancel={() => {
                setDialog(undefined);
              }}
              onConfirm={() => {
                setDialog(undefined);
                const current = confirmed.views.map((view) =>
                  snapshot.instances.find((candidate) => candidate.ref === view.ref),
                );
                if (
                  confirmed.kind === "stop" &&
                  current.some((view, index) => {
                    const requested = confirmed.views[index];
                    return (
                      view === undefined ||
                      requested === undefined ||
                      !sameStopTarget(view, requested)
                    );
                  })
                ) {
                  setFailed(true);
                  showToast(text.confirm.targetChanged);
                  return;
                }
                stopBatch(confirmed.views, confirmed.kind);
              }}
            />
          )}
          {adoptionDraft !== null &&
            (adoptionView !== undefined || adoptionDraft.operationId != null) && (
              <AdoptSheet
                key={adoptionDraft.reference}
                api={api}
                view={adoptionView}
                snapshot={snapshot}
                draft={adoptionDraft}
                returnFocusTo={draftInvoker}
                onCancel={() => {
                  updatePanelState({ draft: null });
                  setDraftInvoker(null);
                }}
                onDraftChange={(draft) => persistPanelState({ draft })}
                onNavigate={(project, service, shared) => {
                  setHighlight(
                    snapshot.instances.find(
                      (view) => view.project === project && view.service === service,
                    )?.ref,
                  );
                  const shown = revealed(
                    { open: panelState.openProjects, closed: panelState.closedProjects },
                    project,
                  );
                  updatePanelState({
                    draft: null,
                    tab: shared ? "shared" : "projects",
                    ...(shared
                      ? {}
                      : { openProjects: [...shown.open], closedProjects: [...shown.closed] }),
                  });
                  setDraftInvoker(null);
                }}
              />
            )}
          {settingsDraft !== null && (
            <SettingsSheet
              api={api}
              link={link}
              snapshot={snapshot}
              draft={settingsDraft}
              returnFocusTo={draftInvoker}
              onCopy={actions.copy}
              onClose={() => {
                updatePanelState({ draft: null });
                setDraftInvoker(null);
              }}
              onDraftChange={(draft) => persistPanelState({ draft })}
            />
          )}
          {ignoreRequest !== undefined && (
            <IgnoreConfirm
              api={api}
              reference={ignoreRequest.view.ref}
              returnFocusTo={ignoreRequest.invoker}
              onCancel={() => {
                setIgnoreRequest(undefined);
              }}
              onConfirm={() => {
                setIgnoreRequest(undefined);
                runOperation("keep", ignoreRequest.view.ref);
              }}
            />
          )}
          {classifyRequest !== undefined && (
            <ClassifySheet
              api={api}
              reference={classifyRequest.view.ref}
              classification={snapshot.classifications.find(
                (item) => item.instanceRef === classifyRequest.view.ref,
              )}
              returnFocusTo={classifyRequest.invoker}
              onCancel={() => {
                setClassifyRequest(undefined);
              }}
              onSave={(choice) => {
                setClassifyRequest(undefined);
                void startOperation({
                  kind: "classify",
                  targets: [
                    { reference: classifyRequest.view.ref, classification: { class: choice } },
                  ],
                }).catch((failure: unknown) => {
                  setFailed(true);
                  showToast(failure instanceof Error ? failure.message : String(failure));
                });
              }}
            />
          )}
          {listInvoker !== undefined && (
            <IgnoredList
              api={api}
              revision={operations.length + (operations[0]?.revision ?? 0)}
              returnFocusTo={listInvoker}
              onClose={() => {
                setListInvoker(undefined);
              }}
              onRestore={async (id) => {
                const done = await startOperation({
                  kind: "restore",
                  targets: [{ reference: id }],
                });
                const failed = done?.targets.find((target) => target.status === "failed");
                if (failed) throw new Error(failed.error ?? "Falló");
              }}
            />
          )}
          {sharedDraft !== null && (
            <ConsumersSheet
              key={sharedDraft.shared}
              api={api}
              draft={sharedDraft}
              returnFocusTo={draftInvoker}
              onClose={() => {
                updatePanelState({ draft: null });
                setDraftInvoker(null);
              }}
              onDraftChange={(draft) => persistPanelState({ draft })}
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
  onSettings,
  body,
  footer,
  overlay,
  tabKey,
}: {
  title: string;
  subtitle: string;
  onSettings?: (invoker: HTMLButtonElement) => void;
  body: ReactElement | null;
  footer?: ReactElement;
  overlay?: ReactElement;
  tabKey?: string;
}): ReactElement {
  return (
    <div className="panel">
      <header className="head">
        <div className="head-row">
          <h1>{title}</h1>
          {onSettings && (
            <button
              type="button"
              className="icon-btn"
              aria-label={text.settings.open}
              title={text.settings.open}
              onClick={(event) => {
                onSettings(event.currentTarget);
              }}
            >
              <GearIcon size={16} />
            </button>
          )}
        </div>
        <p>{subtitle}</p>
      </header>
      <main
        className="body"
        key={tabKey}
        role="tabpanel"
        id={tabKey === undefined ? undefined : panelId(tabKey as Tab)}
        aria-labelledby={tabKey === undefined ? undefined : tabId(tabKey as Tab)}
      >
        {body}
      </main>
      {footer ?? <div />}
      {overlay}
    </div>
  );
}

async function copyText(value: string): Promise<void> {
  await navigator.clipboard.writeText(value);
}

function stopExpectation(view: InstanceView): StopExpectation {
  return {
    targetIdentity: view.target_identity,
    pid: view.pid,
    container: view.container,
    port: view.port,
    project: view.project,
    service: view.service,
    kind: view.kind,
    role: view.role,
    shared: view.shared,
    startedAt: view.started_at,
  };
}

function sameStopTarget(current: InstanceView, confirmed: InstanceView): boolean {
  return JSON.stringify(stopExpectation(current)) === JSON.stringify(stopExpectation(confirmed));
}
