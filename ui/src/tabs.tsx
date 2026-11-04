import type { ReactElement } from "react";

import { useI18n } from "./i18n/provider";
import { BellIcon, DatabaseIcon, FolderIcon, ListIcon, OrphanIcon } from "./icons";

export type Tab = "attention" | "projects" | "shared" | "orphans" | "activity";

export const TABS: { id: Tab; icon: (props: { size?: number }) => ReactElement }[] = [
  { id: "attention", icon: BellIcon },
  { id: "projects", icon: FolderIcon },
  { id: "shared", icon: DatabaseIcon },
  { id: "orphans", icon: OrphanIcon },
  { id: "activity", icon: ListIcon },
];

export const tabId = (id: Tab): string => `tab-${id}`;
export const panelId = (id: Tab): string => `panel-${id}`;

/** The tab an arrow, Home or End key moves to; roving focus wraps at the ends. */
function nextTab(current: Tab, key: string): Tab | undefined {
  const index = TABS.findIndex((entry) => entry.id === current);
  const last = TABS.length - 1;
  const at =
    key === "ArrowRight"
      ? (index + 1) % TABS.length
      : key === "ArrowLeft"
        ? (index + last) % TABS.length
        : key === "Home"
          ? 0
          : key === "End"
            ? last
            : undefined;
  return at === undefined ? undefined : TABS[at]?.id;
}

export function initialTab(): Tab {
  const wanted = new URLSearchParams(window.location.search).get("tab");
  return TABS.find((entry) => entry.id === wanted)?.id ?? "attention";
}

/** The section tabs, with roving focus and the pending counts as badges. */
export function TabBar({
  tab,
  attentionCount,
  orphanCount,
  onSelect,
}: {
  tab: Tab;
  attentionCount: number;
  orphanCount: number;
  onSelect: (tab: Tab) => void;
}): ReactElement {
  const { text } = useI18n();
  return (
    <div
      className="tabs"
      role="tablist"
      aria-label={text.a11y.sections}
      onKeyDown={(event) => {
        const next = nextTab(tab, event.key);
        if (next === undefined) return;
        event.preventDefault();
        onSelect(next);
        document.getElementById(tabId(next))?.focus();
      }}
    >
      {TABS.map(({ id, icon: Icon }) => {
        const count = id === "attention" ? attentionCount : id === "orphans" ? orphanCount : 0;
        return (
          <button
            key={id}
            type="button"
            role="tab"
            id={tabId(id)}
            className="tab"
            aria-selected={tab === id}
            aria-controls={panelId(id)}
            tabIndex={tab === id ? 0 : -1}
            onClick={() => {
              onSelect(id);
            }}
          >
            <Icon size={18} />
            <span>{text.tabs[id]}</span>
            {count > 0 && (
              <span className="badge" aria-label={text.a11y.pending(count)}>
                {count}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}
