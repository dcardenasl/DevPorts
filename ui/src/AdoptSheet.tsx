import { useCallback, useEffect, useEffectEvent, useRef, useState, type ReactElement } from "react";

import type { Backend } from "./api";
import { adoptionFailure, adoptionMessage, needsNewReview, reviewDraft } from "./adoption";
import { useMounted, useSingleFlight } from "./hooks";
import { Modal } from "./Modal";
import { text } from "./text";
import type {
  AdoptDraft,
  AdoptTarget,
  AdoptionError,
  AdoptionOptions,
  InstanceView,
  OperationView,
  Snapshot,
} from "./types";

const t = text.adoption;

type Review = NonNullable<AdoptDraft["review"]>;

export function AdoptSheet({
  api,
  view,
  snapshot,
  draft,
  onCancel,
  onDraftChange,
  onIgnore,
  onNavigate,
  returnFocusTo,
}: {
  api: Backend;
  view: InstanceView | undefined;
  snapshot: Snapshot;
  draft: AdoptDraft;
  onCancel: () => void;
  onDraftChange: (draft: AdoptDraft) => Promise<void>;
  onIgnore: () => void;
  onNavigate: (project: string, service: string, shared: boolean) => void;
  returnFocusTo: HTMLElement | null;
}): ReactElement {
  const [options, setOptions] = useState<AdoptionOptions>();
  const [failure, setFailure] = useState<AdoptionError>();
  const [busy, setBusy] = useState(false);
  const [observedOperation, setObservedOperation] = useState<{
    id: string;
    value: OperationView | null;
  }>();
  const updateOperation = useCallback((id: string, next: OperationView | null) => {
    setObservedOperation((prior) => {
      if (
        prior?.id === id &&
        prior.value !== null &&
        (next === null || prior.value.revision > next.revision)
      )
        return prior;
      return { id, value: next };
    });
  }, []);
  const [queryError, setQueryError] = useState<string>();
  const [reload, setReload] = useState(0);
  const nameRef = useRef<HTMLInputElement>(null);
  const firstProjectRef = useRef<HTMLButtonElement>(null);
  const optionsFocusRef = useRef<HTMLButtonElement>(null);
  const serviceRef = useRef<HTMLInputElement>(null);
  const isAlive = useMounted();
  const exclusive = useSingleFlight();

  const review = draft.review ?? undefined;
  const operationId = draft.operationId ?? undefined;
  const operation = observedOperation?.id === operationId ? observedOperation?.value : undefined;
  const choosing = draft.mode !== "review";
  const inFlight =
    operationId !== undefined &&
    (operation === undefined || operation?.status === "queued" || operation?.status === "running");

  useEffect(() => {
    if (operationId !== undefined || !choosing) return;
    let active = true;
    void api.adoptionOptions(draft.reference).then(
      (next) => {
        if (active) setOptions(next);
      },
      (error: unknown) => {
        if (active) setFailure(adoptionFailure(error));
      },
    );
    return () => {
      active = false;
    };
  }, [api, draft.reference, choosing, operationId, reload]);

  // Restoring a review is read-only. Invalid fields remain editable and visible.
  const reviewId = review?.id;
  const readReview = useEffectEvent(() => review);
  useEffect(() => {
    const restored = readReview();
    if (operationId !== undefined || choosing || restored === undefined) return;
    let active = true;
    void api.validateAdoption(restored.id, restored.edits).catch((error: unknown) => {
      if (active) setFailure(adoptionFailure(error));
    });
    return () => {
      active = false;
    };
  }, [api, reviewId, operationId, choosing]);

  useEffect(() => {
    if (operationId === undefined) return;
    const state = { active: true };
    const active = (): boolean => state.active;
    let unsubscribe: (() => void) | undefined;
    const merge = (next: OperationView): void => {
      if (!state.active || next.id !== operationId) return;
      updateOperation(operationId, next);
    };
    void (async () => {
      unsubscribe = await api.subscribeOperations(merge);
      if (!state.active) {
        unsubscribe();
        return;
      }
      const stored = await api.operation(operationId);
      if (!active()) return;
      if (stored === null) updateOperation(operationId, null);
      else merge(stored);
    })().catch((error: unknown) => {
      if (state.active) setQueryError(adoptionMessage(adoptionFailure(error)));
    });
    return () => {
      state.active = false;
      unsubscribe?.();
    };
  }, [api, operationId, reload, updateOperation]);

  const persist = async (next: AdoptDraft): Promise<void> => {
    await onDraftChange(next);
  };
  const change = (next: AdoptDraft): void => {
    void persist(next).catch((error: unknown) => {
      if (isAlive()) setFailure(adoptionFailure(error));
    });
  };
  const prepare = (target: AdoptTarget, previous?: Review): Promise<void> =>
    exclusive(async () => {
      setBusy(true);
      setFailure(undefined);
      try {
        const preview = await api.previewAdoption(draft.reference, target);
        if (!isAlive()) return;
        const next = reviewDraft(preview);
        if (previous !== undefined) next.edits = previous.edits;
        await persist({ ...draft, mode: "review", review: next, operationId: null });
      } catch (error) {
        if (isAlive()) setFailure(adoptionFailure(error));
      } finally {
        if (isAlive()) setBusy(false);
      }
    });
  const save = (): Promise<void> =>
    exclusive(async () => {
      if (!review) return;
      setBusy(true);
      setFailure(undefined);
      try {
        const checked = await api.validateAdoption(review.id, review.edits);
        if (!isAlive()) return;
        const id = crypto.randomUUID();
        // Persist the exact submission key before sending. Restore only queries it.
        await persist({
          ...draft,
          review: { ...review, context: reviewDraft(checked).context },
          operationId: id,
        });
        if (!isAlive()) return;
        const result = await api.startOperation({
          id,
          kind: "adopt",
          targets: [
            {
              reference: draft.reference,
              adoptTarget: review.target,
              adoption: { id: review.id, edits: review.edits },
            },
          ],
        });
        if (isAlive()) updateOperation(id, result);
      } catch (error) {
        if (isAlive()) setFailure(adoptionFailure(error));
      } finally {
        if (isAlive()) setBusy(false);
      }
    });
  const targetResult = operation?.targets[0];
  const operationFailure = targetResult?.adoptionError ?? undefined;
  const displayedFailure = operationFailure ?? failure;
  const locked = busy || inFlight || queryError !== undefined;
  const optionFor = (target: AdoptTarget) =>
    options?.destinations.find(
      (destination) => JSON.stringify(destination.target) === JSON.stringify(target),
    );
  const choice = (target: AdoptTarget, label: string): ReactElement => {
    const destination = optionFor(target);
    return (
      <div key={JSON.stringify(target)}>
        <button
          type="button"
          className="option"
          disabled={busy || destination?.eligible !== true}
          onClick={() => {
            void prepare(target);
          }}
        >
          {label}
        </button>
        {destination?.eligible === false && (
          <p className="soft adoption-reason">{destination.reason}</p>
        )}
      </div>
    );
  };
  const goBack = (): void => {
    setFailure(undefined);
    change({ ...draft, mode: "options", operationId: null });
  };
  const reopen = (): void => {
    setFailure(undefined);
    setQueryError(undefined);
    change({ ...draft, operationId: null });
  };

  return (
    <Modal
      title={draft.mode === "review" ? t.review : text.orphans.sheetTitle}
      description={draft.mode === "review" ? t.body : t.chooses}
      bottom
      onCancel={onCancel}
      returnFocusTo={returnFocusTo}
      initialFocusRef={
        draft.mode === "review"
          ? serviceRef
          : draft.mode === "new"
            ? nameRef
            : draft.mode === "others"
              ? firstProjectRef
              : optionsFocusRef
      }
    >
      {displayedFailure !== undefined && displayedFailure.code !== "invalidField" && (
        <p className="form-error" role="alert">
          {adoptionMessage(displayedFailure)}
        </p>
      )}
      {draft.mode !== "review" && (
        <>
          {options === undefined && failure === undefined && <p role="status">{t.loading}</p>}
          {failure !== undefined && (
            <button
              type="button"
              className="btn"
              onClick={() => {
                setQueryError(undefined);
                setReload((value) => value + 1);
              }}
            >
              {t.retry}
            </button>
          )}
          {draft.mode === "options" && (
            <div className="stack">
              {view?.project != null &&
                choice({ project: view.project }, text.orphans.asService(view.project))}
              <button
                type="button"
                className="option"
                ref={draft.lastStep === "others" ? optionsFocusRef : undefined}
                disabled={busy || options === undefined}
                onClick={() => {
                  change({ ...draft, mode: "others", lastStep: "others" });
                }}
              >
                {text.orphans.another}
              </button>
              {view?.container != null && choice("shared", text.orphans.shared)}
              {view?.container === null && (
                <div>
                  <button
                    type="button"
                    className="option"
                    ref={draft.lastStep === "new" ? optionsFocusRef : undefined}
                    disabled={busy || optionFor({ new: null })?.eligible !== true}
                    onClick={() => {
                      change({ ...draft, mode: "new", lastStep: "new" });
                    }}
                  >
                    {text.orphans.newProject}
                  </button>
                  {optionFor({ new: null })?.reason && (
                    <p className="soft adoption-reason">{optionFor({ new: null })?.reason}</p>
                  )}
                </div>
              )}
              <button type="button" className="option" disabled={busy} onClick={onIgnore}>
                {text.orphans.ignore}
              </button>
            </div>
          )}
          {draft.mode === "others" && (
            <div className="stack">
              {options?.destinations
                .filter(
                  (destination) =>
                    typeof destination.target === "object" && "project" in destination.target,
                )
                .map((destination) =>
                  choice(
                    destination.target,
                    text.orphans.asService(
                      typeof destination.target === "object"
                        ? (destination.target.project ?? "")
                        : "",
                    ),
                  ),
                )}
            </div>
          )}
          {draft.mode === "new" && (
            <form
              className="stack"
              onSubmit={(event) => {
                event.preventDefault();
                void prepare({ new: draft.name.trim() || null });
              }}
            >
              <label className="soft" htmlFor="project-name">
                {text.orphans.projectName}
              </label>
              <input
                className="input"
                id="project-name"
                ref={nameRef}
                value={draft.name}
                maxLength={63}
                disabled={busy}
                onChange={(event) => {
                  change({ ...draft, name: event.target.value });
                }}
              />
              <button type="submit" className="btn primary-green" disabled={busy}>
                {t.continue}
              </button>
            </form>
          )}
          {busy && <p role="status">{t.preparing}</p>}
        </>
      )}
      {draft.mode === "review" && review !== undefined && (
        <div className="stack adoption-review">
          <dl className="adoption-location">
            <dt>{t.root}</dt>
            <dd>{review.context.root}</dd>
            <dt>{t.manifest}</dt>
            <dd>{review.context.manifest}</dd>
            <dt>{t.port}</dt>
            <dd>{review.context.observedPort ?? t.unknownPort}</dd>
          </dl>
          {review.context.createsManifest && <p className="soft">{t.creates}</p>}
          {review.context.warnings.length > 0 && (
            <ul className="adoption-warnings">
              {review.context.warnings.map((warning, index) => (
                <li key={`${index}:${warning}`}>{warning}</li>
              ))}
            </ul>
          )}
          {operation?.status !== "succeeded" && (
            <form
              className="stack"
              onSubmit={(event) => {
                event.preventDefault();
                void save();
              }}
            >
              <label htmlFor="service-name">{t.name}</label>
              <input
                id="service-name"
                className="input"
                ref={serviceRef}
                autoComplete="off"
                value={review.edits.name}
                maxLength={63}
                disabled={locked || operationId !== undefined}
                aria-invalid={
                  displayedFailure?.code === "invalidField" && displayedFailure.field === "name"
                }
                onChange={(event) => {
                  change({
                    ...draft,
                    review: { ...review, edits: { ...review.edits, name: event.target.value } },
                  });
                }}
              />
              {displayedFailure?.code === "invalidField" && displayedFailure.field === "name" && (
                <p className="form-error">{displayedFailure.message}</p>
              )}
              <label htmlFor="service-start">{t.start}</label>
              <textarea
                id="service-start"
                className="input adoption-command"
                value={review.edits.start ?? ""}
                maxLength={16_384}
                spellCheck={false}
                disabled={locked || operationId !== undefined}
                aria-describedby="service-start-hint"
                aria-invalid={
                  displayedFailure?.code === "invalidField" && displayedFailure.field === "start"
                }
                onChange={(event) => {
                  change({
                    ...draft,
                    review: {
                      ...review,
                      edits: {
                        ...review.edits,
                        start: event.target.value === "" ? null : event.target.value,
                      },
                    },
                  });
                }}
              />
              <p className="soft" id="service-start-hint">
                {t.startHint}
              </p>
              {displayedFailure?.code === "invalidField" && displayedFailure.field === "start" && (
                <p className="form-error">{displayedFailure.message}</p>
              )}
              {operationId === undefined && !needsNewReview(displayedFailure) && (
                <button type="submit" className="btn primary-green" disabled={locked}>
                  {busy ? t.saving : t.save}
                </button>
              )}
            </form>
          )}
          {inFlight && !queryError && (
            <p role="status">{operation === undefined ? t.checking : t.saving}</p>
          )}
          {queryError && (
            <p className="form-error" role="alert">
              {queryError}
            </p>
          )}
          {(queryError !== undefined ||
            (operationId !== undefined && failure !== undefined && operation === undefined)) && (
            <button
              type="button"
              className="btn"
              onClick={() => {
                setQueryError(undefined);
                setReload((value) => value + 1);
              }}
            >
              {t.retryQuery}
            </button>
          )}
          {operation === null && !busy && <p role="status">{t.unsent}</p>}
          {operation?.status === "failed" && (
            <p className="form-error" role="alert">
              {targetResult?.error}
            </p>
          )}
          {(operation === null || operation?.status === "failed") &&
            !needsNewReview(operationFailure) && (
              <button type="button" className="btn" onClick={reopen}>
                {t.editAgain}
              </button>
            )}
          {needsNewReview(displayedFailure) && (
            <button
              type="button"
              className="btn"
              disabled={busy || view === undefined}
              onClick={() => {
                void prepare(review.target, review);
              }}
            >
              {t.newReview}
            </button>
          )}
          {view === undefined && operation?.status !== "succeeded" && (
            <p className="soft">{t.targetMissing}</p>
          )}
          {operation?.status === "succeeded" && targetResult?.result && (
            <div role="status" className="stack">
              <p>
                {text.operations.adopted(targetResult.result.project, targetResult.result.service)}
              </p>
              {(
                review.target === "shared"
                  ? snapshot.instances.some(
                      (candidate) =>
                        candidate.shared && candidate.service === targetResult.result?.service,
                    )
                  : snapshot.projects.some(
                      (project) => project.name === targetResult.result?.project,
                    )
              ) ? (
                <button
                  type="button"
                  className="btn primary-green"
                  onClick={() => {
                    if (targetResult.result)
                      onNavigate(
                        targetResult.result.project,
                        targetResult.result.service,
                        review.target === "shared",
                      );
                  }}
                >
                  {review.target === "shared" ? t.openShared : t.openProject}
                </button>
              ) : (
                <p className="soft">{t.pendingCard}</p>
              )}
            </div>
          )}
        </div>
      )}
      <div className="row adoption-footer">
        {draft.mode !== "options" && operationId === undefined ? (
          <button type="button" className="btn" disabled={busy} onClick={goBack}>
            {text.orphans.back}
          </button>
        ) : (
          <span />
        )}
        <button type="button" className="link-btn" onClick={onCancel}>
          {operation?.status === "succeeded"
            ? t.done
            : operationId !== undefined
              ? t.close
              : text.orphans.cancel}
        </button>
      </div>
    </Modal>
  );
}
