import { describe, expect, it } from "vitest";

import { adoptionMessage } from "./adoption";
import { usesMessage } from "./consumers";
import { operationFailureText } from "./failures";
import { en } from "./i18n/en";
import { es } from "./i18n/es";
import {
  actionErrorText,
  commandErrorText,
  diagnosticText,
  dockerProblemText,
  fileProblemText,
  problemText,
  failureText,
  fieldProblemText,
  ineligibilityText,
  limitationText,
  listProblemText,
  reviewNoteText,
  rootProblemText,
  stopFailureText,
  unsuitabilityText,
} from "./reasons";
import { settingsMessage } from "./settings";
import type {
  ActionError,
  AdoptionError,
  CommandError,
  Diagnostic,
  DiagnosticKind,
  DockerProblem,
  FieldProblem,
  FileProblem,
  FormKey,
  FormProblem,
  Ineligibility,
  Limitation,
  ListProblem,
  OperationTargetResult,
  Problem,
  ReviewNote,
  RootProblemKind,
  SettingsError,
  StopFailure,
  Unsuitability,
  UsesError,
} from "./types";

const catalogs = [
  ["es", es],
  ["en", en],
] as const;

// One sample of every code the core can send. Adding a code to a type fails the compiler
// here (the arrays are checked against the unions), so a code cannot go without its sentence.
const kinds: DiagnosticKind[] = [
  { code: "syntax", detail: "d" },
  { code: "unknownKey", key: "k" },
  { code: "testIgnored" },
  { code: "versionMissing" },
  { code: "unsupportedVersion", version: "2" },
  { code: "versionMustBeOne" },
  { code: "projectMissing" },
  { code: "invalidProjectName", name: "N" },
  { code: "unknownShared", name: "mysql" },
  { code: "servicesMissing" },
  { code: "invalidServiceName", name: "N" },
  { code: "cwdAndDocker", service: "api" },
  { code: "noCwdNorDocker", service: "api" },
  { code: "emptyDocker", service: "api" },
  { code: "emptyCwd", service: "api" },
  { code: "cwdNotAbsolute", service: "api" },
  { code: "cwdAbsolute", service: "api", cwd: "/x" },
  { code: "cwdEscapes", service: "api", cwd: "../x" },
  { code: "startMultiline", service: "api" },
  { code: "portOutOfRange", service: "api", port: "70000" },
  { code: "portPrivileged", service: "api", port: 80 },
  { code: "portTaken", port: 8080, other: "web" },
  { code: "invalidHealth", service: "api", health: "ping" },
  { code: "invalidKind", service: "api", kind: "x" },
  { code: "emptyMatch", service: "api" },
  { code: "invalidDuration", key: "lease.default", detail: "d" },
  { code: "maxDepthRange", value: "9" },
  { code: "retentionRange", value: "0" },
  { code: "maxPerServiceRange", value: "20" },
  { code: "hostIntervalRange" },
  { code: "temporalPortRange" },
  { code: "leaseDefaultOverMax" },
  { code: "leaseExtendOverMax" },
  { code: "leaseWarnOverDefault" },
];
const diagnostics: Diagnostic[] = kinds.map((kind, index) => ({
  severity: index % 2 === 0 ? "error" : "warning",
  line: index % 3 === 0 ? null : index,
  kind,
}));
const file: FileProblem = { code: "invalid", path: "/p/.devports.yml", diagnostics };
const problemsOfTheCore: Problem[] = [
  { code: "folderUnreadable", path: "/p", detail: "d" },
  { code: "manifestUnusable", problem: file },
  { code: "manifestUnusable", problem: { code: "unreadable", path: "/p", detail: "d" } },
  { code: "manifestWarnings", path: "/p", diagnostics },
  { code: "manifestWarnings", path: "/p", diagnostics: [] },
  { code: "duplicateProjectName", name: "n", roots: ["/a", "/b"] },
  { code: "activityLogWrite", detail: "d" },
  { code: "activityLogPrune", detail: "d" },
  { code: "catalogNotRefreshed" },
  { code: "instanceNotSaved", reference: "r", detail: "d" },
];
const dockers: DockerProblem[] = [{ code: "connectionLost" }, { code: "unreachable", detail: "d" }];
const ineligibility: Ineligibility[] = [
  { code: "configInvalid", problem: file },
  { code: "noManifest" },
  { code: "manifestInvalid", problem: file },
  { code: "readOnly", path: "/p" },
  { code: "rootsNotEditable" },
];
const keys: FormKey[] = ["roots", "uses", "services", "project"];
const problems: FormProblem[] = [
  "notFound",
  "notAList",
  "notSimpleList",
  "multiLine",
  "mixedIndent",
  "emptyItem",
  "invalidValue",
  "notEditable",
];
const limitations: Limitation[] = [
  { code: "notRegularFile" },
  { code: "hardLinked" },
  { code: "notUtf8" },
  { code: "tooLarge", max: 262144 },
  { code: "ownerNotKept" },
  { code: "targetNotAbsolute" },
  { code: "noTargetFile" },
  { code: "noTargetFolder" },
  { code: "targetFolderNotDirectory" },
  { code: "targetFolderNotFound" },
  { code: "targetFolderUnresolvable" },
  ...keys.flatMap((key) => problems.map((problem) => ({ code: "form" as const, key, problem }))),
  { code: "editInvalid", document: "config", problem: file },
  { code: "editInvalid", document: "manifest", problem: file },
  { code: "editTouchedMore", key: "uses" },
  { code: "other", detail: "d" },
];
const unsuitabilities: Unsuitability[] = [
  { code: "instanceUnavailable" },
  { code: "sharedNeedsContainer" },
  { code: "containerNeedsProject" },
  { code: "folderUnresolvable", detail: "d" },
  { code: "noRoot" },
  { code: "noKnownFolder" },
  { code: "folderTooBroad" },
  { code: "manifestExists" },
  { code: "outsideProjectRoot" },
  { code: "outOfScanScope" },
  { code: "projectNotInCatalog" },
  { code: "resultTooLarge" },
  { code: "notDescribable" },
  { code: "fileInvalid", problem: file },
  { code: "other", detail: "d" },
];
const fields: FieldProblem[] = ["invalidName", "invalidStart", "nameTaken", "tooLong"];
const lists: ListProblem[] = [{ code: "empty" }, { code: "tooMany", max: 32 }];
const roots: RootProblemKind[] = [
  "empty",
  "unsafeText",
  "notAbsolute",
  "tooBroad",
  "duplicate",
  "missing",
];
const notes: ReviewNote[] = [
  { code: "launcherProgram", program: "npm" },
  { code: "notOneLine" },
  { code: "noStartCommand" },
  { code: "manifestWarning", diagnostic: diagnostics[0] as Diagnostic },
];
const stops: StopFailure[] = [
  { code: "protected", pid: 1 },
  { code: "otherUser", pid: 2 },
  { code: "stillAlive" },
  { code: "processTableUnreadable" },
  { code: "dockerUnavailable" },
  { code: "failed", detail: "d" },
];
const actions: ActionError[] = [
  { code: "unknownInstance", reference: "r" },
  { code: "notRunning", reference: "r" },
  { code: "targetChanged" },
  { code: "closing" },
  { code: "noKnownFolder" },
  { code: "tooManyCorrections" },
  ...stops.map((failure) => ({ code: "stopFailed" as const, failure })),
  { code: "storage", detail: "d" },
  { code: "internal", detail: "d" },
];
const commands: CommandError[] = [
  { code: "tooManyTargets", max: 64 },
  { code: "onlyLocalUrls" },
  { code: "invalidPanelState", detail: "d" },
  { code: "operationInvalid" },
  { code: "operationIdConflict" },
  { code: "operationAtCapacity" },
  { code: "operationNotFound" },
  { code: "nothingToRetry" },
  ...actions.map((error) => ({ code: "action" as const, error })),
  { code: "io", detail: "d" },
];
const settings: SettingsError[] = [
  ...ineligibility.map((reason) => ({ code: "ineligible" as const, reason })),
  { code: "invalidRoots", problems: [{ index: 0, kind: "empty" }] },
  ...lists.map((reason) => ({ code: "invalidList" as const, reason })),
  { code: "documentChanged", path: "/c" },
  ...limitations.map((limitation) => ({ code: "unsupported" as const, limitation })),
  { code: "io", detail: "d" },
];
const uses: UsesError[] = [
  { code: "unknownShared", name: "n" },
  { code: "unknownProject", project: "p" },
  ...ineligibility.map((reason) => ({ code: "ineligible" as const, reason })),
  { code: "documentChanged", path: "/m" },
  ...limitations.map((limitation) => ({ code: "unsupported" as const, limitation })),
  { code: "blocked" },
  { code: "invalidRequest" },
  { code: "closing" },
  { code: "io", detail: "d" },
];
const adoptions: AdoptionError[] = [
  ...unsuitabilities.map((reason) => ({ code: "unsuitable" as const, reason })),
  ...fields.map((problem) => ({ code: "invalidField" as const, field: "name", problem })),
  { code: "documentChanged", path: "/m" },
  { code: "targetChanged" },
  { code: "previewExpired" },
  { code: "requestChanged" },
  { code: "atCapacity" },
  ...limitations.map((limitation) => ({ code: "unsupported" as const, limitation })),
  { code: "closing" },
  { code: "io", detail: "d" },
];

