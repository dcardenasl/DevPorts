import {
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type MouseEvent,
  type ReactElement,
  type RefObject,
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
import type { Catalog } from "../i18n/es";
import { useI18n } from "../i18n/provider";
import { countdown, secondsUntil } from "../time";
import type { InstanceView } from "../types";
import { canRequestStop, instanceTitle } from "../instance";
import { InstanceFacts } from "./InstanceDetails";
import { Dot, Empty, PortChip, Tag, dotFor, stateLabel } from "./shared-parts";

/** Smooth scrolling is motion: it is skipped when the person asked for less. */
function prefersReducedMotion(): boolean {
  return (
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

export function projectsSubtitle(actions: Actions, text: Catalog): string {
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
  const { text } = useI18n();
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
  const { text } = useI18n();
  const quiet = node.runningCount === 0 && node.unknownCount === 0;
  const ports = occupiedPorts(node);
  const single = node.repos.length === 1 ? node.repos[0]?.repo : undefined;
  // One branch for every repo is said once, in the header; several are told apart per row.
  const branches = new Set(
    node.repos.flatMap((repo) => (repo.repo.branch === null ? [] : [repo.repo.branch])),
  );
  const sharedBranch = branches.size === 1 ? ([...branches][0] ?? null) : null;
  const place = [shortPath(node.summary.root), sharedBranch]
    .filter((part) => part !== null && part.length > 0)
    .join(" · ");
  return (
    <section className={quiet ? "group quiet" : "group"}>
      <button type="button" className="item" aria-expanded={open} onClick={onToggle}>
        <span className={open ? "chev open" : "chev"}>
          <ChevronIcon size={16} />
        </span>
        <span className="item-main">
          <span className="item-title">
            <span className="ellipsis">{node.summary.name}</span>
          </span>
          <span className="item-sub mono ellipsis">{place}</span>
        </span>
        {ports.length > 0 ? (
          <span className="item-end">
            {ports.slice(0, CHIPS_SHOWN).map((chip) => (
              <PortChip key={chip.port} port={chip.port} tone={chip.tone} />
            ))}
            {ports.length > CHIPS_SHOWN && <Tag>+{ports.length - CHIPS_SHOWN}</Tag>}
          </span>
        ) : (
          <span className="soft item-end">
            {node.unknownCount > 0 ? text.projects.unconfirmed : text.projects.noActive}
          </span>
        )}
      </button>
      {open && (
        <>
          {node.repos.map((repo) => (
            <div key={repo.repo.path}>
              {single === undefined && repo.services.length !== 1 && (
                <div className="item compact">
                  <FolderIcon size={14} />
                  <span className="mono grow ellipsis soft">{repo.repo.name}</span>
                  {repo.repo.branch !== null && <Tag>{repo.repo.branch}</Tag>}
                </div>
              )}
              {repo.services.map((service) => (
                <ServiceRows
                  key={service.name}
                  node={node}
                  service={service}
                  branch={branches.size > 1 && repo.services.length === 1 ? repo.repo.branch : null}
                  actions={actions}
                  highlight={highlight}
                />
              ))}
            </div>
          ))}
          <div className="item compact">
            <LinkIcon size={14} />
            <span className="grow row wrap">
              <span className="soft">{text.projects.uses}</span>
              {node.summary.uses.map((name) => (
                <button
                  key={name}
                  type="button"
                  className="tag"
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
          </div>
          <div className="item-actions">
            <button
              type="button"
              className="btn danger"
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
  branch,
  actions,
  highlight,
}: {
  node: ProjectNode;
  service: ServiceNode;
  branch: string | null;
  actions: Actions;
  highlight: string | undefined;
}): ReactElement {
  const { text } = useI18n();
  const principal = service.principal;
  const ref = useRef<HTMLDivElement>(null);
  const flagged = highlight !== undefined && principal?.ref === highlight;
  useEffect(() => {
    if (flagged)
      ref.current?.scrollIntoView({
        block: "center",
        behavior: prefersReducedMotion() ? "auto" : "smooth",
      });
  }, [flagged]);
  const duplicates = service.duplicates.filter((view) => !view.ignored);
  const summary = <ServiceSummary service={service} branch={branch} />;
  const buttons = <ServiceButtons node={node} service={service} actions={actions} />;

  return (
    <>
      {principal === undefined ? (
        <div className="item indent" ref={ref}>
          <span className="item-line">{summary}</span>
          {buttons}
        </div>
      ) : (
        <DetailedRow
          view={principal}
          className={flagged ? "item indent flagged" : "item indent"}
          rowRef={ref}
          main={summary}
          side={buttons}
        />
      )}
      {service.temporals.map((temporal) => (
        <TemporalRow key={temporal.ref} view={temporal} actions={actions} />
      ))}
      {duplicates.length > 0 && (
        <div className="item compact indent">
          <span className="grow ellipsis label is-red">
            {text.projects.duplicatesOf(duplicates.length)}
          </span>
          {duplicates.length > 1 && (
            <button
              type="button"
              className="btn danger"
              onClick={(event) => {
                actions.requestStop(duplicates, event.currentTarget);
              }}
            >
              {text.attention.stopAll(duplicates.length)}
            </button>
          )}
        </div>
      )}
      {duplicates.map((view) => (
        <DetailedRow
          key={view.ref}
          view={view}
          className="item indent"
          main={
            <>
              <Dot kind="red" />
              <span className="mono grow">:{view.port}</span>
            </>
          }
          side={
            <>
              <button
                type="button"
                className="btn danger"
                onClick={(event) => {
                  actions.requestStop([view], event.currentTarget);
                }}
              >
                {text.attention.stop}
              </button>
              <button
                type="button"
                className="btn"
                onClick={() => {
                  actions.keep(view);
                }}
              >
                {text.attention.keep}
              </button>
            </>
          }
        />
      ))}
    </>
  );
}

/** A row whose left part opens the instance's details below it; the actions stay apart. */
function DetailedRow({
  view,
  className,
  rowRef,
  main,
  side,
}: {
  view: InstanceView;
  className: string;
  rowRef?: RefObject<HTMLDivElement | null>;
  main: ReactElement;
  side: ReactElement;
}): ReactElement {
  const { text } = useI18n();
  const factsId = useId();
  const [open, setOpen] = useState(false);
  return (
    <div>
      <div className={className} ref={rowRef}>
        <button
          type="button"
          className="item-line"
          aria-expanded={open}
          aria-controls={factsId}
          aria-label={text.instance.detailsIdentity(instanceTitle(view, text), view.ref)}
          onClick={() => {
            setOpen(!open);
          }}
        >
          {main}
        </button>
        {side}
      </div>
      {open && (
        <div className="item-detail" id={factsId}>
          <InstanceFacts view={view} />
        </div>
      )}
    </div>
  );
}

function ServiceSummary({
  service,
  branch,
}: {
  service: ServiceNode;
  branch: string | null;
}): ReactElement {
  const { text } = useI18n();
  const principal = service.principal;
  const port = principal?.port ?? service.port;
  return (
    <>
      <Dot kind={principal === undefined ? "down" : dotFor(principal.state)} />
      <span className="name ellipsis">{service.name}</span>
      {branch !== null && <Tag>{branch}</Tag>}
      <span className={principal?.state === "unhealthy" ? "is-red" : "soft"}>
        {principal === undefined ? stateLabel("down", text) : stateLabel(principal.state, text)}
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
  const { text } = useI18n();
  const principal = service.principal;
  const running = canRequestStop(principal);
  return (
    <span className="item-end tight">
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
  const { text } = useI18n();
  const left = secondsUntil(view.expires_at, actions.now) ?? 0;
  return (
    <DetailedRow
      view={view}
      className="item indent"
      main={
        <>
          <Dot kind={dotFor(view.state)} />
          <span className="name">{text.projects.temporal}</span>
          <span className="soft grow ellipsis">{view.reason}</span>
          <span className="data is-amber">
            {view.state === "healthy" || view.state === "unhealthy"
              ? countdown(left)
              : stateLabel(view.state, text)}
          </span>
        </>
      }
      side={
        canRequestStop(view) ? (
          <IconButton
            label={text.attention.terminate}
            danger
            onClick={(event) => {
              actions.requestStop([view], event.currentTarget);
            }}
          >
            <StopIcon size={14} />
          </IconButton>
        ) : (
          <></>
        )
      }
    />
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
