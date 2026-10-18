import { useCallback, useMemo, useState, type ReactElement } from "react";

import type { Actions } from "./actions";
import { ConfirmStop } from "./dialogs";
import { AdoptSheet } from "./AdoptSheet";
import { ConsumersSheet } from "./ConsumersSheet";
import { ClassifySheet } from "./ClassifySheet";
import { IgnoreConfirm, IgnoredList } from "./IgnoreSheets";
import { consumersDraft } from "./consumers";
import { useConnection, useNow, useOperations, usePanelStore, useToast } from "./hooks";
import {
  BellIcon,
  CheckIcon,
  DatabaseIcon,
  FolderIcon,
  ListIcon,
  OrphanIcon,
  WarningIcon,
} from "./icons";
import { buildOrphans, dependentsOf, draftTargetExists } from "./model";
import { text } from "./text";
import type { AdoptDraft, InstanceView, PanelState, PanelTab, StopExpectation } from "./types";
import { ActivityView, activitySubtitle } from "./views/ActivityView";
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

function initialTab(): Tab {
  const wanted = new URLSearchParams(window.location.search).get("tab");
  return TABS.find((entry) => entry.id === wanted)?.id ?? "attention";
}

export function App(): ReactElement {
  const { api, snapshot, error, panelState: hydratedState, missingDraft } = useConnection();
  const now = useNow();
  const [highlight, setHighlight] = useState<string>();
  const [dialog, setDialog] = useState<DialogRequest>();
  const [ignoreRequest, setIgnoreRequest] = useState<IgnoreRequest>();
  const [classifyRequest, setClassifyRequest] = useState<IgnoreRequest>();
  const [listInvoker, setListInvoker] = useState<HTMLButtonElement | null>();
  const [preparingProjectStop, setPreparingProjectStop] = useState(false);
  const [draftInvoker, setDraftInvoker] = useState<HTMLButtonElement | null>(null);
  const [logCount, setLogCount] = useState<number>();
  const [toast, showToast] = useToast();
  const [failed, setFailed] = useState(false);
  const { panelState, persist: persistPanelState } = usePanelStore(api, hydratedState);
  const tab = panelState?.tab ?? initialTab();
  const operationState = useOperations(api);
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

  const expandProject = useCallback(
    (expandedProject: string | null): void => {
      updatePanelState({ expandedProject, expandedProjectSet: true });
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
      {operations.length > 0 && (
        <section className="card" aria-label="Operaciones">
          <div className="eyebrow">{text.operations.title}</div>
          {operations.slice(0, 5).map((operation) => (
            <div key={operation.id} className="soft" style={{ marginTop: 8 }}>
              <strong>{text.operations.kind[operation.kind]}</strong>
              {operation.targets.map((target) => (
                <div key={target.reference} role={target.status === "failed" ? "alert" : "status"}>
                  {target.reference}:{" "}
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
          onExpandedProjectChange={expandProject}
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
  const adoptionDraft = panelState.draft?.kind === "adopt" ? panelState.draft : null;
  const sharedDraft = panelState.draft?.kind === "consumers" ? panelState.draft : null;
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
                  updatePanelState({
                    draft: null,
                    tab: shared ? "shared" : "projects",
                    expandedProject: shared ? panelState.expandedProject : project,
                    expandedProjectSet: true,
                  });
                  setDraftInvoker(null);
                }}
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
