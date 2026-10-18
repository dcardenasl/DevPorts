import { useEffect, useRef, useState, type ReactElement } from "react";

import type { Backend } from "./api";
import { Criterion } from "./IgnoreSheets";
import { Modal } from "./Modal";
import { text } from "./text";
import type { Classification, IgnoreRule, ProcessClass } from "./types";

const t = text.classify;

type Choice = ProcessClass | "auto";

const label = (class_: ProcessClass): string =>
  class_ === "project" ? t.project : class_ === "auxiliary" ? t.auxiliary : t.unclassified;

/**
 * Corrects where one loose process is listed. It shows the evidence and the exact
 * criterion the correction is remembered under; it never ignores or stops anything.
 */
export function ClassifySheet({
  api,
  reference,
  classification,
  onCancel,
  onSave,
  returnFocusTo,
}: {
  api: Backend;
  reference: string;
  classification: Classification | undefined;
  onCancel: () => void;
  onSave: (choice: ProcessClass | null) => void;
  returnFocusTo: HTMLElement | null;
}): ReactElement {
  const detected = classification?.detected ?? "unclassified";
  const [choice, setChoice] = useState<Choice>(
    classification?.corrected === true ? classification.class : "auto",
  );
  const [rule, setRule] = useState<IgnoreRule>();
  const [failure, setFailure] = useState<string>();
  const firstRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    let active = true;
    void api.rulePreview(reference).then(
      (next) => {
        if (active) setRule(next);
      },
      (error: unknown) => {
        if (active) setFailure(error instanceof Error ? error.message : String(error));
      },
    );
    return () => {
      active = false;
    };
  }, [api, reference]);
  const options: [Choice, string][] = [
    ["auto", t.autoNow(label(detected))],
    ["project", t.project],
    ["auxiliary", t.auxiliary],
    ["unclassified", t.unclassified],
  ];
  return (
    <Modal
      title={t.title}
      description={t.body}
      bottom
      onCancel={onCancel}
      returnFocusTo={returnFocusTo}
      initialFocusRef={firstRef}
    >
      <div className="stack">
        <p className="soft">
          {t.current}: <strong>{label(classification?.class ?? "unclassified")}</strong>
          {classification?.origin ? ` · ${t.origin(classification.origin)}` : ""}
        </p>
        <fieldset className="choices">
          <legend className="eyebrow">{t.choose}</legend>
          {options.map(([value, caption], index) => (
            <label key={value} className="consumer">
              <input
                ref={index === 0 ? firstRef : undefined}
                type="radio"
                name="classification"
                checked={choice === value}
                onChange={() => {
                  setChoice(value);
                }}
              />
              <span>{caption}</span>
            </label>
          ))}
        </fieldset>
        <p className="soft">{t.scope}</p>
        <div className="eyebrow">{t.evidence}</div>
        {classification === undefined || classification.signals.length === 0 ? (
          <p className="soft">{t.noEvidence}</p>
        ) : (
          <ul className="soft evidence">
            {classification.signals.map((signal) => (
              <li key={signal}>{t.signals[signal] ?? signal}</li>
            ))}
          </ul>
        )}
        {failure !== undefined && (
          <p className="form-error" role="alert">
            {failure}
          </p>
        )}
        {rule !== undefined && <Criterion rule={rule} />}
      </div>
      <div className="row adoption-footer">
        <button type="button" className="btn" onClick={onCancel}>
          {text.orphans.cancel}
        </button>
        <button
          type="button"
          className="btn primary-green"
          disabled={rule === undefined}
          onClick={() => {
            onSave(choice === "auto" ? null : choice);
          }}
        >
          {t.save}
        </button>
      </div>
    </Modal>
  );
}
