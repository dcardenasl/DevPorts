import type { ReactElement } from "react";

import { useI18n } from "./i18n/provider";
import { GearIcon } from "./icons";
import { panelId, tabId, type Tab } from "./tabs";

export function Shell({
  title,
  subtitle,
  onSettings,
  body,
  footer,
  overlay,
  tabKey,
}: {
  title: string;
  subtitle: string;
  onSettings?: (invoker: HTMLButtonElement) => void;
  body: ReactElement | null;
  footer?: ReactElement;
  overlay?: ReactElement;
  tabKey?: string;
}): ReactElement {
  const { text } = useI18n();
  return (
    <div className="panel">
      <header className="head">
        <div className="head-row">
          <h1>{title}</h1>
          {onSettings && (
            <button
              type="button"
              className="icon-btn"
              aria-label={text.settings.open}
              title={text.settings.open}
              onClick={(event) => {
                onSettings(event.currentTarget);
              }}
            >
              <GearIcon size={16} />
            </button>
          )}
        </div>
        <p>{subtitle}</p>
      </header>
      <main
        className="body"
        key={tabKey}
        role="tabpanel"
        id={tabKey === undefined ? undefined : panelId(tabKey as Tab)}
        aria-labelledby={tabKey === undefined ? undefined : tabId(tabKey as Tab)}
      >
        {body}
      </main>
      {footer ?? <div />}
      {overlay}
    </div>
  );
}
