import type { ReactElement } from "react";

import type { Link } from "./connection";
import { useI18n } from "./i18n/provider";
import { dockerProblemText, problemText } from "./reasons";
import { agoFrom } from "./time";
import { Dot } from "./views/shared-parts";
import type { Snapshot } from "./types";

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
  const { text, locale } = useI18n();
  const t = text.link;
  const since = agoFrom(link.receivedAt, now, locale);
  const dockerDown = !snapshot.docker_reachable && snapshot.docker_error !== null;
  const warnings = snapshot.problems.length;
  if (link.phase === "ready" && !dockerDown && warnings === 0) return null;
  return (
    <>
      {(link.phase === "reconnecting" || link.phase === "failed") && (
        <section className="group" role={link.phase === "failed" ? "alert" : "status"}>
          <div className="item">
            <Dot kind="red" />
            <div className="item-main">
              <div className="item-sub">
                {link.phase === "failed"
                  ? t.lost(since)
                  : t.reconnecting(link.attempt, link.attempts, since)}
              </div>
            </div>
          </div>
          <div className="item-actions">
            <button type="button" className="btn" onClick={onRetry}>
              {link.phase === "failed" ? t.retry : t.retryNow}
            </button>
          </div>
        </section>
      )}
      {dockerDown && (
        <section className="group" role="status">
          <div className="item">
            <Dot kind="down" />
            <div className="item-main">
              <div className="item-title">{text.status.dockerDown}</div>
              <div className="item-sub">{text.status.dockerDownBody}</div>
              {snapshot.docker_error !== null && (
                <div className="item-sub mono">
                  {shortReason(dockerProblemText(snapshot.docker_error, text))}
                </div>
              )}
            </div>
          </div>
        </section>
      )}
      {warnings > 0 && (
        <section className="group" role="status">
          <div className="item">
            <Dot kind="amber" />
            <span className="grow">{t.problems(warnings)}</span>
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
        </section>
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
  const { text } = useI18n();
  const t = text.link;
  return (
    <section className="stack" aria-label={t.state}>
      <div className="label soft">{t.state}</div>
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
            : t.dockerDown(shortReason(dockerProblemText(snapshot.docker_error, text)))}
        </dd>
      </dl>
      <div className="label soft">{t.problemsTitle}</div>
      {snapshot.problems.length === 0 ? (
        <p className="soft">{t.noProblems}</p>
      ) : (
        <ul className="soft evidence">
          {snapshot.problems.map((problem) => {
            const sentence = problemText(problem, text);
            return <li key={sentence}>{sentence}</li>;
          })}
        </ul>
      )}
    </section>
  );
}
