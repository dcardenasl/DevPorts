// Browser-only fixture for ignore rules. The exact matching stays the Rust core's job;
// this keeps the snapshot and the rule list coherent so the panel can be developed without Tauri.
import type { IgnoreRule, InstanceView, Snapshot } from "./types";

export function mockIgnored(get: () => Snapshot, publish: (next: Snapshot) => void) {
  const rules: { rule: IgnoreRule; reference: string }[] = [];
  let counter = 0;
  const find = (reference: string): InstanceView => {
    const view = get().instances.find((candidate) => candidate.ref === reference);
    if (view === undefined) throw new Error(`no existe la instancia «${reference}»`);
    return view;
  };
  const describe = (view: InstanceView): IgnoreRule => {
    if (view.container === null && (view.cwd === null || view.argv.length === 0))
      throw new Error("no se puede recordar un proceso sin carpeta conocida");
    return {
      id: `rule-${String(counter + 1).padStart(4, "0")}`,
      kind: view.container === null ? "process" : "container",
      folder: view.container === null ? view.cwd : null,
      command: view.container === null ? view.argv : [],
      container: view.container,
      context: view.container === null ? null : "default",
      legacy: false,
      createdAt: null,
    };
  };
  const setIgnored = (reference: string, ignored: boolean): void => {
    const state = get();
    publish({
      ...state,
      instances: state.instances.map((view) =>
        view.ref === reference ? { ...view, ignored } : view,
      ),
      attention: ignored
        ? state.attention.filter((item) => item.instance_ref !== reference)
        : state.attention,
    });
  };
  return {
    ignorePreview: async (reference: string): Promise<IgnoreRule> => {
      await Promise.resolve();
      return describe(find(reference));
    },
    ignoredRules: async (): Promise<IgnoreRule[]> => {
      await Promise.resolve();
      return rules.map((entry) => entry.rule).reverse();
    },
    /** Saves the rule for `reference` and hides what it names. */
    keep: (reference: string): void => {
      const rule = describe(find(reference));
      counter += 1;
      rules.push({
        rule: { ...rule, createdAt: new Date(Date.UTC(2026, 9, 17, 12, counter)).toISOString() },
        reference,
      });
      setIgnored(reference, true);
    },
    /** Forgets a rule; restoring one that is already gone is not an error. */
    restore: (id: string): void => {
      const index = rules.findIndex((entry) => entry.rule.id === id);
      const [removed] = index < 0 ? [] : rules.splice(index, 1);
      if (removed) setIgnored(removed.reference, false);
    },
  };
}
