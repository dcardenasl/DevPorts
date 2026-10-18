// Browser-only fixture for the scan folders. Real validation and discovery stay the Rust
// core's job; this keeps a coherent in-memory list so the panel works without Tauri.
import { settingsMessage } from "./settings";
import type {
  OperationRequest,
  OperationTargetResult,
  ScanFolders,
  ScanPreview,
  ScanRoot,
  SettingsError,
} from "./types";

type Settled = Pick<
  OperationTargetResult,
  "status" | "error" | "settingsError" | "scanFolderCount"
>;

/** Projects each folder would contain, so a change has a visible effect. */
const CONTENTS: Record<string, string[]> = {
  "/Users/demo/developer": ["tienda-demo", "ci4-kickstart"],
  "/Users/demo/extra": ["extra-app"],
  "/Users/demo/work": ["work-api", "work-web"],
};
const MISSING = ["/Users/demo/missing"];

export function mockSettings() {
  let roots = ["~/developer"];
  let version = 1;
  const resolve = (configured: string): string =>
    configured.startsWith("~/") ? `/Users/demo/${configured.slice(2)}` : configured;
  const view = (configured: string): ScanRoot => {
    const path = resolve(configured);
    const exists = !MISSING.includes(path);
    return { configured, path, exists, projects: exists ? (CONTENTS[path]?.length ?? 0) : 0 };
  };
  const names = (list: string[]): string[] =>
    [...new Set(list.flatMap((root) => CONTENTS[resolve(root)] ?? []))].sort();
  const validate = (list: string[]): SettingsError | undefined => {
    if (list.length === 0) return { code: "invalidList", message: "deja al menos una carpeta" };
    const problems = list.flatMap((entry, index) => {
      const path = resolve(entry.trim());
      const seen = list.slice(0, index).some((other) => resolve(other.trim()) === path);
      const message =
        entry.trim() === ""
          ? "escribe una carpeta"
          : !path.startsWith("/")
            ? "usa una ruta absoluta o que empiece por ~/"
            : seen
              ? "esa carpeta ya está en la lista"
              : !roots.some((known) => resolve(known) === path) && MISSING.includes(path)
                ? "la carpeta no existe"
                : undefined;
      return message === undefined ? [] : [{ index, message }];
    });
    return problems.length === 0 ? undefined : { code: "invalidRoots", problems };
  };
  const folders = (): ScanFolders => ({
    configPath: "/Users/demo/.config/devports/config.yml",
    roots: roots.map(view),
    fromDefaults: version === 1,
    maxDepth: 3,
    editable: true,
    reason: null,
    base: `v${String(version)}`,
  });
  return {
    scanFolders: async (): Promise<ScanFolders> => {
      await Promise.resolve();
      return folders();
    },
    previewScanFolders: async (list: string[]): Promise<ScanPreview> => {
      await Promise.resolve();
      const error = validate(list);
      if (error) throw Object.assign(new Error(settingsMessage(error)), error);
      const before = names(roots);
      const after = names(list);
      return {
        roots: list.map((entry) => view(entry.trim())),
        added: after.filter((name) => !before.includes(name)),
        removed: before.filter((name) => !after.includes(name)),
        unchanged:
          JSON.stringify(list.map((root) => resolve(root.trim()))) ===
          JSON.stringify(roots.map(resolve)),
      };
    },
    /** Runs a whole `setScanFolders` request the way the shell does. */
    settle: (request: OperationRequest): Settled[] =>
      request.targets.map((target): Settled => {
        const submitted = target.scanFolders;
        const count = submitted?.roots.length ?? null;
        const error: SettingsError | undefined = submitted
          ? (validate(submitted.roots) ??
            (submitted.base === `v${String(version)}` ||
            JSON.stringify(submitted.roots.map((r) => resolve(r.trim()))) ===
              JSON.stringify(roots.map(resolve))
              ? undefined
              : {
                  code: "documentChanged" as const,
                  path: "/Users/demo/.config/devports/config.yml",
                }))
          : { code: "io", message: "falta indicar las carpetas" };
        if (error === undefined && submitted) {
          roots = submitted.roots.map((root) => root.trim());
          version += 1;
        }
        return error === undefined
          ? { status: "succeeded", error: null, settingsError: null, scanFolderCount: count }
          : {
              status: "failed",
              error: settingsMessage(error),
              settingsError: error,
              scanFolderCount: count,
            };
      }),
  };
}
