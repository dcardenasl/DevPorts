// Presentation only: lifecycle and process facts always come from the core.
import type { Catalog } from "./i18n/es";
import type { InstanceState, InstanceView, OperationTargetResult } from "./types";

export function stateLabel(state: InstanceState, text: Catalog): string {
  return text.instance.states[state];
}

export function dotFor(state: InstanceState): "green" | "red" | "amber" | "down" | "loose" {
  switch (state) {
    case "healthy":
      return "green";
    case "unhealthy":
      return "red";
    case "starting":
    case "stopping":
    case "forgotten":
      return "amber";
    case "unknown":
      return "loose";
    case "down":
      return "down";
  }
}

/** A known observed target; unknown is never treated as a stopped instance. */
export function canRequestStop(view: InstanceView | undefined): boolean {
  return (
    view !== undefined &&
    view.target_identity !== null &&
    ["healthy", "unhealthy", "forgotten"].includes(view.state)
  );
}

/** Observed lifecycle activity, including an in-flight stop; unknown is uncertain. */
export function isActiveInstance(view: InstanceView | undefined): boolean {
  return (
    view !== undefined && ["healthy", "unhealthy", "forgotten", "stopping"].includes(view.state)
  );
}

/** Short executable title, with arguments kept out of headings and tooltips. */
export function instanceTitle(view: InstanceView, text: Catalog): string {
  return (
    view.service ?? view.container ?? executableName(view.argv) ?? text.instance.unknownProgram
  );
}

/**
 * The program's name. A daemon that rewrites its title (`php-fpm: master process
 * (/opt/homebrew/…/php-fpm.conf)`) is named by what precedes the colon, not by the last
 * path segment inside its description.
 */
function executableName(argv: readonly string[]): string | undefined {
  const program = argv[0];
  if (program === undefined) return undefined;
  const title = program.split(": ")[0];
  return title !== undefined && title !== program
    ? title
    : program.split("/").filter(Boolean).at(-1);
}

/** Diagnostic command shape, never raw argument values: even positional secrets
 * and shell scripts are masked rather than guessing which flags are sensitive. */
export function commandLine(argv: readonly string[], text: Catalog): string {
  const program = executableName(argv);
  if (program === undefined) return text.instance.unavailable;
  return [
    program,
    ...argv.slice(1).map((argument) => {
      const option = argument.match(/^(--[a-zA-Z][a-zA-Z0-9-]*)(?:=([\s\S]*))?$/);
      if (option !== null)
        return option[2] === undefined ? option[1] : `${option[1]}=${text.instance.hidden}`;
      return text.instance.hidden;
    }),
  ].join(" ");
}

/** What an operation result calls its target: the service and port, not an opaque reference. */
export function targetLabel(
  target: OperationTargetResult,
  instances: readonly InstanceView[],
  text: Catalog,
): string {
  const expected = target.stopExpectation;
  const live = instances.find((view) => view.ref === target.reference);
  const name =
    expected?.service ??
    expected?.container ??
    (live === undefined ? null : instanceTitle(live, text));
  if (name === null) return target.reference;
  const port = expected?.port ?? live?.port ?? null;
  return port === null ? name : `${name} (:${port})`;
}
