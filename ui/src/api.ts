// The only file that knows how the panel reaches the core. In the application it
// calls Tauri commands; in a browser during development it runs on sample data.

import type {
  InstanceView,
  LogEntry,
  OperationRequest,
  OperationView,
  PanelState,
  Snapshot,
} from "./types";

export interface Backend {
  snapshot(): Promise<Snapshot>;
  previewProjectStop(project: string): Promise<InstanceView[]>;
  panelState(): Promise<PanelState>;
  savePanelState(state: PanelState): Promise<void>;
  operations(): Promise<OperationView[]>;
  operation(id: string): Promise<OperationView | null>;
  subscribeOperations(listener: (operation: OperationView) => void): Promise<() => void>;
  startOperation(request: OperationRequest): Promise<OperationView>;
  retryOperation(id: string): Promise<OperationView>;
  /** Calls `listener` with every new snapshot; resolves to the unsubscribe function. */
  subscribe(listener: (snapshot: Snapshot) => void): Promise<() => void>;
  log(sinceSeconds: number): Promise<LogEntry[]>;
  openUrl(url: string): Promise<void>;
  /** Tells the shell the first frame is painted so it can show the window. */
  panelReady(): Promise<void>;
}

interface TauriGlobal {
  core: { invoke: <T>(command: string, args?: Record<string, unknown>) => Promise<T> };
  event: {
    // `T` is the caller's promise about the payload.
    // eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters
    listen: <T>(name: string, handler: (event: { payload: T }) => void) => Promise<() => void>;
  };
}

declare global {
  interface Window {
    __TAURI__?: TauriGlobal;
  }
}

export const SNAPSHOT_EVENT = "snapshot";
export const OPERATION_EVENT = "operation";

export function createTauriBackend(tauri: TauriGlobal): Backend {
  const invoke = tauri.core.invoke;
  let stateWrites = Promise.resolve();
  return {
    snapshot: () => invoke<Snapshot>("get_snapshot"),
    previewProjectStop: (project) => invoke<InstanceView[]>("preview_project_stop", { project }),
    panelState: () => invoke<PanelState>("get_panel_state"),
    savePanelState: (state) => {
      stateWrites = stateWrites.then(() => invoke("set_panel_state", { state }));
      return stateWrites;
    },
    operations: () => invoke<OperationView[]>("get_operations"),
    operation: (id) => invoke<OperationView | null>("get_operation", { id }),
    subscribeOperations: (listener) =>
      tauri.event.listen<OperationView>(OPERATION_EVENT, (event) => {
        listener(event.payload);
      }),
    startOperation: async (request) => {
      try {
        return await invoke<OperationView>("start_operation", { request });
      } catch (failure) {
        const existing = await invoke<OperationView | null>("get_operation", { id: request.id });
        if (existing !== null) return existing;
        return invoke<OperationView>("start_operation", { request }).catch(() => {
          throw failure;
        });
      }
    },
    retryOperation: async (id) => {
      try {
        return await invoke<OperationView>("retry_operation", { id });
      } catch (failure) {
        const existing = await invoke<OperationView | null>("get_operation", { id });
        if (existing !== null && existing.status !== "failed" && existing.status !== "partial") {
          return existing;
        }
        return invoke<OperationView>("retry_operation", { id }).catch(() => {
          throw failure;
        });
      }
    },
    subscribe: (listener) =>
      tauri.event.listen<Snapshot>(SNAPSHOT_EVENT, (event) => {
        listener(event.payload);
      }),
    log: (sinceSeconds) => invoke<LogEntry[]>("get_log", { sinceSeconds }),
    openUrl: (url) => invoke("open_url", { url }),
    panelReady: () => invoke("panel_ready"),
  };
}

let cached: Promise<Backend> | undefined;

/** The backend for this page: the real one inside the application. */
export function backend(): Promise<Backend> {
  cached ??= (async () => {
    const tauri = window.__TAURI__;
    if (tauri !== undefined) return createTauriBackend(tauri);
    // Development in a browser only: the sample data never reaches a release
    // build because Vite removes this branch where `import.meta.env.DEV` is false.
    if (import.meta.env.DEV) {
      const { mockBackend } = await import("./mock");
      return mockBackend();
    }
    throw new Error("DevPorts: the panel must run inside the application");
  })();
  return cached;
}
