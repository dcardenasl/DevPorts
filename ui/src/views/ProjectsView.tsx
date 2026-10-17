import { useEffect, useMemo, useRef, type MouseEvent, type ReactElement } from "react";

import type { Actions } from "../actions";
import { ChevronIcon, CopyIcon, ExternalIcon, FolderIcon, LinkIcon, StopIcon } from "../icons";
import { buildProjects, startCommand, type ProjectNode, type ServiceNode } from "../model";
import { text } from "../text";
import { countdown, secondsUntil } from "../time";
import type { InstanceView } from "../types";
import { Dot, Empty, Tag, dotFor, stateLabel } from "./shared-parts";

export function projectsSubtitle(actions: Actions): string {
  const nodes = buildProjects(actions.snapshot);
  const repos = nodes.reduce((sum, node) => sum + node.repos.length, 0);
  const services = nodes.reduce((sum, node) => sum + node.serviceCount, 0);
  return text.projects.summary(nodes.length, repos, services);
}

export function ProjectsView({
  actions,
  highlight,
  expandedProject,
  onExpandedProjectChange,
}: {
  actions: Actions;
  highlight: string | undefined;
  expandedProject: string | null;
  onExpandedProjectChange: (name: string | null) => void;
}): ReactElement {
  const nodes = useMemo(() => buildProjects(actions.snapshot), [actions.snapshot]);
  const open = expandedProject;

  // Showing a service from "Atención" opens its project in the shell state.
  const seenHighlight = useRef(highlight);
  useEffect(() => {
    if (seenHighlight.current === highlight) return;
    seenHighlight.current = highlight;
    if (highlight === undefined) return;
    const owner = nodes.find((node) =>
      node.repos.some((repo) =>
        repo.services.some((service) => service.principal?.ref === highlight),
      ),
    );
    if (owner !== undefined) onExpandedProjectChange(owner.summary.name);
  }, [highlight, nodes, onExpandedProjectChange]);

  if (nodes.length === 0) {
    return <Empty title={text.projects.none} body={text.projects.noneBody} />;
  }
  return (
    <>
      {nodes.map((node) => (
        <ProjectCard
          key={node.summary.name}
          node={node}
          open={open === node.summary.name}
          onToggle={() => {
            onExpandedProjectChange(open === node.summary.name ? null : node.summary.name);
          }}
          actions={actions}
          highlight={highlight}
        />
      ))}
    </>
  );
}

function ProjectCard({
  node,
  open,
  onToggle,
  actions,
  highlight,
}: {
  node: ProjectNode;
  open: boolean;
  onToggle: () => void;
  actions: Actions;
  highlight: string | undefined;
}): ReactElement {
  const quiet = node.runningCount === 0;
  return (
    <section className={quiet ? "project dim-project" : "project"}>
      <button type="button" className="project-head" aria-expanded={open} onClick={onToggle}>
        <span className={open ? "chev open" : "chev"}>
          <ChevronIcon size={16} />
        </span>
        <span className="grow">
          <div className="name ellipsis">{node.summary.name}</div>
          <div className="dim mono ellipsis" style={{ fontSize: 11 }}>
            {shortPath(node.summary.root)}
          </div>
        </span>
        <span className="soft" style={{ fontSize: 12 }}>
          {quiet
            ? text.projects.noActive
            : `${node.repos.length} ${node.repos.length === 1 ? "repo" : "repos"} · ${node.serviceCount} ${node.serviceCount === 1 ? "servicio" : "servicios"}`}
        </span>
      </button>
      {open && (
        <>
          {node.repos.map((repo) => (
            <div key={repo.repo.path}>
              <div className="repo">
                <FolderIcon size={14} />
                <span className="mono grow ellipsis">{repo.repo.name}</span>
                {repo.repo.branch !== null && <Tag>{repo.repo.branch}</Tag>}
              </div>
              {repo.services.map((service) => (
                <ServiceRows
                  key={service.name}
                  node={node}
                  service={service}
                  actions={actions}
                  highlight={highlight}
                />
              ))}
            </div>
          ))}
          {node.summary.uses.length > 0 && (
            <div className="uses">
              <LinkIcon size={14} />
              <span>{text.projects.uses}</span>
              {node.summary.uses.map((name) => (
                <Tag key={name} tone="blue">
                  {name}
                </Tag>
              ))}
            </div>
          )}
        </>
      )}
    </section>
  );
}

