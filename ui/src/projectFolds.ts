// Which project cards are open. A card follows its activity (open while something runs)
// until the person opens or folds it by hand; that choice then wins and is remembered by
// the shell, so it survives the panel being closed.

import type { ProjectNode } from "./model";

export interface ProjectFolds {
  open: readonly string[];
  closed: readonly string[];
}

export const NO_FOLDS: ProjectFolds = { open: [], closed: [] };

const active = (node: ProjectNode): boolean => node.runningCount > 0 || node.unknownCount > 0;

export function isOpen(folds: ProjectFolds, node: ProjectNode): boolean {
  const name = node.summary.name;
  if (folds.open.includes(name)) return true;
  if (folds.closed.includes(name)) return false;
  return active(node);
}

const without = (names: readonly string[], name: string): string[] =>
  names.filter((item) => item !== name);

/** Opens or folds one card by hand. */
export function toggled(folds: ProjectFolds, node: ProjectNode): ProjectFolds {
  const name = node.summary.name;
  return isOpen(folds, node)
    ? { open: without(folds.open, name), closed: [...without(folds.closed, name), name] }
    : { open: [...without(folds.open, name), name], closed: without(folds.closed, name) };
}

/** Opens a card without touching the others, e.g. to show a service from Atención. */
export function revealed(folds: ProjectFolds, name: string): ProjectFolds {
  return { open: [...without(folds.open, name), name], closed: without(folds.closed, name) };
}

/** Opens or folds every card by hand. */
export function setAll(nodes: readonly ProjectNode[], open: boolean): ProjectFolds {
  const names = nodes.map((node) => node.summary.name);
  return open ? { open: names, closed: [] } : { open: [], closed: names };
}

/** Forgets projects that no longer exist. */
export function pruned(folds: ProjectFolds, names: readonly string[]): ProjectFolds {
  const known = new Set(names);
  return {
    open: folds.open.filter((name) => known.has(name)),
    closed: folds.closed.filter((name) => known.has(name)),
  };
}
