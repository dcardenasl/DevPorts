// Every string the person reads, in English. It has the very same shape as the Spanish
// source (`Catalog`), so a missing key or a changed signature does not compile.
import type { DocumentKind, FormProblem } from "../types";
import { type Catalog, type EventContext } from "./es";
import { pluralizer } from "./format";

const { pick, counted } = pluralizer("en");

export const en: Catalog = {
  adoption: {
    review: "Review adoption",
    body: "Registers the server that is already running. It does not restart it.",
    loading: "Looking up destinations…",
    preparing: "Preparing the review…",
    saving: "Saving the declaration…",
    checking: "Checking the result of the save…",
    retryQuery: "Check again",
    retry: "Retry",
    newReview: "Prepare a new review",
    editAgain: "Edit again",
    save: "Save",
    continue: "Review",
    name: "Service name",
    start: "Start command",
    startHint: "Saved for future launches. You can leave it empty if there is no stable command.",
    root: "Project root",
    manifest: "Target file",
    port: "Observed port",
    unknownPort: "Not available",
    creates: "This file will be created when you save.",
    chooses: "Choose a destination to review the proposal before saving.",
    close: "Close",
    done: "Done",
    openProject: "View project",
    openShared: "View shared",
    pendingCard: "The catalog does not show the destination yet. Wait for it to refresh.",
    unsent:
      "The operation that was sent could not be found. It has not been sent again: edit again and confirm Save once more.",
    optionsFailed: "The destinations could not be looked up.",
    targetMissing: "The instance is no longer available to prepare another review.",
  },
  tabs: {
    attention: "Attention",
    projects: "Projects",
    shared: "Shared",
    orphans: "No project",
    activity: "Activity",
  },
  attention: {
    title: "Attention",
    allClear: "All clear",
    allClearBody: "Nothing is waiting for your decision.",
    waiting: (count: number) =>
      pick(
        count,
        "1 thing is waiting for your decision",
        `${count} things are waiting for your decision`,
      ),
    official: "OFFICIAL",
    expiring: "TEMPORARY, ABOUT TO EXPIRE",
    hung: "HUNG · NOT RESPONDING",
    duplicates: (count: number, service: string) =>
      `${count} UNREGISTERED · DUPLICATING ${service}`,
    stopAll: (count: number) => `Stop all ${count}`,
    stop: "Stop",
    keep: "Keep",
    legitimate: "Mark as legitimate",
    extend: (minutes: number) => `Extend ${minutes} min`,
    terminate: "End",
    seeInProjects: "See in Projects",
    dismiss: "Ignore",
    askedBy: (who: string) => `requested by ${who}`,
    since: (time: string) => `since ${time}`,
  },
  instance: {
    states: {
      healthy: "Healthy",
      unhealthy: "Hung · not responding",
      down: "Stopped",
      starting: "Starting",
      stopping: "Stopping",
      forgotten: "Forgotten · still running",
      unknown: "Unknown · not being observed right now",
    },
    roles: { principal: "Primary", temporal: "Temporary", unregistered: "Unregistered" },
    details: (name: string) => `Details of ${name}`,
    detailsIdentity: (name: string, reference: string) => `Details of ${name} · ref ${reference}`,
    state: "State",
    role: "Role",
    origin: "Declared origin",
    reference: "Reference",
    pid: "PID",
    identity: "Target identity",
    started: "Started",
    ports: "Observed ports",
    noObservedPorts: "No ports observed in the current snapshot",
    referencePort: "Reference port",
    folder: "Folder",
    worktree: "Worktree",
    branch: "Branch",
    container: "Container",
    compose: "Compose project",
    image: "Image",
    registration: "Registration",
    registered: "Registered with devports run",
    observed: "Not registered with devports run",
    noProject: "No known project",
    unavailable: "Not available",
    unknownProgram: "Process",
    launcher: "Launch chain (informational)",
    unknownLauncher: "Launcher not available; its origin is not guessed",
    command: "Command (values hidden)",
    hidden: "[hidden]",
    commandPrivacy:
      "The executable and the long options are shown. Values and positional arguments are hidden to protect credentials and scripts.",
    stale: "Possibly older code",
    staleBody: "This is a hint based on Git dates, not proof of a fault nor permission to restart.",
  },
  projects: {
    title: "Projects",
    summary: (projects: number, repos: number, services: number) =>
      `${counted(projects, "project", "projects")} · ${counted(repos, "repo", "repos")} · ${counted(services, "service", "services")}`,
    none: "No projects yet",
    noneBody:
      "A project is a folder with a .devports.yml file. Adopt a process from “No project” to create the first one.",
    noActive: "no active services",
    foldAll: "Fold all",
    openAll: "Open all",
    unconfirmed: "state not confirmed",
    uses: "Uses",
    temporal: "Temporary",
    open: "Open in the browser",
    copy: "Copy start command",
    copied: "Command copied",
    stop: "Stop",
    stopProject: "Stop the project's services",
    preparingStop: "Preparing the list…",
    nothingToStop: "There are no active applications in this project that can be stopped.",
    unregistered: "unregistered",
    duplicatesOf: (count: number) => `${count} unregistered`,
  },
  shared: {
    title: "Shared",
    summary: (count: number) => counted(count, "service", "services"),
    none: "There are no shared services",
    onlyUndeclared: "None declared yet",
    noneBody:
      "They are declared in ~/.config/devports/shared.yml and projects cite them in “uses”.",
    undeclared: "UNDECLARED",
    undeclaredBody:
      "They are running and nobody declared them. Declare them so projects can cite them.",
    declare: "Declare",
    declared: "Declared in shared.yml",
    usedBy: "Used by",
    nobody: "No project uses it",
    data: "data",
    docker: "docker",
  },
  preferences: {
    appearance: "Appearance",
    theme: "Theme",
    themes: { system: "System", light: "Light", dark: "Dark" },
    language: "Language",
    languages: { system: "System", es: "Español", en: "English" },
    saveFailed: "It could not be saved. What was there is kept.",
  },
  settings: {
    open: "Settings",
    sheetTitle: "Settings",
    title: "Scan folders",
    description:
      "DevPorts looks for projects (.devports.yml) in these folders. Removing one only stops looking in it: no files are deleted and no processes are stopped.",
    loading: "Reading the configuration…",
    loadFailed: "The configuration could not be read.",
    retryLoad: "Read again",
    notEditable: "It cannot be edited from here",
    fromDefaults: "You are using the default folder; saving will write it to config.yml.",
    exists: "exists",
    missing: "does not exist right now",
    projects: (count: number) => counted(count, "project", "projects"),
    folder: (index: number) => `Folder ${index}`,
    placeholder: "~/developer",
    add: "Add folder",
    remove: (folder: string) => `Remove ${folder}`,
    review: "Review the effect",
    noChanges: "No changes to save",
    reviewTitle: "Review the effect",
    reviewBody:
      "This will change which projects appear. No file is deleted and no process is stopped.",
    added: "Will start to be listed",
    removed: "Will stop being listed",
    nothingChanges: "The list of projects does not change.",
    save: "Save",
    saving: "Saving the configuration…",
    checking: "Checking the result of the save…",
    retryQuery: "Check again",
    retry: "Retry",
    reread: "Read the configuration again",
    unsent:
      "The operation that was sent could not be found. It has not been sent again: review again and confirm Save.",
    saved: "Configuration saved. The changes are already in use.",
    advanced: "Expiry times and advanced limits are edited in the file:",
    copyPath: "Copy path",
    done: "Done",
    close: "Close",
  },
  classify: {
    groupProject: "Development servers",
    groupUnclassified: "Unclassified",
    groupAuxiliary: "Possible helper tools",
    auxiliaryBody:
      "They are usually daemons or programs another tool launches. It is a hint, not a fact: they stay here with all their actions and you can correct the classification.",
    unclassifiedBody: "There are not enough hints to classify them.",
    auxiliaries: (count: number) => counted(count, "possible helper", "possible helpers"),
    action: "Reclassify",
    title: "Correct the classification",
    body: "It only changes which group it is listed in. It does not ignore, adopt, stop or restart it.",
    current: "Now",
    evidence: "Hints",
    noEvidence: "No hints.",
    scope:
      "The correction applies only to this exact folder and command, like an ignore rule, and is remembered across restarts. If they change, the automatic classification returns.",
    choose: "List as",
    auto: "Automatic",
    autoNow: (label: string) => `Automatic (now: ${label})`,
    project: "Development server",
    auxiliary: "Helper tool",
    unclassified: "Unclassified",
    save: "Save",
    corrected: "corrected",
    origin: (origin: string) => `from ${origin}`,
    signals: {
      homebrewDaemon: "Daemon installed with Homebrew",
      editorExtension: "Program of an editor extension",
      knownProject: "Runs inside a known project",
      gitWorktree: "Runs inside a Git repository",
    },
  },
  ignore: {
    action: "Ignore",
    title: "Ignore this service",
    body: "It stops being shown and asking for attention. It is not stopped or adopted, and it keeps running.",
    criterion: "Exactly this will be ignored",
    folder: "Folder",
    command: "Command (values hidden)",
    container: "Container",
    context: "Docker context",
    scope:
      "Only the same folder and the same command with the same arguments match, also after a restart or a pid change. If the folder, the command or an argument (for example the port) changes, it will show up again.",
    containerScope:
      "Only the container with that name in that Docker context matches, also after a restart.",
    restore: "You can undo it from “View ignored”.",
    confirm: "Ignore this service",
    loading: "Preparing the rule…",
    failed: "The rule could not be prepared.",
    view: "View ignored",
    listTitle: "Ignored services",
    listBody:
      "Each rule hides only what it describes. Restoring removes it and the service is observed again; it starts and stops nothing.",
    none: "There are no ignored services.",
    legacy: "earlier rule",
    legacyBody:
      "Saved by an earlier version: it matches as it always did, without widening. The command is shown approximately.",
    since: "Since",
    restoreAction: "Restore",
    restoring: "Restoring…",
    unknown: "A rule this version cannot read. It can still be restored.",
    close: "Close",
  },
  consumers: {
    manage: "Manage projects",
    title: (shared: string) => `Projects of ${shared}`,
    description:
      "Tick which projects declare this resource in “uses”. It is a declaration in their manifests: it does not detect real connections nor stop anything.",
    declaredOnly: "Real use may not be declared.",
    associated: "Associated projects",
    nobody: "No associated projects",
    count: (count: number) => counted(count, "associated project", "associated projects"),
    search: "Search for a project",
    noMatch: "No project matches the search.",
    loading: "Reading the manifests…",
    loadFailed: "The manifests could not be read.",
    retryLoad: "Read again",
    undefinedShared:
      "This resource is not defined in shared.yml. Declare the resource first and then associate projects.",
    review: "Review changes",
    noChanges: "No changes to save",
    reviewTitle: "Review changes",
    reviewBody:
      "Only the “uses” line of these manifests will be edited. Nothing is stopped, restarted or deleted.",
    add: "Will be added to",
    remove: "Will be removed from",
    save: "Save",
    saving: "Saving the manifests…",
    checking: "Checking the result of the save…",
    retryQuery: "Check again",
    retry: "Retry pending ones",
    reread: "Read the manifests again",
    unsent:
      "The operation that was sent could not be found. It has not been sent again: review again and confirm Save.",
    saved: "Saved",
    pending: "Pending",
    partial: "Some projects were saved; the rest are still pending.",
    done: "Done",
    close: "Close",
    unchanged: "was already like this",
    manageFor: (shared: string) => `Manage projects of ${shared}`,
    associate: (shared: string) => `Associate ${shared}`,
    stopped: "Stopped",
    copyStart: "Copy start command",
    noStart: "No start command declared in shared.yml.",
    open: "Open",
  },
  orphans: {
    title: "No project",
    group: (count: number) => counted(count, "process", "processes"),
    otherGroup: "Others",
    summary: (count: number) =>
      count === 0 ? "Nothing loose" : `${counted(count, "process", "processes")} without an owner`,
    none: "Nothing loose",
    noneBody: "Everything that listens on a port belongs to a project.",
    adopt: "Adopt",
    seems: (project: string) => `Looks like ${project}'s`,
    sheetTitle: "Where does it go?",
    back: "Back",
    asService: (project: string) => `As a service of ${project}`,
    suggested: "SUGGESTED",
    another: "Another project",
    anotherBody: "Choose among the known projects",
    newProject: "Create a new project",
    newProjectBody: (folder: string) => `With ${folder} as its root`,
    shared: "As a shared service",
    sharedBody: "Several projects will use it (shared.yml)",
    projectName: "Project name",
    create: "Create",
    cancel: "Cancel",
    added: (project: string) => `Added to ${project}`,
  },
  activity: {
    title: "Activity",
    summary: (count: number) =>
      count === 0 ? "No activity in 24 h" : counted(count, "event", "events"),
    of: (shown: number, total: number) => `${shown} of ${counted(total, "event", "events")}`,
    window: "Last 24 h",
    loading: "Loading activity…",
    unreadable: "Could not be read",
    closing: "DevPorts is closing.",
    loadFailed: "The activity could not be read. This does not mean there was none.",
    staleFailed: "The activity could not be refreshed; the last reading is shown.",
    retry: "Retry",
    noneWindow: "No activity in the last 24 h",
    noneWindowBody: "Starts, rejections, expiries and warnings about your servers show up here.",
    noMatch: (total: number) =>
      `No event matches this filter (there are ${counted(total, "event", "events")} in total).`,
    showAll: "Show all",
    all: "All",
    rejections: "Rejections",
    temporals: "Temporary",
    alerts: "Alerts",
    none: "No activity for this filter",
    unnamedService: "a service",
    events: {
      core_started: () => "DevPorts started",
      run_accepted: (e: EventContext) =>
        e.temporal
          ? `Test of ${e.service} ${e.port} for “${e.reason ?? "no reason"}”`
          : `${e.service} registered as Primary ${e.port}`,
      run_rejected: (e: EventContext) =>
        `${e.launcher === null ? "A process" : `An agent (${e.launcher})`} tried to start another instance of ${e.service}; the Primary is still ${e.url === null ? "" : `at ${e.url}`}`,
      reuse: (e: EventContext) =>
        `${e.launcher === null ? "A process" : `An agent (${e.launcher})`} reused the Primary of ${e.service}`,
      reservation_released: (e: EventContext) =>
        `The port reserved for ${e.service} was released: nobody opened it in time`,
      bound: (e: EventContext) => `${e.service} ${e.port} was bound to its process`,
      port_stolen: (e: EventContext) =>
        `Another process took port ${e.portNumber ?? "?"}, reserved for ${e.service}`,
      lease_warned: (e: EventContext) => `The test of ${e.service} ${e.port} expires in 1 minute`,
      lease_extended: (e: EventContext) =>
        `The test of ${e.service} ${e.port} was extended; it expires ${e.expires ?? "later"}`,
      lease_expired: (e: EventContext) =>
        `The test of ${e.service} ${e.port} expired and was stopped`,
      stopped: (e: EventContext) => `${e.service} ${e.port} was stopped`,
      stop_failed: (e: EventContext) =>
        `${e.service} ${e.port} could not be stopped: it is left as forgotten`,
      duplicate_detected: (e: EventContext) =>
        `An unregistered duplicate of ${e.service} ${e.port} appeared`,
      kept: (e: EventContext) =>
        `You kept ${e.service} ${e.port} although it duplicates the Primary`,
      restored: () => "You restored an ignored service",
      reclassified: (e: EventContext) => `You changed the classification of ${e.service} ${e.port}`,
      adopted: (e: EventContext) => `You adopted ${e.service} into a manifest`,
      unregistered_seen: (e: EventContext) => `An unregistered process appeared ${e.port}`,
      process_gone: (e: EventContext) => `${e.service} ${e.port} stopped running`,
    },
    labels: {
      rejection: "REJECTED",
      temporal: "TEMPORARY",
      primary: "PRIMARY",
      warned: "WARNING",
      extended: "EXTENDED",
      expired: "EXPIRED",
      unregistered: "UNREGISTERED",
      adopted: "ADOPTED",
      kept: "KEPT",
      restored: "RESTORED",
      reclassified: "RECLASSIFIED",
      stopped: "STOPPED",
      alert: "ALERT",
    },
  },
  operations: {
    title: "Recent operations",
    kind: {
      stop: "Stop",
      stopProject: "Stop project",
      extend: "Extend",
      keep: "Ignore",
      restore: "Restore",
      classify: "Classification",
      setScanFolders: "Scan folders",
      adopt: "Adopt",
      setConsumers: "Associated projects",
    },
    queued: "Queued",
    running: "Running…",
    succeeded: "Completed",
    retry: "Retry failed ones",
    failed: "Failed",
    adopted: (project: string, service: string) => `Added to ${project} · ${service}`,
  },
  confirm: {
    stopProjectTitle: (project: string) => `Stop the applications of ${project}?`,
    projectStopBody:
      "Only the instances listed will be stopped. Data and shared services stay active. New launches are not part of this action.",
    stopTitle: (name: string) => `Stop ${name}?`,
    stopBody: "It will be asked to finish and, if it does not, it will be forced.",
    stopGroupTitle: (count: number) => `Stop these ${count} instances?`,
    dataStopBody:
      "The data container will be stopped. DevPorts will not delete the container nor its volumes.",
    affects: "These will lose this service:",
    identity: (view: {
      pid: number | null;
      container: string | null;
      ref: string;
      started_at: string | null;
    }) =>
      view.pid !== null
        ? `Process pid ${view.pid}${view.started_at === null ? "" : ` · started ${view.started_at}`}`
        : `Container ${view.container ?? "—"} · ref ${view.ref}`,
    port: (port: number | null) => (port === null ? "No port" : `Port ${port}`),
    origin: (view: { shared: boolean; project: string | null; service: string | null }) =>
      view.shared
        ? "Shared resource"
        : view.project === null
          ? "No project"
          : `${view.project}${view.service === null ? "" : ` · ${view.service}`}`,
    targetChanged: "The instance changed since the confirmation. Review it and confirm again.",
    cancel: "Cancel",
    stop: (name: string) => `Stop ${name}`,
    stopGroup: "Stop selected",
  },
  status: {
    dockerDown: "Docker is not responding",
    dockerDownBody:
      "Containers show as “not confirmed”, not as stopped. Open Docker Desktop: DevPorts checks again on its own.",
    draftGone:
      "The process of this draft is no longer available. The form was discarded; no action was taken.",
    problems: "Core warnings",
    connecting: "Connecting to the core…",
    loading: "Loading the information about projects and ports…",
    failed: "The action could not be completed",
  },
  link: {
    connectingAttempt: (attempt: number, total: number) =>
      `Connecting to DevPorts… attempt ${attempt} of ${total}`,
    cannotConnect: "Could not connect",
    cannotConnectBody:
      "The panel cannot talk to DevPorts. Check that the app is still open (icon in the menu bar) and retry. Operations you already confirmed are not sent again.",
    reconnecting: (attempt: number, total: number, since: string) =>
      `No connection to DevPorts · retrying (${attempt}/${total}). This data is from ${since}.`,
    lost: (since: string) =>
      `It could not reconnect. This data is from ${since} and actions may fail.`,
    retry: "Retry",
    retryNow: "Retry now",
    problems: (count: number) =>
      pick(
        count,
        "1 configuration or manifest warning",
        `${count} configuration or manifest warnings`,
      ),
    details: "See details",
    state: "State",
    core: "Core",
    docker: "Docker",
    coreOk: "Connected",
    coreRetrying: "No connection; retrying",
    coreLost: "No connection",
    dockerOk: "Responding",
    dockerDown: (reason: string) => `Not responding: ${reason}`,
    noProblems: "No configuration or manifest warnings.",
    problemsTitle: "Warnings",
  },
  errors: {
    settings: {
      invalidRoots: "Some folders cannot be used. Fix them and review again.",
      documentChanged: (path: string) =>
        `${path} changed since you reviewed it. Read the configuration again before saving.`,
    },
    adoption: {
      documentChanged: (path: string) => `${path} changed. Prepare a new review before saving.`,
      targetChanged: "The instance or its destination changed. Review the proposal again.",
      previewExpired: "The review expired. Your fields are kept; prepare another review.",
      requestChanged:
        "This review already belongs to another save. Check its result or prepare another review.",
      atCapacity: "There are too many operations running. Wait and try again.",
    },
    uses: {
      unknownShared: (name: string) =>
        `“${name}” is no longer defined in shared.yml. Declare it before associating it.`,
      unknownProject: (project: string) => `The project “${project}” no longer exists.`,
      documentChanged: (path: string) =>
        `${path} changed since you reviewed it. Check again before saving.`,
      blocked: "Nothing was written: another project in this selection did not pass validation.",
      invalidRequest: "The selection repeats a project or exceeds the maximum.",
    },
  },
  reasons: {
    io: (detail: string) => `Could not read or write: ${detail}`,
    closing: "DevPorts is closing.",
    action: {
      unknownInstance: (reference: string) => `There is no instance “${reference}”.`,
      notRunning: (reference: string) => `The instance “${reference}” is not running.`,
      targetChanged: "The instance changed since the confirmation; review it again.",
      noKnownFolder: "A process without a known folder cannot be remembered.",
      tooManyCorrections: "There are too many classification corrections; reset some.",
      storage: (detail: string) => `Could not read or save: ${detail}`,
      internal: (detail: string) => `Internal error: ${detail}`,
    },
    stop: {
      protected: (pid: number) => `Process ${pid} is protected (DevPorts or its environment).`,
      otherUser: (pid: number) => `Process ${pid} belongs to another user.`,
      stillAlive: "The instance is still running after trying to stop it.",
      processTableUnreadable: "The process table could not be read.",
      dockerUnavailable: "Docker is not available.",
      failed: (detail: string) => detail,
    },
    command: {
      tooManyTargets: (max: number) =>
        `There are too many instances for a single operation (at most ${max}); stop services one by one.`,
      onlyLocalUrls: "Only local addresses are opened.",
      invalidPanelState: (detail: string) => `The panel context is not valid: ${detail}`,
      operationInvalid: "The operation request is not valid.",
      operationIdConflict: "That identifier already belongs to another operation.",
      operationAtCapacity: "There are too many operations running.",
      operationNotFound: "The operation does not exist.",
      nothingToRetry: "The operation has no failed targets to retry.",
    },
    ineligibility: {
      configInvalid: (detail: string) =>
        `config.yml has errors; fix them by hand before editing: ${detail}`,
      noManifest: "The project has no .devports.yml yet; adopt one of its services first.",
      manifestInvalid: (detail: string) => `The manifest is not valid: ${detail}`,
      readOnly: (path: string) => `${path} is read-only.`,
      rootsNotEditable:
        "config.yml writes “roots” in a form DevPorts does not edit; edit it by hand.",
    },
    limitation: {
      notRegularFile: "Only regular files are edited, not symbolic links.",
      hardLinked: "Manifests with hard links are not replaced.",
      notUtf8: "The file is not UTF-8 text.",
      tooLarge: (max: number) => `Automatic editing accepts at most ${max} bytes.`,
      ownerNotKept: "The owner or group of the file cannot be kept.",
      targetNotAbsolute: "The destination must be absolute and must not contain “..”.",
      noTargetFile: "The destination file is missing.",
      noTargetFolder: "The destination folder is missing.",
      targetFolderNotDirectory: "The destination folder is not a regular directory.",
      targetFolderNotFound: "The destination folder was not found.",
      targetFolderUnresolvable: "The destination folder cannot be resolved.",
      form: (key: string, problem: FormProblem) => {
        const name = `“${key}”`;
        switch (problem) {
          case "notFound":
            return `${name} was not found.`;
          case "notAList":
            return `${name} is not a list; edit it by hand.`;
          case "notSimpleList":
            return `${name} is not a list of simple values; edit it by hand.`;
          case "multiLine":
            return `${name} spans several lines; edit it by hand.`;
          case "mixedIndent":
            return `${name} mixes indentations; edit it by hand.`;
          case "emptyItem":
            return `${name} has an empty item.`;
          case "invalidValue":
            return `${name} contains values that are not simple; edit it by hand.`;
          case "notEditable":
            return `${name} is written in a form DevPorts does not edit; edit it by hand.`;
        }
      },
      editInvalid: (document: DocumentKind, detail: string) =>
        `The edit did not produce ${document === "config" ? "a valid configuration" : "a valid manifest"}: ${detail}`,
      editTouchedMore: (key: string) =>
        `The edit would have changed more than “${key}”; edit it by hand.`,
      other: (detail: string) => detail,
    },
    unsuitability: {
      instanceUnavailable: "That instance is not available for adoption.",
      sharedNeedsContainer: "Only containers can be declared as shared.",
      containerNeedsProject: "For that container choose an existing project or Shared.",
      folderUnresolvable: (detail: string) => `The folder cannot be resolved: ${detail}`,
      noRoot: "The destination has no root.",
      noKnownFolder: "The process has no known folder.",
      folderTooBroad:
        "That folder is too broad to be a project; adopt the process from the folder of its repository.",
      manifestExists:
        "There is already a manifest at the root; choose that project and review its content.",
      outsideProjectRoot: "The process folder is outside the project root.",
      outOfScanScope:
        "This root is not within the scan folders; add its folder in Settings before adopting.",
      projectNotInCatalog: "That project is no longer in the catalog.",
      resultTooLarge: "The result exceeds the automatic editing limit.",
      notDescribable: "That process or container could not be described from that folder.",
      fileInvalid: (detail: string) => detail,
      other: (detail: string) => detail,
    },
    field: {
      invalidName: "Use 1 to 63 lowercase letters, digits or hyphens.",
      invalidStart: "The command must be a single line, without NUL, between 1 and 16384 bytes.",
      nameTaken: "A service with that name already exists.",
      tooLong: "The value exceeds the allowed limit.",
    },
    listProblem: {
      empty: "Leave at least one folder: without any, no project would be discovered.",
      tooMany: (max: number) => `At most ${max} folders.`,
    },
    rootProblem: {
      empty: "type a folder",
      unsafeText:
        "the path contains characters that are not accepted (quotes, backslashes or line breaks)",
      notAbsolute: "use an absolute path or one that starts with ~/",
      tooBroad: "that folder is too broad; choose the one that holds your projects",
      duplicate: "that folder is already in the list",
      missing: "the folder does not exist",
    },
    reviewNote: {
      launcherProgram: (program: string) =>
        `“${program}” usually launches the real server as a child process: review the command.`,
      notOneLine: "The observed command does not fit on one line; give a stable command or script.",
      noStartCommand:
        "A start command could not be proposed; you can declare it without automatic start.",
      manifestWarning: (detail: string) => detail,
    },
  },
  a11y: {
    operations: "Operations",
    sections: "Sections",
    filter: "Filter",
  },
};
