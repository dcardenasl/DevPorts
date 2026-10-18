import type { ReactElement } from "react";

import { commandLine, instanceTitle, stateLabel } from "../instance";
import { text } from "../text";
import type { InstanceView } from "../types";

/** Reusable, read-only diagnostics. No inferred ancestry, missing ports or actions. */
export function InstanceDetails({ view }: { view: InstanceView }): ReactElement {
  return (
    <details className="instance-details">
      <summary aria-label={text.instance.detailsIdentity(instanceTitle(view), view.ref)}>
        {text.instance.details(instanceTitle(view))}
        {view.stale !== null && <span className="amber-text"> · {text.instance.stale}</span>}
      </summary>
      <InstanceFacts view={view} />
    </details>
  );
}

/** The diagnostics themselves, for rows that act as their own trigger. */
export function InstanceFacts({ view }: { view: InstanceView }): ReactElement {
  const missing = text.instance.unavailable;
  const ports = view.ports ?? [];
  const facts: [string, string][] = [
    [text.instance.state, stateLabel(view.state)],
    [text.instance.role, text.instance.roles[view.role]],
    [
      text.instance.origin,
      view.shared ? text.confirm.origin(view) : (view.project ?? text.instance.noProject),
    ],
    [text.instance.reference, view.ref],
    [text.instance.identity, view.target_identity ?? missing],
    [text.instance.started, view.started_at ?? missing],
    [text.instance.ports, ports.length === 0 ? text.instance.noObservedPorts : ports.join(", ")],
    [text.instance.folder, view.cwd ?? missing],
    [
      text.instance.registration,
      view.registered ? text.instance.registered : text.instance.observed,
    ],
  ];
  const optionalFacts: [string, string | null][] = [
    [text.instance.pid, view.pid === null ? null : String(view.pid)],
    [
      text.instance.referencePort,
      view.port === null || ports.includes(view.port) ? null : String(view.port),
    ],
    [text.instance.worktree, view.worktree],
    [text.instance.branch, view.branch],
    [text.instance.container, view.container],
    [text.instance.compose, view.compose],
    [text.instance.image, view.image],
  ];
  for (const [label, value] of optionalFacts) {
    if (value !== null) facts.push([label, value]);
  }
  return (
    <div className="instance-facts">
      <dl>
        {facts.map(([label, value]) => (
          <div key={label}>
            <dt>{label}</dt>
            <dd>{value}</dd>
          </div>
        ))}
      </dl>
      <div className="soft">{text.instance.launcher}</div>
      {view.launcher_chain.length === 0 ? (
        <p>{text.instance.unknownLauncher}</p>
      ) : (
        <ol className="instance-chain">
          {view.launcher_chain.map((link, index) => (
            <li key={`${index}-${link.pid}`}>
              {link.label ?? link.name} · {text.instance.pid} {link.pid}
            </li>
          ))}
        </ol>
      )}
      {view.stale !== null && (
        <p className="instance-stale">
          <strong>{text.instance.stale}</strong> {view.stale}. {text.instance.staleBody}
        </p>
      )}
      <div className="soft">{text.instance.command}</div>
      <code className="instance-command">{commandLine(view.argv)}</code>
      <p className="dim">{text.instance.commandPrivacy}</p>
    </div>
  );
}
