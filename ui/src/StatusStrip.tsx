import type { ReactElement } from "react";

import type { Link } from "./connection";
import { text } from "./text";
import { agoFrom } from "./time";
import type { Snapshot } from "./types";

const t = text.link;

/** Longest docker error shown; the full text can carry local paths. */
const REASON_LIMIT = 160;

export const shortReason = (reason: string): string =>
  reason.length > REASON_LIMIT ? `${reason.slice(0, REASON_LIMIT)}…` : reason;

/**
 * What is wrong around the panel, on every tab: the link to the core, Docker, and
 * configuration or manifest warnings. Each says what it means and what to do; none
 * replaces the screen.
 */
export function StatusStrip({
  link,
  snapshot,
  now,
  onRetry,
  onDetails,
}: {
  link: Link;
  snapshot: Snapshot;
  now: number;
  onRetry: () => void;
  onDetails: (invoker: HTMLButtonElement) => void;
}): ReactElement | null {
  const since = agoFrom(link.receivedAt, now);
  const dockerDown = !snapshot.docker_reachable && snapshot.docker_error !== null;
  const warnings = snapshot.problems.length;
  if (link.phase === "ready" && !dockerDown && warnings === 0) return null;
  return (
    <>
      {(link.phase === "reconnecting" || link.phase === "failed") && (
        <div className="card coral" role={link.phase === "failed" ? "alert" : "status"}>
          <div className="soft">
            {link.phase === "failed"
              ? t.lost(since)
              : t.reconnecting(link.attempt, link.attempts, since)}
          </div>
          <div className="row" style={{ marginTop: 8, justifyContent: "flex-end" }}>
            <button type="button" className="btn" onClick={onRetry}>
              {link.phase === "failed" ? t.retry : t.retryNow}
            </button>
          </div>
        </div>
      )}
      {dockerDown && (
        <div className="card blue" role="status">
          <div className="eyebrow blue-text">{text.status.dockerDown}</div>
          <div className="soft">{text.status.dockerDownBody}</div>
          {snapshot.docker_error !== null && (
            <div className="soft mono" style={{ marginTop: 4, fontSize: 11 }}>
              {shortReason(snapshot.docker_error)}
            </div>
          )}
        </div>
      )}
      {warnings > 0 && (
        <div className="card blue" role="status">
          <div className="row">
            <span className="soft grow">{t.problems(warnings)}</span>
            <button
              type="button"
              className="btn"
              onClick={(event) => {
                onDetails(event.currentTarget);
              }}
            >
              {t.details}
            </button>
          </div>
        </div>
      )}
    </>
  );
}

/** The state section of the settings sheet: link, Docker and every warning in full. */
export function StatusDetails({
  link,
  snapshot,
}: {
  link: Link;
  snapshot: Snapshot;
}): ReactElement {
  return (
    <section className="stack" aria-label={t.state}>
      <div className="eyebrow">{t.state}</div>
      <dl className="adoption-location">
        <dt>{t.core}</dt>
        <dd>
          {link.phase === "ready"
            ? t.coreOk
            : link.phase === "failed"
              ? t.coreLost
              : t.coreRetrying}
        </dd>
        <dt>{t.docker}</dt>
        <dd>
          {snapshot.docker_reachable || snapshot.docker_error === null
            ? t.dockerOk
            : t.dockerDown(shortReason(snapshot.docker_error))}
        </dd>
      </dl>
      <div className="eyebrow">{t.problemsTitle}</div>
      {snapshot.problems.length === 0 ? (
        <p className="soft">{t.noProblems}</p>
      ) : (
        <ul className="soft evidence">
          {snapshot.problems.map((problem) => (
            <li key={problem}>{problem}</li>
          ))}
        </ul>
      )}
    </section>
  );
}
