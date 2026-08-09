import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { DataRoomTreeNode } from "../../data/dataRoom";
import type { DealExtractionLocationState } from "../../data/dealExtraction";
import { Icon } from "../ui/Icon";
import { ArrowEndOnRectangleIcon } from "../ui/icons/ArrowEndOnRectangleIcon";
import { DataRoomSidebarTabs } from "./DataRoomSidebarTabs";
import { NewAnalysisMenu } from "./NewAnalysisMenu";

type DataRoomExplorerProps = {
  collapsed: boolean;
  dealName: string;
  dealRoomPath: string;
  navigationState?: DealExtractionLocationState;
  nodes: DataRoomTreeNode[];
  onCollapse: () => void;
  onUploadNewFile: () => void;
  onSelectFile: (node: DataRoomTreeNode) => void;
  rootPath?: string;
  selectedFilePath?: string;
  treeError?: string;
  treeLoading?: boolean;
};

export function DataRoomExplorer({
  collapsed,
  dealName,
  dealRoomPath,
  navigationState,
  nodes,
  onCollapse,
  onUploadNewFile,
  onSelectFile,
  rootPath,
  selectedFilePath,
  treeError,
  treeLoading = false,
}: DataRoomExplorerProps) {
  const [expandedNodeIds, setExpandedNodeIds] = useState<Set<string>>(() => new Set());

  useEffect(() => {
    setExpandedNodeIds((current) => {
      const next = new Set(current);
      collectDefaultExpandedNodeIds(nodes, next);
      return next;
    });
  }, [nodes]);

  function toggleNode(nodeId: string) {
    setExpandedNodeIds((current) => {
      const next = new Set(current);

      if (next.has(nodeId)) {
        next.delete(nodeId);
      } else {
        next.add(nodeId);
      }

      return next;
    });
  }

  return (
    <aside
      aria-hidden={collapsed}
      className={`flex shrink-0 overflow-hidden bg-background transition-[width,border-color] duration-300 ${
        collapsed ? "w-0 border-r-0" : "w-72 border-r border-white/80"
      }`}
    >
      <div className={`flex h-full w-72 shrink-0 flex-col ${collapsed ? "pointer-events-none invisible" : "visible"}`}>
        <div className="flex h-16 shrink-0 items-center justify-between border-b border-outline-variant/70 px-4">
          <Link
            aria-label={`Back to ${dealName} deal room`}
            className="flex h-10 w-10 items-center justify-center rounded-full text-primary transition hover:bg-white/70"
            state={navigationState}
            to={dealRoomPath}
          >
            <Icon className="h-5 w-5" name="home" />
          </Link>
          <span className="truncate text-[1rem] font-bold text-text-main [font-family:var(--font-heading)]">
            Data Room
          </span>
          <button
            aria-label="Collapse data room sidebar"
            className="flex h-10 w-10 items-center justify-center rounded-full text-muted transition hover:bg-white/70 hover:text-text-main"
            onClick={onCollapse}
            title="Collapse data room sidebar"
            type="button"
          >
            <ArrowEndOnRectangleIcon className="h-6 w-6" direction="left" />
          </button>
        </div>

        <div className="flex min-h-0 flex-1 flex-col gap-3 p-4">
          <NewAnalysisMenu onUploadNewFile={onUploadNewFile} />

          <div className="mb-2">
            <DataRoomSidebarTabs activeTab="data-room" />
          </div>

          <div className="workspace-scrollbar-hidden min-h-0 flex-1 overflow-y-auto pr-1">
            {rootPath ? (
              <p
                className="mb-3 truncate px-2 text-[10px] font-semibold uppercase tracking-[0.1em] text-muted/80"
                title={rootPath}
              >
                Local · {rootPath}
              </p>
            ) : null}
            {treeLoading ? (
              <ExplorerStatus detail="Reading the configured local folder…" title="Loading data room" />
            ) : null}
            {treeError ? <ExplorerStatus detail={treeError} title="Data room unavailable" /> : null}
            <div className="space-y-1">
              {nodes.map((node) => (
                <ExplorerNodeItem
                  depth={0}
                  expandedNodeIds={expandedNodeIds}
                  key={node.id}
                  node={node}
                  onSelectFile={onSelectFile}
                  onToggle={toggleNode}
                  selectedFilePath={selectedFilePath}
                />
              ))}
            </div>
          </div>

          <div className="mt-auto border-t border-white/50 pt-4">
            <div className="flex items-center gap-3 rounded-xl px-3 py-2 transition hover:bg-white/50">
              <div className="flex h-9 w-9 items-center justify-center rounded-full bg-secondary-fixed-dim text-sm font-semibold text-white">
                A
              </div>
              <div className="flex flex-col">
                <span className="text-sm font-medium text-text-main">Analyst Team</span>
                <span className="text-[11px] font-bold uppercase tracking-[0.14em] text-muted">{dealName}</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </aside>
  );
}

