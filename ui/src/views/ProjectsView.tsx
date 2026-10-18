import {
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type MouseEvent,
  type ReactElement,
} from "react";

import type { Actions } from "../actions";
import { ChevronIcon, CopyIcon, ExternalIcon, FolderIcon, LinkIcon, StopIcon } from "../icons";
import {
  buildProjects,
  occupiedPorts,
  orderProjects,
  startCommand,
  type ProjectNode,
  type ServiceNode,
} from "../model";
import { isOpen, revealed, setAll, toggled, type ProjectFolds } from "../projectFolds";
import { text } from "../text";
import { countdown, secondsUntil } from "../time";
import type { InstanceView } from "../types";
import { canRequestStop, instanceTitle } from "../instance";
import { InstanceDetails, InstanceFacts } from "./InstanceDetails";
import { Dot, Empty, Tag, dotFor, stateLabel } from "./shared-parts";

/** Smooth scrolling is motion: it is skipped when the person asked for less. */
function prefersReducedMotion(): boolean {
  return (
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

export function projectsSubtitle(actions: Actions): string {
  const nodes = buildProjects(actions.snapshot);
  const repos = nodes.reduce((sum, node) => sum + node.repos.length, 0);
  const services = nodes.reduce((sum, node) => sum + node.serviceCount, 0);
  return text.projects.summary(nodes.length, repos, services);
}

export function ProjectsView({
  actions,
  highlight,
  folds,
  onFoldsChange,
}: {
  actions: Actions;
  highlight: string | undefined;
  folds: ProjectFolds;
  onFoldsChange: (folds: ProjectFolds) => void;
}): ReactElement {
  const nodes = useMemo(() => orderProjects(buildProjects(actions.snapshot)), [actions.snapshot]);

  // Showing a service from "Atención" opens its project and leaves the others as they are.
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
    if (owner !== undefined) onFoldsChange(revealed(folds, owner.summary.name));
  }, [highlight, nodes, folds, onFoldsChange]);

  if (nodes.length === 0) {
    return <Empty title={text.projects.none} body={text.projects.noneBody} />;
  }
  const allOpen = nodes.every((node) => isOpen(folds, node));
  return (
    <>
      <div className="projects-bar">
        <button
          type="button"
          className="btn"
          onClick={() => {
            onFoldsChange(setAll(nodes, !allOpen));
          }}
        >
          {allOpen ? text.projects.foldAll : text.projects.openAll}
        </button>
      </div>
      {nodes.map((node) => (
        <ProjectCard
          key={node.summary.name}
          node={node}
          open={isOpen(folds, node)}
          onToggle={() => {
            onFoldsChange(toggled(folds, node));
          }}
          actions={actions}
          highlight={highlight}
        />
      ))}
    </>
  );
}

