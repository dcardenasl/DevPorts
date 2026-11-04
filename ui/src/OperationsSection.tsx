import type { ReactElement } from "react";

import { operationFailureText } from "./failures";
import { useI18n } from "./i18n/provider";
import { targetLabel } from "./instance";
import type { InstanceView, OperationView } from "./types";

/** The operations that are pending, failed or just finished, with a retry for the ones that did not complete. */
export function OperationsSection({
  visible,
  instances,
  onRetry,
}: {
  visible: OperationView[];
  instances: InstanceView[];
  onRetry: (id: string) => void;
}): ReactElement | null {
  const { text } = useI18n();
  if (visible.length === 0) return null;
  return (
    <section className="section" aria-label={text.a11y.operations}>
      <h2 className="section-title">{text.operations.title}</h2>
      <div className="group">
        {visible.slice(0, 5).map((operation) => (
          <div key={operation.id} className="item-note">
            <strong>{text.operations.kind[operation.kind]}</strong>
            {operation.targets.map((target) => (
              <div key={target.reference} role={target.status === "failed" ? "alert" : "status"}>
                {targetLabel(target, instances, text)}:{" "}
                {target.status === "queued"
                  ? text.operations.queued
                  : target.status === "running"
                    ? text.operations.running
                    : target.status === "succeeded"
                      ? target.result
                        ? text.operations.adopted(target.result.project, target.result.service)
                        : text.operations.succeeded
                      : operationFailureText(target, text)}
              </div>
            ))}
            {(operation.status === "failed" || operation.status === "partial") && (
              <div>
                <button
                  type="button"
                  className="btn"
                  onClick={() => {
                    onRetry(operation.id);
                  }}
                >
                  {text.operations.retry}
                </button>
              </div>
            )}
          </div>
        ))}
      </div>
    </section>
  );
}
