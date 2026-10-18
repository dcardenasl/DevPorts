// How the core's typed reasons read (ADR-041): the core sends a code and its parameters, and
// each phrase is written here, in the language in force.
import type { Catalog } from "./i18n/es";
import type {
  ActionError,
  CommandError,
  Diagnostic,
  DockerProblem,
  FileProblem,
  Problem,
  FieldProblem,
  Ineligibility,
  Limitation,
  ListProblem,
  ReviewNote,
  RootProblemKind,
  StopFailure,
  Unsuitability,
} from "./types";

export function ineligibilityText(reason: Ineligibility, text: Catalog): string {
  const t = text.reasons.ineligibility;
  switch (reason.code) {
    case "configInvalid":
      return t.configInvalid(fileProblemText(reason.problem, text));
    case "noManifest":
      return t.noManifest;
    case "manifestInvalid":
      return t.manifestInvalid(fileProblemText(reason.problem, text));
    case "readOnly":
      return t.readOnly(reason.path);
    case "rootsNotEditable":
      return t.rootsNotEditable;
  }
}

export function limitationText(limitation: Limitation, text: Catalog): string {
  const t = text.reasons.limitation;
  switch (limitation.code) {
    case "tooLarge":
      return t.tooLarge(limitation.max);
    case "form":
      return t.form(limitation.key, limitation.problem);
    case "editInvalid":
      return t.editInvalid(limitation.document, fileProblemText(limitation.problem, text));
    case "editTouchedMore":
      return t.editTouchedMore(limitation.key);
    case "other":
      return t.other(limitation.detail);
    case "notRegularFile":
    case "hardLinked":
    case "notUtf8":
    case "ownerNotKept":
    case "targetNotAbsolute":
    case "noTargetFile":
    case "noTargetFolder":
    case "targetFolderNotDirectory":
    case "targetFolderNotFound":
    case "targetFolderUnresolvable":
      return t[limitation.code];
  }
}

export function unsuitabilityText(reason: Unsuitability, text: Catalog): string {
  const t = text.reasons.unsuitability;
  switch (reason.code) {
    case "folderUnresolvable":
      return t.folderUnresolvable(reason.detail);
    case "fileInvalid":
      return t.fileInvalid(fileProblemText(reason.problem, text));
    case "other":
      return t.other(reason.detail);
    case "instanceUnavailable":
    case "sharedNeedsContainer":
    case "containerNeedsProject":
    case "noRoot":
    case "noKnownFolder":
    case "folderTooBroad":
    case "manifestExists":
    case "outsideProjectRoot":
    case "outOfScanScope":
    case "projectNotInCatalog":
    case "resultTooLarge":
    case "notDescribable":
      return t[reason.code];
  }
}

export function fieldProblemText(problem: FieldProblem, text: Catalog): string {
  return text.reasons.field[problem];
}

export function listProblemText(problem: ListProblem, text: Catalog): string {
  return problem.code === "tooMany"
    ? text.reasons.listProblem.tooMany(problem.max)
    : text.reasons.listProblem.empty;
}

export function rootProblemText(kind: RootProblemKind, text: Catalog): string {
  return text.reasons.rootProblem[kind];
}

export function reviewNoteText(note: ReviewNote, text: Catalog): string {
  const t = text.reasons.reviewNote;
  switch (note.code) {
    case "launcherProgram":
      return t.launcherProgram(note.program);
    case "manifestWarning":
      return t.manifestWarning(diagnosticText(note.diagnostic, text));
    case "notOneLine":
    case "noStartCommand":
      return t[note.code];
  }
}

export function stopFailureText(failure: StopFailure, text: Catalog): string {
  const t = text.reasons.stop;
  switch (failure.code) {
    case "protected":
    case "otherUser":
      return t[failure.code](failure.pid);
    case "failed":
      return t.failed(failure.detail);
    case "stillAlive":
    case "processTableUnreadable":
    case "dockerUnavailable":
      return t[failure.code];
  }
}