/** Ports shown in a folded header before «+N». */
const CHIPS_SHOWN = 3;

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
  const quiet = node.runningCount === 0 && node.unknownCount === 0;
  const ports = occupiedPorts(node);
  const single = node.repos.length === 1 ? node.repos[0]?.repo : undefined;
  const place = [shortPath(node.summary.root), single?.branch ?? null]
    .filter((part) => part !== null && part.length > 0)
    .join(" · ");
  return (
    <section className={quiet ? "project dim-project" : "project"}>
      <button type="button" className="project-head" aria-expanded={open} onClick={onToggle}>
        <span className={open ? "chev open" : "chev"}>
          <ChevronIcon size={16} />
        </span>
        <span className="name ellipsis" style={{ flex: "0 1 auto" }}>
          {node.summary.name}
        </span>
        <span className="where dim mono ellipsis" style={{ fontSize: 11 }}>
          {place}
        </span>
        {ports.length > 0 ? (
          <span className="ports">
            {ports.slice(0, CHIPS_SHOWN).map((chip) => (
              <Tag key={chip.port} tone={chip.tone}>
                :{chip.port}
              </Tag>
            ))}
            {ports.length > CHIPS_SHOWN && <Tag>+{ports.length - CHIPS_SHOWN}</Tag>}
          </span>
        ) : (
          <span className="soft" style={{ fontSize: 12 }}>
            {node.unknownCount > 0 ? text.projects.unconfirmed : text.projects.noActive}
          </span>
        )}
      </button>
      {open && (
        <>
          {node.repos.map((repo) => (
            <div key={repo.repo.path}>
              {single === undefined && (
                <div className="repo">
                  <FolderIcon size={14} />
                  <span className="mono grow ellipsis">{repo.repo.name}</span>
                  {repo.repo.branch !== null && <Tag>{repo.repo.branch}</Tag>}
                </div>
              )}
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
          <div className="project-foot">
            <LinkIcon size={14} />
            <span className="uses-list">
              <span>{text.projects.uses}</span>
              {node.summary.uses.map((name) => (
                <button
                  key={name}
                  type="button"
                  className="tag blue"
                  aria-label={text.consumers.manageFor(name)}
                  onClick={(event) => {
                    actions.manageConsumers(name, event.currentTarget, node.summary.name);
                  }}
                >
                  {name}
                </button>
              ))}
              {actions.snapshot.shared
                .filter((item) => !node.summary.uses.includes(item.name))
                .map((item) => (
                  <button
                    key={item.name}
                    type="button"
                    className="tag add"
                    aria-label={text.consumers.associate(item.name)}
                    onClick={(event) => {
                      actions.manageConsumers(item.name, event.currentTarget, node.summary.name);
                    }}
                  >
                    + {item.name}
                  </button>
                ))}
            </span>
            <button
              type="button"
              className="btn"
              disabled={actions.preparingProjectStop}
              onClick={(event) => {
                actions.requestProjectStop(node.summary.name, event.currentTarget);
              }}
            >
              {actions.preparingProjectStop
                ? text.projects.preparingStop
                : text.projects.stopProject}
            </button>
          </div>
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
  const ref = useRef<HTMLDivElement>(null);
  const factsId = useId();
  const [showFacts, setShowFacts] = useState(false);
  const flagged = highlight !== undefined && principal?.ref === highlight;
  useEffect(() => {
    if (flagged)
      ref.current?.scrollIntoView({
        block: "center",
        behavior: prefersReducedMotion() ? "auto" : "smooth",
      });
  }, [flagged]);

  return (
    <>
      <div className={flagged ? "svc highlight" : "svc"} ref={ref}>
        {principal === undefined ? (
          <span className="svc-main">
            <ServiceSummary service={service} />
          </span>
        ) : (
          <button
            type="button"
            className="svc-main"
            aria-expanded={showFacts}
            aria-controls={factsId}
            aria-label={text.instance.detailsIdentity(instanceTitle(principal), principal.ref)}
            onClick={() => {
              setShowFacts(!showFacts);
            }}
          >
            <ServiceSummary service={service} />
          </button>
        )}
        <ServiceButtons node={node} service={service} actions={actions} />
      </div>
      {principal !== undefined && showFacts && (
        <div className="svc-facts" id={factsId}>
          <InstanceFacts view={principal} />
        </div>
      )}
      {service.temporals.map((temporal) => (
        <TemporalRow key={temporal.ref} view={temporal} actions={actions} />
      ))}
      {service.duplicates
        .filter((view) => !view.ignored)
        .map((view) => (
          <div key={view.ref}>
            <div className="svc dup">
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
            <InstanceDetails view={view} />
          </div>
        ))}
    </>
  );
}

function ServiceSummary({ service }: { service: ServiceNode }): ReactElement {
  const principal = service.principal;
  const port = principal?.port ?? service.port;
  return (
    <>
      <Dot kind={principal === undefined ? "down" : dotFor(principal.state)} />
      <span className="name ellipsis" style={{ fontSize: 14 }}>
        {service.name}
      </span>
      <span
        className={
          principal?.state === "unhealthy"
            ? "coral-text"
            : principal?.state === "healthy"
              ? "green-text"
              : "dim"
        }
        style={{ fontSize: 12 }}
      >
        {principal === undefined ? stateLabel("down") : stateLabel(principal.state)}
      </span>
      <span className="grow" />
      <span className="mono soft">{port === null ? "" : `:${port}`}</span>
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
  const running = canRequestStop(principal);
  return (
    <span className="row" style={{ gap: 4 }}>
      {running && principal !== undefined && principal.url !== null && (
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
      {running && principal !== undefined && (
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
    <div>
      <div className="svc nested">
        <Dot kind={dotFor(view.state)} />
        <span className="name" style={{ fontSize: 13 }}>
          {text.projects.temporal}
        </span>
        <span className="soft grow ellipsis">{view.reason}</span>
        <span className="mono amber-text">
          {view.state === "healthy" || view.state === "unhealthy"
            ? countdown(left)
            : stateLabel(view.state)}
        </span>
        {canRequestStop(view) && (
          <IconButton
            label={text.attention.terminate}
            danger
            onClick={(event) => {
              actions.requestStop([view], event.currentTarget);
            }}
          >
            <StopIcon size={14} />
          </IconButton>
        )}
      </div>
      <InstanceDetails view={view} />
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
