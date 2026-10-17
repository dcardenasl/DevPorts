// Presentation only: lifecycle and process facts always come from the core.
import { text } from "./text";
import type { InstanceState, InstanceView } from "./types";

export function stateLabel(state: InstanceState): string {
  return text.instance.states[state];
}

export function dotFor(state: InstanceState): "green" | "coral" | "amber" | "down" | "loose" {
  switch (state) {
    case "healthy":
      return "green";
    case "unhealthy":
      return "coral";
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
export function instanceTitle(view: InstanceView): string {
  return (
    view.service ?? view.container ?? executableName(view.argv) ?? text.instance.unknownProgram
  );
}

function executableName(argv: readonly string[]): string | undefined {
  return argv[0]?.split("/").filter(Boolean).at(-1);
}

/** Diagnostic command shape, never raw argument values: even positional secrets
 * and shell scripts are masked rather than guessing which flags are sensitive. */
export function commandLine(argv: readonly string[]): string {
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
