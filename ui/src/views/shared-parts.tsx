import type { ReactElement, ReactNode } from "react";

export { dotFor, stateLabel } from "../instance";

export function Dot({
  kind,
}: {
  kind: "green" | "coral" | "amber" | "down" | "loose";
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

export function Tag({
  tone,
  children,
}: {
  tone?: "blue" | "green" | "amber" | "coral";
  children: ReactNode;
}): ReactElement {
  return <span className={tone === undefined ? "tag" : `tag ${tone}`}>{children}</span>;
}