type ExplorerNodeItemProps = {
  depth: number;
  expandedNodeIds: Set<string>;
  node: DataRoomTreeNode;
  onSelectFile: (node: DataRoomTreeNode) => void;
  onToggle: (nodeId: string) => void;
  selectedFilePath?: string;
};

function ExplorerNodeItem({
  depth,
  expandedNodeIds,
  node,
  onSelectFile,
  onToggle,
  selectedFilePath,
}: ExplorerNodeItemProps) {
  const isFolder = node.kind === "folder";
  const expanded = expandedNodeIds.has(node.id);
  const selected = Boolean(node.relativePath && node.relativePath === selectedFilePath);

  return (
    <div>
      <button
        aria-current={selected ? "true" : undefined}
        className={`flex w-full items-start gap-2 rounded-lg px-2 py-1.5 text-left transition ${
          selected ? "bg-primary/10 text-text-main" : "hover:bg-white/40"
        }`}
        onClick={() => {
          if (isFolder) {
            onToggle(node.id);
          } else {
            onSelectFile(node);
          }
        }}
        style={{ paddingLeft: `${depth * 18 + 8}px` }}
        title={node.error ? `${node.name} — ${node.error}` : node.name}
        type="button"
      >
        <span className="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center text-muted">
          {isFolder ? <Icon className="h-4 w-4" name={expanded ? "chevronDown" : "chevronRight"} /> : null}
        </span>
        <span className="mt-0.5 shrink-0 text-primary">
          <Icon className="h-[18px] w-[18px]" name={iconNameForNode(node.kind)} />
        </span>
        <span
          className={`min-w-0 whitespace-normal break-words leading-snug [overflow-wrap:anywhere] ${
            isFolder ? "text-[14px] font-medium text-text-main" : "text-[14px] text-text-main/80"
          }`}
        >
          {node.name}
        </span>
        {node.error ? <span className="ml-auto shrink-0 text-error">!</span> : null}
      </button>

      {isFolder && expanded ? (
        <div className="space-y-0.5">
          {node.children?.map((child) => (
            <ExplorerNodeItem
              depth={depth + 1}
              expandedNodeIds={expandedNodeIds}
              key={child.id}
              node={child}
              onSelectFile={onSelectFile}
              onToggle={onToggle}
              selectedFilePath={selectedFilePath}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}

function collectDefaultExpandedNodeIds(nodes: DataRoomTreeNode[], target: Set<string>) {
  for (const node of nodes) {
    if (node.defaultExpanded) {
      target.add(node.id);
    }
    if (node.children) {
      collectDefaultExpandedNodeIds(node.children, target);
    }
  }
}

function ExplorerStatus({ detail, title }: { detail: string; title: string }) {
  return (
    <div className="mx-1 mb-3 rounded-xl border border-outline-variant bg-surface-container-lowest p-3">
      <p className="text-[12px] font-semibold text-text-main">{title}</p>
      <p className="mt-1 break-words text-[11px] leading-5 text-muted">{detail}</p>
    </div>
  );
}

function iconNameForNode(kind: DataRoomTreeNode["kind"]) {
  switch (kind) {
    case "folder":
      return "folderOpen";
    case "pdf":
      return "pdf";
    case "sheet":
      return "sheet";
    default:
      return "doc";
  }
}
