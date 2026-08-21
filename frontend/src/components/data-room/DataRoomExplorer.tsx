import { useEffect, useState } from "react";
import { NavLink } from "react-router-dom";
import type { DataRoomTreeNode } from "../../data/dataRoom";
import type { DealExtractionLocationState } from "../../data/dealExtraction";
import { SidebarFrame } from "../hub/sidebar/SidebarFrame";
import { Icon } from "../ui/Icon";
import { DataRoomSidebarTabs } from "./DataRoomSidebarTabs";
import { NewAnalysisMenu } from "./NewAnalysisMenu";

type DataRoomExplorerProps = {
  dealName: string;
  dealRoomPath: string;
  email?: string;
  navigationState?: DealExtractionLocationState;
  nodes: DataRoomTreeNode[];
  onConnectToSharePoint: () => void;
  onSelectFile: (node: DataRoomTreeNode) => void;
  onUploadNewFile: () => void;
  rootPath?: string;
  selectedFilePath?: string;
  treeError?: string;
  treeLoading?: boolean;
};

export function DataRoomExplorer({
  dealName,
  dealRoomPath,
  email,
  navigationState,
  nodes,
  onConnectToSharePoint,
  onSelectFile,
  onUploadNewFile,
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
    <SidebarFrame
      alignedHeader
      centeredLogo
      email={email}
      headerBackLabel="Back to Deal Room"
      headerBackTo={dealRoomPath}
      navigationState={navigationState}
      profileSubtitle={dealName}
    >
      {({ collapsed }) =>
        collapsed ? (
          <div className="space-y-1">
            <DealRoomBackLink compact navigationState={navigationState} to={dealRoomPath} />
            <DataRoomSidebarTabs activeTab="data-room" compact />
          </div>
        ) : (
          <div className="flex min-h-full flex-col gap-3">
            <NewAnalysisMenu
              onConnectToSharePoint={onConnectToSharePoint}
              onUploadNewFile={onUploadNewFile}
            />
            <div className="space-y-1">
              <DealRoomBackLink navigationState={navigationState} to={dealRoomPath} />
              <DataRoomSidebarTabs activeTab="data-room" />
            </div>

            <div className="min-h-0 flex-1 pt-2">
              {rootPath ? (
                <p
                  className="mb-3 truncate px-3 text-[10px] font-semibold uppercase tracking-[0.1em] text-sidebar-muted"
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
          </div>
        )
      }
    </SidebarFrame>
  );
}

function DealRoomBackLink({
  compact = false,
  navigationState,
  to,
}: {
  compact?: boolean;
  navigationState?: DealExtractionLocationState;
  to: string;
}) {
  return (
    <NavLink
      aria-label="Deal Room"
      className={`flex items-center rounded-lg py-2 text-sidebar-text transition hover:bg-sidebar-hover hover:text-sidebar-active ${
        compact ? "justify-center px-0" : "gap-3 px-3"
      }`}
      state={navigationState}
      title="Deal Room"
      to={to}
    >
      <Icon className="h-5 w-5 shrink-0" name="dashboard" />
      {compact ? null : <span className="text-[13px] font-medium leading-5">Deal Room</span>}
    </NavLink>
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
          selected
            ? "bg-sidebar-selected text-sidebar-active"
            : "text-sidebar-text hover:bg-sidebar-hover hover:text-sidebar-active"
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
        <span className="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center text-sidebar-muted">
          {isFolder ? <Icon className="h-4 w-4" name={expanded ? "chevronDown" : "chevronRight"} /> : null}
        </span>
        <span className="mt-0.5 shrink-0 text-sidebar-muted">
          <Icon className="h-[18px] w-[18px]" name={iconNameForNode(node.kind)} />
        </span>
        <span
          className={`min-w-0 whitespace-normal break-words leading-snug [overflow-wrap:anywhere] ${
            isFolder ? "text-[13px] font-medium" : "text-[13px]"
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
    <div className="mx-1 mb-3 rounded-lg border border-outline-variant/70 bg-background p-3">
      <p className="text-[12px] font-semibold text-sidebar-active">{title}</p>
      <p className="mt-1 break-words text-[11px] leading-5 text-sidebar-muted">{detail}</p>
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
