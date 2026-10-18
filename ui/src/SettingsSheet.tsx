import { useEffect, useEffectEvent, useRef, useState, type ReactElement } from "react";

import type { Backend } from "./api";
import { useMounted, useOperationWatch, useSingleFlight } from "./hooks";
import { Modal } from "./Modal";
import { configuredRoots, rootsChanged, settingsFailure, settingsMessage } from "./settings";
import { text } from "./text";
import type { RootProblem, ScanFolders, ScanPreview, SettingsDraft, SettingsError } from "./types";

const t = text.settings;

/**
 * Edits the folders projects are searched in. Typed folders live in the shell's draft;
 * the file, its version and the effect on discovery are read again on restore. A sent
 * save is followed by its id, never sent twice.
 */
export function SettingsSheet({
  api,
  draft,
  onClose,
  onCopy,
  onDraftChange,
  returnFocusTo,
}: {
  api: Backend;
  draft: SettingsDraft;
  onClose: () => void;
  onCopy: (value: string) => void;
  onDraftChange: (draft: SettingsDraft) => Promise<void>;
  returnFocusTo: HTMLElement | null;
}): ReactElement {
  const [folders, setFolders] = useState<ScanFolders>();
  const [loadError, setLoadError] = useState<string>();
  const [preview, setPreview] = useState<ScanPreview>();
  const [problems, setProblems] = useState<RootProblem[]>([]);
  const [failure, setFailure] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [reads, setReads] = useState(0);
  const firstRef = useRef<HTMLInputElement>(null);
  const isAlive = useMounted();
  const exclusive = useSingleFlight();

  const operationId = draft.operationId ?? undefined;
  const sent = operationId !== undefined;
  const { operation, record, error: queryError, refresh } = useOperationWatch(api, operationId);
  const inFlight =
    sent &&
    queryError === undefined &&
    (operation === undefined || operation?.status === "queued" || operation?.status === "running");

  useEffect(() => {
    if (sent) return;
    let active = true;
    void api.scanFolders().then(
      (next) => {
        if (!active) return;
        setFolders(next);
        setLoadError(undefined);
      },
      (error: unknown) => {
        if (active) setLoadError(settingsMessage(settingsFailure(error)));
      },
    );
    return () => {
      active = false;
    };
  }, [api, sent, reads]);

  const change = (next: SettingsDraft): void => {
    void onDraftChange(next).catch((error: unknown) => {
      if (isAlive()) setFailure(settingsMessage(settingsFailure(error)));
    });
  };
  const showProblems = (error: SettingsError): void => {
    setProblems(error.code === "invalidRoots" ? error.problems : []);
    setFailure(error.code === "invalidRoots" ? undefined : settingsMessage(error));
  };
  // Start from what config.yml says; an empty list is never valid, so empty means "not started".
  const started = draft.roots.length > 0;
  const seed = useEffectEvent(() => {
    if (folders && !started && !sent) change({ ...draft, roots: configuredRoots(folders) });
  });
  useEffect(() => {
    seed();
  }, [folders]);

  const reviewing = draft.reviewing && !sent;
  // Restoring at the review step reads the effect again; it never saves.
  const restorePreview = useEffectEvent((isActive: () => boolean) => {
    if (!reviewing || preview !== undefined || !folders) return;
    void api.previewScanFolders(draft.roots).then(
      (next) => {
        if (isActive()) setPreview(next);
      },
      (error: unknown) => {
        if (!isActive()) return;
        showProblems(settingsFailure(error));
        change({ ...draft, reviewing: false });
      },
    );
  });
  useEffect(() => {
    const state = { active: true };
    restorePreview(() => state.active);
    return () => {
      state.active = false;
    };
  }, [reviewing, folders]);

  const review = (): Promise<void> =>
    exclusive(async () => {
      setBusy(true);
      setFailure(undefined);
      setProblems([]);
      try {
        const next = await api.previewScanFolders(draft.roots);
        if (!isAlive()) return;
        setPreview(next);
        change({ ...draft, reviewing: true });
      } catch (error) {
        if (isAlive()) showProblems(settingsFailure(error));
      } finally {
        if (isAlive()) setBusy(false);
      }
    });
  const save = (): Promise<void> =>
    exclusive(async () => {
      if (!folders) return;
      setBusy(true);
      setFailure(undefined);
      try {
        const id = crypto.randomUUID();
        // Persist the key before sending: a lost response is queried, never re-sent.
        await onDraftChange({ ...draft, operationId: id });
        if (!isAlive()) return;
        record(
          id,
          await api.startOperation({
            id,
            kind: "setScanFolders",
            targets: [
              {
                reference: "config",
                scanFolders: { roots: draft.roots.map((root) => root.trim()), base: folders.base },
              },
            ],
          }),
        );
      } catch (error) {
        if (isAlive()) setFailure(settingsMessage(settingsFailure(error)));
      } finally {
        if (isAlive()) setBusy(false);
      }
    });
  const retry = (id: string): Promise<void> =>
    exclusive(async () => {
      setBusy(true);
      try {
        record(id, await api.retryOperation(id));
      } catch (error) {
        if (isAlive()) setFailure(settingsMessage(settingsFailure(error)));
      } finally {
        if (isAlive()) setBusy(false);
      }
    });
  // After a conflict or a rejected folder the typed list is kept and the file is read again.
  const editAgain = (returned?: SettingsError): void => {
    setFolders(undefined);
    setPreview(undefined);
    setReads((value) => value + 1);
    if (returned) showProblems(returned);
    change({ ...draft, reviewing: false, operationId: null });
  };

  const result = operation?.targets[0];
  const resultError = result?.settingsError ?? undefined;
  const succeeded = operation?.status === "succeeded";
  const stale = resultError?.code === "documentChanged" || resultError?.code === "invalidRoots";
  const known = new Map(folders?.roots.map((root) => [root.configured, root]));
  const changed = folders ? rootsChanged(folders, draft.roots) : false;
  const editable = folders?.editable === true;

  return (
    <Modal
      title={reviewing ? t.reviewTitle : t.title}
      description={reviewing ? t.reviewBody : t.description}
      bottom
      onCancel={onClose}
      returnFocusTo={returnFocusTo}
      initialFocusRef={firstRef}
    >
      {failure !== undefined && (
        <p className="form-error" role="alert">
          {failure}
        </p>
      )}
      {loadError !== undefined && !sent && (
        <>
          <p className="form-error" role="alert">
            {t.loadFailed} {loadError}
          </p>
          <button
            type="button"
            className="btn"
            onClick={() => {
              editAgain();
            }}
          >
            {t.retryLoad}
          </button>
        </>
      )}
      {!sent && folders === undefined && loadError === undefined && (
        <p role="status">{t.loading}</p>
      )}
      {!sent && folders !== undefined && !editable && (
        <p className="form-error" role="alert">
          {t.notEditable}: {folders.reason}
        </p>
      )}

      {!sent && folders !== undefined && !reviewing && (
        <div className="stack">
          {folders.fromDefaults && <p className="soft">{t.fromDefaults}</p>}
          <ul className="consumers" aria-label={t.title}>
            {draft.roots.map((root, index) => {
              const state = known.get(root);
              const problem = problems.find((item) => item.index === index);
              return (
                <li key={index}>
                  <div className="row">
                    <input
                      ref={index === 0 ? firstRef : undefined}
                      className="input grow mono"
                      aria-label={t.folder(index + 1)}
                      aria-invalid={problem !== undefined}
                      placeholder={t.placeholder}
                      value={root}
                      disabled={!editable}
                      spellCheck={false}
                      onChange={(event) => {
                        setProblems([]);
                        change({
                          ...draft,
                          roots: draft.roots.map((item, at) =>
                            at === index ? event.target.value : item,
                          ),
                        });
                      }}
                    />
                    <button
                      type="button"
                      className="btn"
                      aria-label={t.remove(root || t.folder(index + 1))}
                      disabled={!editable}
                      onClick={() => {
                        setProblems([]);
                        change({ ...draft, roots: draft.roots.filter((_, at) => at !== index) });
                      }}
                    >
                      −
                    </button>
                  </div>
                  {problem !== undefined && <p className="form-error">{problem.message}</p>}
                  {problem === undefined && state !== undefined && (
                    <p className="soft adoption-reason">
                      {state.exists ? `${t.exists} · ${t.projects(state.projects)}` : t.missing}
                    </p>
                  )}
                </li>
              );
            })}
          </ul>
          <button
            type="button"
            className="btn"
            disabled={!editable || draft.roots.length >= 32}
            onClick={() => {
              change({ ...draft, roots: [...draft.roots, ""] });
            }}
          >
            {t.add}
          </button>
          <p className="soft">
            {t.advanced} <span className="mono">{folders.configPath}</span>{" "}
            <button
              type="button"
              className="link-btn"
              onClick={() => {
                onCopy(folders.configPath);
              }}
            >
              {t.copyPath}
            </button>
          </p>
        </div>
      )}

      {reviewing && preview !== undefined && (
        <div className="stack">
          <ul className="consumers" aria-label={t.reviewTitle}>
            {preview.roots.map((root) => (
              <li key={root.path}>
                <span className="name mono">{root.path}</span>
                <span className="soft">
                  {" "}
                  · {root.exists ? t.projects(root.projects) : t.missing}
                </span>
              </li>
            ))}
          </ul>
          {(
            [
              [t.added, preview.added],
              [t.removed, preview.removed],
            ] as const
          ).map(([title, names]) =>
            names.length === 0 ? null : (
              <section key={title}>
                <div className="eyebrow">{title}</div>
                <ul className="consumers">
                  {names.map((name) => (
                    <li key={name} className="name">
                      {name}
                    </li>
                  ))}
                </ul>
              </section>
            ),
          )}
          {preview.added.length === 0 && preview.removed.length === 0 && (
            <p className="soft">{t.nothingChanges}</p>
          )}
        </div>
      )}

      {sent && (
        <div className="stack">
          {inFlight && <p role="status">{operation === undefined ? t.checking : t.saving}</p>}
          {queryError !== undefined && (
            <>
              <p className="form-error" role="alert">
                {queryError}
              </p>
              <button type="button" className="btn" onClick={refresh}>
                {t.retryQuery}
              </button>
            </>
          )}
          {operation === null && !busy && (
            <>
              <p role="status">{t.unsent}</p>
              <button
                type="button"
                className="btn"
                onClick={() => {
                  editAgain();
                }}
              >
                {t.reread}
              </button>
            </>
          )}
          {succeeded && <p role="status">{t.saved}</p>}
          {operation?.status === "failed" && result && (
            <p className="form-error" role="alert">
              {resultError ? settingsMessage(resultError) : (result.error ?? "")}
            </p>
          )}
          {operation?.status === "failed" && (
            <div className="row">
              {stale ? (
                <button
                  type="button"
                  className="btn"
                  disabled={busy}
                  onClick={() => {
                    editAgain(resultError);
                  }}
                >
                  {resultError.code === "invalidRoots" ? text.orphans.back : t.reread}
                </button>
              ) : (
                <button
                  type="button"
                  className="btn"
                  disabled={busy}
                  onClick={() => {
                    if (operationId) void retry(operationId);
                  }}
                >
                  {t.retry}
                </button>
              )}
            </div>
          )}
        </div>
      )}

      <div className="row adoption-footer">
        {reviewing ? (
          <button
            type="button"
            className="btn"
            disabled={busy}
            onClick={() => {
              change({ ...draft, reviewing: false });
            }}
          >
            {text.orphans.back}
          </button>
        ) : (
          <span />
        )}
        {!sent && folders !== undefined && editable && !reviewing && (
          <button
            type="button"
            className="btn primary-green"
            disabled={busy || !changed}
            onClick={() => {
              void review();
            }}
          >
            {changed ? t.review : t.noChanges}
          </button>
        )}
        {reviewing && (
          <button
            type="button"
            className="btn primary-green"
            disabled={busy || preview === undefined || preview.unchanged}
            onClick={() => {
              void save();
            }}
          >
            {busy ? t.saving : t.save}
          </button>
        )}
        <button type="button" className="link-btn" onClick={onClose}>
          {succeeded ? t.done : sent ? t.close : text.orphans.cancel}
        </button>
      </div>
    </Modal>
  );
}
