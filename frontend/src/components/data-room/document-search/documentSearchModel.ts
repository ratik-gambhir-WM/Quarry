export type DocumentSearchTarget = {
  kind: "pdf-page";
  page: number;
};

export type DocumentSearchItem = {
  disabledReason?: string;
  id: string;
  imageAlt?: string;
  imageUrl?: string;
  primaryText: string;
  searchText?: string;
  secondaryText?: string;
  tertiaryText?: string;
};

export type DocumentSearchResult = {
  excerpt: string;
  fileName: string;
  id: string;
  location: string;
  target?: DocumentSearchTarget;
};

export function isDocumentSearchResultAvailable({
  currentFileName,
  numPages,
  result,
}: {
  currentFileName: string;
  numPages: number;
  result: DocumentSearchResult;
}): boolean {
  return (
    result.fileName === currentFileName &&
    result.target?.kind === "pdf-page" &&
    result.target.page >= 1 &&
    result.target.page <= numPages
  );
}

export function formatDocumentSearchFileName(fileName: string): string {
  return fileName.replace(/_/g, " ");
}