export function actionErrorText(error: ActionError, text: Catalog): string {
  const t = text.reasons.action;
  switch (error.code) {
    case "unknownInstance":
    case "notRunning":
      return t[error.code](error.reference);
    case "targetChanged":
      return t.targetChanged;
    case "closing":
      return text.reasons.closing;
    case "noKnownFolder":
    case "tooManyCorrections":
      return t[error.code];
    case "stopFailed":
      return stopFailureText(error.failure, text);
    case "storage":
    case "internal":
      return t[error.code](error.detail);
  }
}

export function commandErrorText(error: CommandError, text: Catalog): string {
  const t = text.reasons.command;
  switch (error.code) {
    case "tooManyTargets":
      return t.tooManyTargets(error.max);
    case "invalidPanelState":
      return t.invalidPanelState(error.detail);
    case "action":
      return actionErrorText(error.error, text);
    case "io":
      return text.reasons.io(error.detail);
    case "onlyLocalUrls":
    case "operationInvalid":
    case "operationIdConflict":
    case "operationAtCapacity":
    case "operationNotFound":
    case "nothingToRetry":
      return t[error.code];
  }
}

const COMMAND_CODES = [
  "tooManyTargets",
  "onlyLocalUrls",
  "invalidPanelState",
  "operationInvalid",
  "operationIdConflict",
  "operationAtCapacity",
  "operationNotFound",
  "nothingToRetry",
  "action",
  "io",
];
const ACTION_CODES = [
  "unknownInstance",
  "notRunning",
  "targetChanged",
  "closing",
  "noKnownFolder",
  "tooManyCorrections",
  "stopFailed",
  "storage",
  "internal",
];

/**
 * A rejected call as text. A typed error from the shell or the core is written in the
 * language in force; anything else (a lost connection, a script error) says what it says.
 */
export function failureText(failure: unknown, text: Catalog): string {
  if (typeof failure === "object" && failure !== null && "code" in failure) {
    const code = failure.code;
    if (typeof code === "string") {
      if (COMMAND_CODES.includes(code)) return commandErrorText(failure as CommandError, text);
      if (ACTION_CODES.includes(code)) return actionErrorText(failure as ActionError, text);
    }
  }
  return failure instanceof Error ? failure.message : String(failure);
}

/** One problem of a file: its line, when it has one, and what is wrong. */
export function diagnosticText(diagnostic: Diagnostic, text: Catalog): string {
  const message = text.reasons.diagnostic(diagnostic.kind);
  return diagnostic.line === null ? message : text.reasons.atLine(diagnostic.line, message);
}

export function fileProblemText(problem: FileProblem, text: Catalog): string {
  const t = text.reasons.file;
  if (problem.code === "unreadable") return t.unreadable(problem.path, problem.detail);
  const errors = problem.diagnostics
    .filter((diagnostic) => diagnostic.severity === "error")
    .map((diagnostic) => diagnosticText(diagnostic, text));
  return t.invalid(problem.path, errors.join("; "));
}

/** A warning of the core, as the strip and the settings sheet list it. */
export function problemText(problem: Problem, text: Catalog): string {
  const t = text.reasons.problem;
  switch (problem.code) {
    case "folderUnreadable":
      return t.folderUnreadable(problem.path, problem.detail);
    case "manifestUnusable":
      return fileProblemText(problem.problem, text);
    case "manifestWarnings": {
      const first = problem.diagnostics[0];
      return `${problem.path}: ${first === undefined ? "" : diagnosticText(first, text)}`;
    }
    case "duplicateProjectName":
      return t.duplicateProjectName(problem.name, problem.roots.join(", "));
    case "activityLogWrite":
    case "activityLogPrune":
      return t[problem.code](problem.detail);
    case "catalogNotRefreshed":
      return t.catalogNotRefreshed;
    case "instanceNotSaved":
      return t.instanceNotSaved(problem.reference, problem.detail);
  }
}

export function dockerProblemText(problem: DockerProblem, text: Catalog): string {
  return problem.code === "connectionLost"
    ? text.reasons.docker.connectionLost
    : text.reasons.docker.unreachable(problem.detail);
}
