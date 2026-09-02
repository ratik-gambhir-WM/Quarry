export type DataRoomTreeNode = {
  children?: DataRoomTreeNode[];
  defaultExpanded?: boolean;
  error?: string;
  id: string;
  kind: "doc" | "folder" | "pdf" | "sheet";
  name: string;
  relativePath?: string;
  storedFileId?: string;
};

export type DataRoomChip =
  | {
      body: string;
      category: string;
      id: string;
      tone: "accent" | "muted" | "primary";
      type: "text";
    }
  | {
      bars: number[];
      category: string;
      footer: string;
      id: string;
      tone: "muted" | "primary";
      type: "chart";
    }
  | {
      category: string;
      id: string;
      rows: Array<{ label: string; value: string }>;
      tone: "accent" | "primary";
      type: "metrics";
    };

export type EditorBlock =
  | { id: string; text: string; type: "paragraph" }
  | { id: string; text: string; type: "heading" }
  | { id: string; text: string; type: "quote" }
  | { id: string; type: "dropzone" }
  | {
      columns: Array<{
        items: string[];
        title: string;
        tone: "error" | "primary";
      }>;
      id: string;
      type: "callouts";
    };

export type DealDataRoomView = {
  chips: DataRoomChip[];
  editorBlocks: EditorBlock[];
  reportTitle: string;
  tree: DataRoomTreeNode[];
  versionLabel: string;
};

export function hasDataRoomFiles(nodes: DataRoomTreeNode[]): boolean {
  const pending = [...nodes];

  while (pending.length > 0) {
    const node = pending.pop();
    if (!node) continue;
    if (node.kind !== "folder") return true;
    if (node.children) pending.push(...node.children);
  }

  return false;
}

export function isUnconfiguredDataRoomError(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return message.includes("no local data-room root is configured");
}