function ServiceRows({
  node,
  service,
  actions,
  highlight,
}: {
  node: ProjectNode;
  service: ServiceNode;
  actions: Actions;
  highlight: string | undefined;
}): ReactElement {
  const principal = service.principal;
  const running = principal !== undefined && principal.state !== "down";
  const ref = useRef<HTMLDivElement>(null);
  const flagged = highlight !== undefined && principal?.ref === highlight;
  useEffect(() => {
    if (flagged) ref.current?.scrollIntoView({ block: "center", behavior: "smooth" });
  }, [flagged]);

  return (
    <>
      <div className={flagged ? "svc highlight" : "svc"} ref={ref}>
        <Dot kind={principal === undefined ? "down" : dotFor(principal.state)} />
        <span className="name" style={{ fontSize: 14 }}>
          {service.name}
        </span>
        <span
          className={
            principal?.state === "unhealthy" ? "coral-text" : running ? "green-text" : "dim"
          }
          style={{ fontSize: 12 }}
        >
          {principal === undefined ? text.projects.down : stateLabel(principal.state)}
        </span>
        <span className="grow" />
        <span className="mono soft">
          {principal?.port === null || principal === undefined
            ? service.port === null
              ? ""
              : `:${service.port}`
            : `:${principal.port}`}
        </span>
        <ServiceButtons node={node} service={service} actions={actions} />
      </div>
      {service.temporals.map((temporal) => (
        <TemporalRow key={temporal.ref} view={temporal} actions={actions} />
      ))}
      {service.duplicates
        .filter((view) => !view.ignored)
        .map((view) => (
          <div className="svc dup" key={view.ref}>
            <Dot kind="coral" />
            <span className="grow ellipsis">
              {service.name} · <span className="coral-text">{text.projects.unregistered}</span>
            </span>
            <span className="mono coral-text">:{view.port}</span>
            <button
              type="button"
              className="btn"
              style={{ height: 26, padding: "0 10px" }}
              onClick={(event) => {
                actions.requestStop([view], event.currentTarget);
              }}
            >
              {text.attention.stop}
            </button>
            <button
              type="button"
              className="btn"
              style={{ height: 26, padding: "0 10px" }}
              onClick={() => {
                actions.keep(view);
              }}
            >
              {text.attention.keep}
            </button>
          </div>
        ))}
    </>
  );
}

function ServiceButtons({
  node,
  service,
  actions,
}: {
  node: ProjectNode;
  service: ServiceNode;
  actions: Actions;
}): ReactElement {
  const principal = service.principal;
  const running = principal !== undefined && principal.state !== "down" && principal.ref !== "-";
  return (
    <span className="row" style={{ gap: 4 }}>
      {running && principal.url !== null && (
        <IconButton
          label={text.projects.open}
          onClick={() => {
            actions.openUrl(principal.url ?? "");
          }}
        >
          <ExternalIcon size={14} />
        </IconButton>
      )}
      {!running && (
        <IconButton
          label={text.projects.copy}
          onClick={() => {
            actions.copy(startCommand(node.summary, service.name));
          }}
        >
          <CopyIcon size={14} />
        </IconButton>
      )}
      {running && (
        <IconButton
          label={`${text.projects.stop} ${service.name}`}
          danger
          onClick={(event) => {
            actions.requestStop([principal], event.currentTarget);
          }}
        >
          <StopIcon size={14} />
        </IconButton>
      )}
    </span>
  );
}

function TemporalRow({ view, actions }: { view: InstanceView; actions: Actions }): ReactElement {
  const left = secondsUntil(view.expires_at, actions.now) ?? 0;
  return (
    <div className="svc nested">
      <span className="dot amber" aria-hidden="true" />
      <span className="name" style={{ fontSize: 13 }}>
        {text.projects.temporal}
      </span>
      <span className="soft grow ellipsis">{view.reason}</span>
      <span className="mono amber-text">{countdown(left)}</span>
      <IconButton
        label={text.attention.terminate}
        danger
        onClick={(event) => {
          actions.requestStop([view], event.currentTarget);
        }}
      >
        <StopIcon size={14} />
      </IconButton>
    </div>
  );
}

export function IconButton({
  label,
  onClick,
  children,
  danger = false,
}: {
  label: string;
  onClick: (event: MouseEvent<HTMLButtonElement>) => void;
  children: ReactElement;
  danger?: boolean;
}): ReactElement {
  return (
    <button
      type="button"
      className={danger ? "icon-btn danger" : "icon-btn"}
      aria-label={label}
      title={label}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

function shortPath(path: string): string {
  const parts = path.split("/").filter((part) => part.length > 0);
  return parts.slice(-2).join("/");
}