// covers: UXR-9.7
describe.each(catalogs)("every reason the core sends has a sentence in %s", (_name, text) => {
  const sentences: [string, string[]][] = [
    ["ineligibility", ineligibility.map((value) => ineligibilityText(value, text))],
    ["limitation", limitations.map((value) => limitationText(value, text))],
    ["unsuitability", unsuitabilities.map((value) => unsuitabilityText(value, text))],
    ["field", fields.map((value) => fieldProblemText(value, text))],
    ["list", lists.map((value) => listProblemText(value, text))],
    ["root", roots.map((value) => rootProblemText(value, text))],
    ["note", notes.map((value) => reviewNoteText(value, text))],
    ["stop", stops.map((value) => stopFailureText(value, text))],
    ["action", actions.map((value) => actionErrorText(value, text))],
    ["command", commands.map((value) => commandErrorText(value, text))],
    ["diagnostic", diagnostics.map((value) => diagnosticText(value, text))],
    ["file", [fileProblemText(file, text)]],
    ["problem", problemsOfTheCore.map((value) => problemText(value, text))],
    ["docker", dockers.map((value) => dockerProblemText(value, text))],
    ["settings", settings.map((value) => settingsMessage(value, text))],
    ["uses", uses.map((value) => usesMessage(value, text))],
    ["adoption", adoptions.map((value) => adoptionMessage(value, text))],
  ];

  it.each(sentences)("%s", (_family, written) => {
    for (const sentence of written) {
      expect(sentence.trim()).not.toBe("");
      expect(sentence).not.toContain("undefined");
    }
  });
});

