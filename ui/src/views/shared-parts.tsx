import type { ReactElement, ReactNode } from "react";

import { text } from "../text";
import type { InstanceState } from "../types";

export function Dot({
  kind,
}: {
  kind: "green" | "coral" | "amber" | "down" | "loose";
}): ReactElement {
  return <span className={kind === "green" ? "dot" : `dot ${kind}`} aria-hidden="true" />;
}

export function dotFor(state: InstanceState): "green" | "coral" | "down" {
  if (state === "unhealthy") return "coral";
  if (state === "down" || state === "forgotten") return "down";
  return "green";
}

export function stateLabel(state: InstanceState): string {
  switch (state) {
    case "unhealthy":
      return text.projects.hung;
    case "down":
      return text.projects.down;
    case "starting":
      return text.projects.starting;
    case "forgotten":
      return text.projects.forgotten;
    case "stopping":
      return "Deteniendo";
    case "healthy":
      return text.projects.principal;
  }
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

/** The program and arguments of a process, as one line. */
export function commandLine(argv: readonly string[], container: string | null): string {
  if (container !== null) return container;
  return argv.length === 0 ? "—" : argv.join(" ");
}
