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
import { appTheme } from "./boot";
import {
  useConnection,
  useNow,
  useOperations,
  usePanelStore,
  usePreferences,
  useToast,
} from "./hooks";
import { CheckIcon, WarningIcon } from "./icons";
import { buildOrphans, dependentsOf, draftTargetExists } from "./model";
import { APP_NAME } from "./brand";
import { operationFailureText } from "./failures";
import { failureText } from "./reasons";
import { I18nProvider, useI18n } from "./i18n/provider";
import type { AdoptDraft, InstanceView, LanguagePreference, PanelState, PanelTab } from "./types";
import { ActivityView, activitySubtitle, type ActivityStatus } from "./views/ActivityView";
import { AttentionView, attentionSubtitle } from "./views/AttentionView";
import { OrphansView, orphansSubtitle } from "./views/OrphansView";
import { ProjectsView, projectsSubtitle } from "./views/ProjectsView";
import { SharedView, sharedSubtitle } from "./views/SharedView";
import { OperationsSection } from "./OperationsSection";
import { Shell } from "./Shell";
import { instanceTitle } from "./instance";
import { sameStopTarget, stopExpectation } from "./stopTarget";
import { TabBar, initialTab, type Tab } from "./tabs";

type IgnoreRequest = { view: InstanceView; invoker: HTMLButtonElement };
type DialogRequest = {
  kind: "stop" | "stopProject";
  views: InstanceView[];
  invoker: HTMLButtonElement | null;
  project?: string;
};

/** The panel, in the language in force: the preference the shell asked for, until it is changed. */
export function App({
  initialLanguage = "system",
}: {
  initialLanguage?: LanguagePreference;
}): ReactElement {
  const [language, setLanguage] = useState(initialLanguage);
  return (
    <I18nProvider preference={language}>
      <Panel onLanguage={setLanguage} />
    </I18nProvider>
  );
}

function Panel({
  onLanguage,
}: {
  onLanguage: (language: LanguagePreference) => void;
}): ReactElement {
  const { text } = useI18n();
  const {
    api,
    snapshot,
    link,
    retry: retryConnection,
    panelState: hydratedState,
    missingDraft,
  } = useConnection();
  const now = useNow();
  const preferences = usePreferences(api, appTheme, onLanguage);
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
  const { start: startOperation, retry: retryOperation, operations, visible } = operationState;
  const runOperation = useCallback(
    (kind: "extend" | "keep", reference: string): void => {
      void startOperation({
        kind,
        targets: [{ reference }],
      }).catch((failure: unknown) => {
        setFailed(true);
        showToast(failureText(failure, text));
      });
    },
    [startOperation, showToast, text],
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
        showToast(failureText(failure, text));
      });
    },
    [startOperation, showToast, text],
  );

  const updatePanelState = useCallback(
    (change: Partial<PanelState>): void => {
      void persistPanelState(change).catch((failure: unknown) => {
        setFailed(true);
        showToast(failureText(failure, text));
      });
    },
    [persistPanelState, showToast, text],
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
          showToast(failureText(failure, text));
        },
      );
    },
    [showToast, text],
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
              showToast(failureText(failure, text));
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
    text,
  ]);

  if (
    api === undefined ||
    snapshot === undefined ||
    actions === undefined ||
    panelState === undefined
  ) {
    return link.phase === "failed" ? (
      <Shell
        title={APP_NAME}
        subtitle={text.link.cannotConnect}
        body={
          <section className="group" role="alert">
            <div className="item-note">
              <p>{text.link.cannotConnectBody}</p>
              {link.error !== null && <p className="mono">{shortReason(link.error)}</p>}
            </div>
            <div className="item-actions">
              <button type="button" className="btn primary" onClick={retryConnection}>
                {text.link.retry}
              </button>
            </div>
          </section>
        }
      />
    ) : (
      <Shell
        title={APP_NAME}
        subtitle={
          link.attempt > 0
            ? text.link.connectingAttempt(link.attempt, link.attempts)
            : text.status.connecting
        }
        body={
          <div className="loading" role="status">
            <div className="progress" aria-hidden="true">
              <div className="progress-bar" />
            </div>
            <p className="soft">{text.status.loading}</p>
          </div>
        }
      />
    );
  }

  const orphanCount = buildOrphans(snapshot).length;
  const subtitle: Record<Tab, string> = {
    attention: attentionSubtitle(snapshot.attention.length, text),
    projects: projectsSubtitle(actions, text),
    shared: sharedSubtitle(actions, text),
    orphans: orphansSubtitle(actions, text),
    activity: activitySubtitle(activity, text),
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
      <OperationsSection
        visible={visible}
        instances={snapshot.instances}
        onRetry={(id) => {
          void retryOperation(id);
        }}
      />
      {(missingDraft ||
        (panelState.draft !== null && !draftTargetExists(panelState.draft, snapshot))) && (
        <p className="section-note" role="status">
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
        <TabBar
          tab={tab}
          attentionCount={snapshot.attention.length}
          orphanCount={orphanCount}
          onSelect={selectTab}
        />
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
              kept={[
                ...new Set(
                  confirmed.views.flatMap((view) => {
                    const principal =
                      view.duplicate_of === null
                        ? undefined
                        : snapshot.instances.find((other) => other.ref === view.duplicate_of);
                    return principal === undefined ? [] : [instanceTitle(principal, text)];
                  }),
                ),
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
              preferences={preferences.preferences}
              preferencesFailed={preferences.failed}
              onPreferences={preferences.change}
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
                  showToast(failureText(failure, text));
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
                if (failed) throw new Error(operationFailureText(failed, text));
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

async function copyText(value: string): Promise<void> {
  await navigator.clipboard.writeText(value);
}