// covers: UXR-9.7
describe("the sentences carry what the core said", () => {
  it("puts the parameters in the language in force", () => {
    expect(limitationText({ code: "form", key: "roots", problem: "mixedIndent" }, es)).toBe(
      "«roots» mezcla sangrías; edítalo a mano.",
    );
    expect(limitationText({ code: "form", key: "roots", problem: "mixedIndent" }, en)).toBe(
      "“roots” mixes indentations; edit it by hand.",
    );
    expect(listProblemText({ code: "tooMany", max: 32 }, en)).toBe("At most 32 folders.");
    expect(stopFailureText({ code: "protected", pid: 7 }, en)).toBe(
      "Process 7 is protected (DevPorts or its environment).",
    );
    expect(actionErrorText({ code: "notRunning", reference: "ab12" }, es)).toBe(
      "La instancia «ab12» no está en marcha.",
    );
  });

  it("keeps the technical detail as the system said it", () => {
    expect(settingsMessage({ code: "io", detail: "EACCES: permission denied" }, en)).toBe(
      "Could not read or write: EACCES: permission denied",
    );
    expect(
      ineligibilityText(
        {
          code: "configInvalid",
          problem: { code: "unreadable", path: "/c", detail: "EACCES" },
        },
        en,
      ),
    ).toContain("EACCES");
    expect(
      fileProblemText(
        {
          code: "invalid",
          path: "/p/.devports.yml",
          diagnostics: [
            { severity: "error", line: 4, kind: { code: "noCwdNorDocker", service: "api" } },
            { severity: "warning", line: 9, kind: { code: "unknownKey", key: "x" } },
          ],
        },
        en,
      ),
    ).toBe("/p/.devports.yml: line 4: “api” needs “cwd” (host service) or “docker” (container).");
  });
});

// covers: UXR-9.7
describe("a rejected call as text", () => {
  it("writes a typed command error in the language in force", () => {
    expect(failureText({ code: "onlyLocalUrls" }, en)).toBe("Only local addresses are opened.");
    expect(failureText({ code: "action", error: { code: "closing" } }, es)).toBe(
      "DevPorts se está cerrando.",
    );
    expect(failureText({ code: "tooManyTargets", max: 64 }, en)).toContain("at most 64");
  });

  it("keeps the message of anything else", () => {
    expect(failureText(new Error("response lost"), en)).toBe("response lost");
    expect(failureText("plain", en)).toBe("plain");
    expect(failureText({ code: "somethingNew" }, en)).toBe("[object Object]");
  });
});

// covers: UXR-9.7
describe("the text of a failed operation target", () => {
  const target = (given: Partial<OperationTargetResult>): OperationTargetResult => ({
    reference: "r",
    adoptTarget: null,
    status: "failed",
    error: "una frase del shell",
    result: null,
    attempts: 1,
    ...given,
  });

  it("prefers the typed failure, whatever family it is", () => {
    expect(
      operationFailureText(
        target({ actionError: { code: "stopFailed", failure: { code: "stillAlive" } } }),
        en,
      ),
    ).toBe("The instance is still running after trying to stop it.");
    expect(operationFailureText(target({ usesError: { code: "blocked" } }), en)).toContain(
      "Nothing was written",
    );
    expect(
      operationFailureText(
        target({ settingsError: { code: "invalidList", reason: { code: "empty" } } }),
        en,
      ),
    ).toContain("Leave at least one folder");
    expect(
      operationFailureText(target({ adoptionError: { code: "previewExpired" } }), es),
    ).toContain("caducó");
  });

  it("falls back to the shell's sentence and then to a generic word", () => {
    expect(operationFailureText(target({}), en)).toBe("una frase del shell");
    expect(operationFailureText(target({ error: null }), en)).toBe("Failed");
  });
});
