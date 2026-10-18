import type { ReactElement, ReactNode } from "react";

export { dotFor, stateLabel } from "../instance";

/** The one carrier of status colour: a dot, a ring (in transition) or a dashed ring (no owner). */
export function Dot({
  kind,
}: {
  kind: "green" | "red" | "amber" | "down" | "loose";
}): ReactElement {
  return <span className={kind === "green" ? "dot" : `dot ${kind}`} aria-hidden="true" />;
}

export function Empty({ title, body }: { title: string; body: string }): ReactElement {
  return (
    <div className="empty">
      <strong>{title}</strong>
      {body}
    </div>
  );
}

/** A neutral datum: a port, a branch, a name. Status never colours it. */
export function Tag({ children }: { children: ReactNode }): ReactElement {
  return <span className="tag">{children}</span>;
}

/** A port with the health of whoever holds it: the dot is the only colour. */
export function PortChip({
  port,
  tone,
}: {
  port: number;
  tone: "green" | "red" | "amber";
}): ReactElement {
  return (
    <span className="tag chip">
      <Dot kind={tone} />:{port}
    </span>
  );
}

/**
 * A titled group of rows. The title may be a state the catalogue writes in capitals; it reads
 * as a sentence, and it is coloured only when the group asks for an action.
 */
export function Section({
  title,
  tone,
  children,
}: {
  title?: string;
  tone?: "amber" | "red";
  children: ReactNode;
}): ReactElement {
  return (
    <section className="section">
      {title !== undefined && (
        <h2
          className={
            tone === undefined ? "section-title sentence" : `section-title sentence is-${tone}`
          }
        >
          {title}
        </h2>
      )}
      <div className="group">{children}</div>
    </section>
  );
}
