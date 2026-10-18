import { useEffect, useRef, useState, type ReactElement } from "react";

import type { Backend } from "./api";
import { commandLine } from "./instance";
import { useMounted, useSingleFlight } from "./hooks";
import { Modal } from "./Modal";
import { useI18n } from "./i18n/provider";
import type { IgnoreRule } from "./types";

const failureText = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/** The criterion of a rule, shown the same way before it is saved and when it is listed. */
export function Criterion({ rule }: { rule: IgnoreRule }): ReactElement {
  const { text } = useI18n();
  const t = text.ignore;
  return (
    <dl className="adoption-location">
      {rule.folder !== null && (
        <>
          <dt>{t.folder}</dt>
          <dd className="mono">{rule.folder}</dd>
        </>
      )}
      {rule.command.length > 0 && (
        <>
          <dt>{t.command}</dt>
          <dd className="mono">{commandLine(rule.command, text)}</dd>
        </>
      )}
      {rule.container !== null && (
        <>
          <dt>{t.container}</dt>
          <dd className="mono">{rule.container}</dd>
        </>
      )}
      {rule.context !== null && (
        <>
          <dt>{t.context}</dt>
          <dd className="mono">{rule.context}</dd>
        </>
      )}
    </dl>
  );
}

/** Shows the exact rule and ignores only after an explicit confirmation. */
export function IgnoreConfirm({
  api,
  reference,
  onCancel,
  onConfirm,
  returnFocusTo,
}: {
  api: Backend;
  reference: string;
  onCancel: () => void;
  onConfirm: () => void;
  returnFocusTo: HTMLElement | null;
}): ReactElement {
  const { text } = useI18n();
  const t = text.ignore;
  const [rule, setRule] = useState<IgnoreRule>();
  const [failure, setFailure] = useState<string>();
  const cancelRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    let active = true;
    void api.rulePreview(reference).then(
      (next) => {
        if (active) setRule(next);
      },
      (error: unknown) => {
        if (active) setFailure(failureText(error));
      },
    );
    return () => {
      active = false;
    };
  }, [api, reference]);
  return (
    <Modal
      title={t.title}
      description={t.body}
      bottom
      onCancel={onCancel}
      returnFocusTo={returnFocusTo}
      initialFocusRef={cancelRef}
    >
      {rule === undefined && failure === undefined && <p role="status">{t.loading}</p>}
      {failure !== undefined && (
        <p className="form-error" role="alert">
          {t.failed} {failure}
        </p>
      )}
      {rule !== undefined && (
        <div className="stack">
          <div className="eyebrow">{t.criterion}</div>
          <Criterion rule={rule} />
          <p className="soft">{rule.kind === "container" ? t.containerScope : t.scope}</p>
          <p className="soft">{t.restore}</p>
        </div>
      )}
      <div className="row adoption-footer">
        <button type="button" className="btn" ref={cancelRef} onClick={onCancel}>
          {text.orphans.cancel}
        </button>
        <button
          type="button"
          className="btn primary-green"
          disabled={rule === undefined}
          onClick={onConfirm}
        >
          {t.confirm}
        </button>
      </div>
    </Modal>
  );
}

/** Every saved rule, including those no running process matches, each restorable. */
export function IgnoredList({
  api,
  revision,
  onRestore,
  onClose,
  returnFocusTo,
}: {
  api: Backend;
  /** Changes when the shell finished an operation, so the list is read again. */
  revision: number;
  onRestore: (id: string) => Promise<void>;
  onClose: () => void;
  returnFocusTo: HTMLElement | null;
}): ReactElement {
  const { text } = useI18n();
  const t = text.ignore;
  const [rules, setRules] = useState<IgnoreRule[]>();
  const [loadFailure, setLoadFailure] = useState<string>();
  const [restoreFailure, setRestoreFailure] = useState<string>();
  const [pending, setPending] = useState<string>();
  const [reads, setReads] = useState(0);
  const closeRef = useRef<HTMLButtonElement>(null);
  const isAlive = useMounted();
  const exclusive = useSingleFlight();
  useEffect(() => {
    let active = true;
    void api.ignoredRules().then(
      (next) => {
        if (!active) return;
        setRules(next);
        setLoadFailure(undefined);
      },
      (error: unknown) => {
        if (active) setLoadFailure(failureText(error));
      },
    );
    return () => {
      active = false;
    };
  }, [api, revision, reads]);
  const restore = (id: string): Promise<void> =>
    exclusive(async () => {
      setPending(id);
      setRestoreFailure(undefined);
      try {
        await onRestore(id);
        if (isAlive()) setReads((value) => value + 1);
      } catch (error) {
        if (isAlive()) setRestoreFailure(failureText(error));
      } finally {
        if (isAlive()) setPending(undefined);
      }
    });
  return (
    <Modal
      title={t.listTitle}
      description={t.listBody}
      bottom
      onCancel={onClose}
      returnFocusTo={returnFocusTo}
      initialFocusRef={closeRef}
    >
      {(restoreFailure ?? loadFailure) !== undefined && (
        <p className="form-error" role="alert">
          {restoreFailure ?? loadFailure}
        </p>
      )}
      {rules === undefined && loadFailure === undefined && (
        <p role="status">{text.consumers.loading}</p>
      )}
      {rules?.length === 0 && <p role="status">{t.none}</p>}
      {rules !== undefined && rules.length > 0 && (
        <ul className="consumers ignored-rules" aria-label={t.listTitle}>
          {rules.map((rule) => (
            <li key={rule.id} className="card">
              {rule.kind === "unknown" ? (
                <p className="soft">{t.unknown}</p>
              ) : (
                <Criterion rule={rule} />
              )}
              {rule.legacy && (
                <p className="soft">
                  <span className="tag amber">{t.legacy}</span> {t.legacyBody}
                </p>
              )}
              {rule.createdAt !== null && (
                <p className="soft">
                  {t.since} {rule.createdAt}
                </p>
              )}
              <div className="row" style={{ justifyContent: "flex-end" }}>
                <button
                  type="button"
                  className="btn"
                  disabled={pending !== undefined}
                  onClick={() => {
                    void restore(rule.id);
                  }}
                >
                  {pending === rule.id ? t.restoring : t.restoreAction}
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
      <div className="row adoption-footer">
        <span />
        <button type="button" className="link-btn" ref={closeRef} onClick={onClose}>
          {t.close}
        </button>
      </div>
    </Modal>
  );
}
