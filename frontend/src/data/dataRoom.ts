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

export type DataRoomFileEntry = {
  folderPath: string[];
  node: DataRoomTreeNode;
  source: "local" | "stored";
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

export function flattenDataRoomFiles(
  nodes: readonly DataRoomTreeNode[],
): DataRoomFileEntry[] {
  const files: DataRoomFileEntry[] = [];

  function visit(items: readonly DataRoomTreeNode[], folderPath: string[]) {
    for (const node of items) {
      if (node.kind === "folder") {
        visit(node.children ?? [], [...folderPath, node.name]);
      } else {
        files.push({
          folderPath,
          node,
          source: node.storedFileId ? "stored" : "local",
        });
      }
    }
  }

  visit(nodes, []);
  return files;
}

export function isUnconfiguredDataRoomError(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return message.includes("no local data-room root is configured");
}

export function buildStoredDocumentNodes(
  documents: ReadonlyArray<{ displayName: string; fileId: string }>,
): DataRoomTreeNode[] {
  if (documents.length === 0) {
    return [];
  }

  return [
    {
      children: documents.map((document) => ({
        id: `stored-document:${document.fileId}`,
        kind: getStoredDocumentKind(document.displayName),
        name: document.displayName,
        relativePath: `stored-document:${document.fileId}`,
        storedFileId: document.fileId,
      })),
      defaultExpanded: true,
      id: "stored-documents",
      kind: "folder",
      name: "Saved documents",
    },
  ];
}

function getStoredDocumentKind(displayName: string): DataRoomTreeNode["kind"] {
  const extension = displayName.split(".").pop()?.toLowerCase();
  if (extension === "pdf") return "pdf";
  if (extension === "xls" || extension === "xlsx") return "sheet";
  return "doc";
}
