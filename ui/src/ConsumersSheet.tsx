import { useEffect, useRef, useState, type ReactElement } from "react";

import type { Backend } from "./api";
import {
  consumerChanges,
  consumersRequest,
  needsFreshRead,
  failureMessage,
  targetFailure,
} from "./consumers";
import { useMounted, useOperationWatch, useSingleFlight } from "./hooks";
import { Modal } from "./Modal";
import { text } from "./text";
import type { ConsumerOptions, ConsumersDraft } from "./types";

const t = text.consumers;

/**
 * Chooses which projects declare one shared service, reviews the manifests that
 * would change and saves them as one tracked operation. Unsent choices live in the
 * shell's draft; a sent operation is followed by its id, never sent twice.
 */
export function ConsumersSheet({
  api,
  draft,
  onClose,
  onDraftChange,
  returnFocusTo,
}: {
  api: Backend;
  draft: ConsumersDraft;
  onClose: () => void;
  onDraftChange: (draft: ConsumersDraft) => Promise<void>;
  returnFocusTo: HTMLElement | null;
}): ReactElement {
  const [options, setOptions] = useState<ConsumerOptions>();
  const [loadError, setLoadError] = useState<string>();
  const [failure, setFailure] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [query, setQuery] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);
  const focusRef = useRef<HTMLInputElement>(null);
  const isAlive = useMounted();
  const exclusive = useSingleFlight();

  const operationId = draft.operationId ?? undefined;
  const { operation, record, error: queryError, refresh } = useOperationWatch(api, operationId);
  const sent = operationId !== undefined;
  const inFlight =
    sent &&
    queryError === undefined &&
    (operation === undefined || operation?.status === "queued" || operation?.status === "running");

  const [reads, setReads] = useState(0);
  useEffect(() => {
    if (sent) return;
    let active = true;
    void api.sharedConsumers(draft.shared).then(
      (next) => {
        if (!active) return;
        setOptions(next);
        setLoadError(undefined);
      },
      (error: unknown) => {
        if (active) setLoadError(error instanceof Error ? error.message : failureMessage(error));
      },
    );
    return () => {
      active = false;
    };
  }, [api, draft.shared, sent, reads]);

  // The rows exist only after the manifests are read: focus the project the sheet
  // was opened for (or the search) then, and again when returning from the review.
  const listed = options !== undefined && !sent && !draft.reviewing;
  const focusTarget = draft.focus === null ? searchRef : focusRef;
  useEffect(() => {
    if (listed) focusTarget.current?.focus();
  }, [listed, focusTarget]);

  const change = (next: ConsumersDraft): void => {
    void onDraftChange(next).catch((error: unknown) => {
      if (isAlive()) setFailure(failureMessage(error));
    });
  };
  const changes = options ? consumerChanges(options, draft.selected) : [];
  const toggle = (project: string, on: boolean): void => {
    const rest = draft.selected.filter((name) => name !== project);
    change({ ...draft, selected: on ? [...rest, project] : rest });
  };
  const save = (): Promise<void> =>
    exclusive(async () => {
      if (!options || changes.length === 0) return;
      setBusy(true);
      setFailure(undefined);
      try {
        const id = crypto.randomUUID();
        // Persist the key before sending: a lost response is queried, never re-sent.
        await onDraftChange({ ...draft, operationId: id });
        if (!isAlive()) return;
        record(id, await api.startOperation(consumersRequest(id, draft.shared, changes)));
      } catch (error) {
        if (isAlive()) setFailure(failureMessage(error));
      } finally {
        if (isAlive()) setBusy(false);
      }
    });
  const retry = (id: string): Promise<void> =>
    exclusive(async () => {
      setBusy(true);
      setFailure(undefined);
      try {
        record(id, await api.retryOperation(id));
      } catch (error) {
        if (isAlive()) setFailure(failureMessage(error));
      } finally {
        if (isAlive()) setBusy(false);
      }
    });
  // After a conflict or a vanished operation the selection is kept and the files are read again.
  const readAgain = (): void => {
    setOptions(undefined);
    setFailure(undefined);
    setReads((value) => value + 1);
    change({ ...draft, reviewing: false, operationId: null });
  };

  const shown =
    options?.consumers.filter((choice) =>
      choice.project.toLowerCase().includes(query.trim().toLowerCase()),
    ) ?? [];
  const selectedCount = draft.selected.length;
  const failedTargets = operation?.targets.filter((target) => target.status === "failed") ?? [];
  const stale = failedTargets.some((target) => needsFreshRead(target.usesError));
  const reviewing = draft.reviewing && options !== undefined && !sent;

  return (
    <Modal
      title={reviewing ? t.reviewTitle : t.title(draft.shared)}
      description={reviewing ? t.reviewBody : t.description}
      bottom
      onCancel={onClose}
      returnFocusTo={returnFocusTo}
      initialFocusRef={draft.focus === null ? searchRef : focusRef}
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
          <button type="button" className="btn" onClick={readAgain}>
            {t.retryLoad}
          </button>
        </>
      )}
      {!sent && options === undefined && loadError === undefined && (
        <p role="status">{t.loading}</p>
      )}
      {!sent && options !== undefined && !options.defined && (
        <p className="form-error" role="alert">
          {t.undefinedShared}
        </p>
      )}

      {!sent && options !== undefined && !reviewing && (
        <div className="stack">
          <input
            ref={searchRef}
            className="input"
            type="search"
            aria-label={t.search}
            placeholder={t.search}
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
            }}
          />
          <p className="soft" role="status">
            {selectedCount === 0 ? t.nobody : t.count(selectedCount)} · {t.declaredOnly}
          </p>
          <ul className="consumers" aria-label={t.associated}>
            {shown.map((choice) => {
              const ticked = draft.selected.includes(choice.project);
              const locked = !choice.eligible || !options.defined;
              return (
                <li key={choice.project}>
                  <label className="consumer">
                    <input
                      ref={choice.project === draft.focus ? focusRef : undefined}
                      type="checkbox"
                      checked={ticked}
                      disabled={locked}
                      onChange={(event) => {
                        toggle(choice.project, event.target.checked);
                      }}
                    />
                    <span className="name">{choice.project}</span>
                  </label>
                  {choice.reason !== null && (
                    <p className="soft adoption-reason">{choice.reason}</p>
                  )}
                </li>
              );
            })}
          </ul>
          {shown.length === 0 && <p className="soft">{t.noMatch}</p>}
        </div>
      )}

      {reviewing && (
        <div className="stack">
          {([true, false] as const).map((include) => {
            const rows = changes.filter((item) => item.include === include);
            return rows.length === 0 ? null : (
              <section key={String(include)}>
                <div className="eyebrow">{include ? t.add : t.remove}</div>
                <ul className="consumers">
                  {rows.map((row) => (
                    <li key={row.project}>
                      <span className="name">{row.project}</span>
                      <div className="soft mono adoption-reason">{row.manifest}</div>
                    </li>
                  ))}
                </ul>
              </section>
            );
          })}
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
              <button type="button" className="btn" onClick={readAgain}>
                {t.reread}
              </button>
            </>
          )}
          {operation && (
            <ul className="consumers" aria-label={t.associated}>
              {operation.targets.map((target) => {
                const message = targetFailure(target);
                return (
                  <li key={target.reference}>
                    <span className="name">{target.reference}</span>{" "}
                    <span className="soft">
                      {target.status === "succeeded"
                        ? t.saved
                        : target.status === "failed"
                          ? ""
                          : t.pending}
                    </span>
                    {message !== undefined && (
                      <p className="form-error" role="alert">
                        {message}
                      </p>
                    )}
                  </li>
                );
              })}
            </ul>
          )}
          {operation?.status === "partial" && <p role="status">{t.partial}</p>}
          {failedTargets.length > 0 && (
            <div className="row">
              <button
                type="button"
                className="btn"
                disabled={busy}
                onClick={() => {
                  void retry(operationId);
                }}
              >
                {t.retry}
              </button>
              {stale && (
                <button type="button" className="btn" disabled={busy} onClick={readAgain}>
                  {t.reread}
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
        {!sent && options !== undefined && options.defined && !reviewing && (
          <button
            type="button"
            className="btn primary-green"
            disabled={changes.length === 0}
            onClick={() => {
              change({ ...draft, reviewing: true });
            }}
          >
            {changes.length === 0 ? t.noChanges : t.review}
          </button>
        )}
        {reviewing && (
          <button
            type="button"
            className="btn primary-green"
            disabled={busy}
            onClick={() => {
              void save();
            }}
          >
            {busy ? t.saving : t.save}
          </button>
        )}
        <button type="button" className="link-btn" onClick={onClose}>
          {operation?.status === "succeeded" ? t.done : sent ? t.close : text.orphans.cancel}
        </button>
      </div>
    </Modal>
  );
}
