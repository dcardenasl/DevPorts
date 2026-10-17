// The only file that knows how the panel reaches the core. In the application it
// calls Tauri commands; in a browser during development it runs on sample data.

import type { AdoptTarget, LogEntry, Snapshot } from "./types";

export interface Backend {
  snapshot(): Promise<Snapshot>;
  /** Calls `listener` with every new snapshot; resolves to the unsubscribe function. */
  subscribe(listener: (snapshot: Snapshot) => void): Promise<() => void>;
  stop(ref: string): Promise<void>;
  extend(ref: string): Promise<void>;
  keep(ref: string): Promise<void>;
  adopt(ref: string, target: AdoptTarget): Promise<{ project: string; service: string }>;
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

function tauriBackend(tauri: TauriGlobal): Backend {
  const invoke = tauri.core.invoke;
  return {
    snapshot: () => invoke<Snapshot>("get_snapshot"),
    subscribe: (listener) =>
      tauri.event.listen<Snapshot>(SNAPSHOT_EVENT, (event) => {
        listener(event.payload);
      }),
    stop: (ref) => invoke("stop_instance", { reference: ref }),
    extend: (ref) => invoke("extend_instance", { reference: ref }),
    keep: (ref) => invoke("keep_instance", { reference: ref }),
    adopt: (ref, target) => invoke("adopt_instance", { reference: ref, target }),
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
    if (tauri !== undefined) return tauriBackend(tauri);
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
