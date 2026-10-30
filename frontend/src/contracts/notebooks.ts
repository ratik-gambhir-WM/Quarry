import type { NotebookBlock } from "../components/notebook/notebookSchema";

export type Notebook = {
  notebookId: string;
  title: string;
  dealId: string | null;
  createdAt: string;
  updatedAt: string;
};
export type NotebookFolder = {
  folderId: string;
  notebookId: string;
  name: string;
  createdAt: string;
  updatedAt: string;
};
export type NoteSummary = {
  noteId: string;
  notebookId: string;
  folderId: string | null;
  title: string;
  contentSchemaVersion: 1;
  revision: number;
  createdAt: string;
  updatedAt: string;
};
export type PersistedNote = NoteSummary & { blocks: NotebookBlock[] };
export type Pagination = { limit?: number; offset?: number };
export type NotebookListOptions = Pagination & { dealId?: string | "none" };
export type NoteListOptions = Pagination & {
  folderId?: string | "none";
  q?: string;
};
export type PageMetadata = { limit: number; offset: number; hasMore: boolean };
export type NotebookPage = PageMetadata & { notebooks: Notebook[] };
export type FolderPage = PageMetadata & { folders: NotebookFolder[] };
export type NotePage = PageMetadata & { notes: NoteSummary[] };
export type CreateNotebook = { title: string; dealId?: string | null };
export type PatchNotebook = { title?: string; dealId?: string | null };
export type CreateNote = {
  operationId: string;
  title?: string;
  folderId?: string | null;
  blocks?: NotebookBlock[];
  contentSchemaVersion?: 1;
};
export type PatchNote = {
  operationId: string;
  expectedRevision: number;
  title?: string;
  folderId?: string | null;
} & (
  | { blocks: NotebookBlock[]; contentSchemaVersion: 1 }
  | { blocks?: never; contentSchemaVersion?: never }
);
export type NotebookSaveJob = {
  jobId: string;
  notebookId: string;
  noteId: string;
  status: "queued" | "processing" | "completed" | "failed";
  stage: "persisting" | "exporting" | "chunking" | "embedding" | "indexing";
  attempt: number;
  sequence: number;
  committedRevision: number | null;
  sqliteCommitted: boolean;
  errorCode: string | null;
  retryable: boolean;
  statusUrl: string;
  eventsUrl: string;
};
export type NotebookSaveHandlers = {
  onEvent(job: NotebookSaveJob): void;
  onConnectionError?(message: string): void;
};
export interface NotebookApi {
  createNotebook(input: CreateNotebook): Promise<Notebook>;
  listNotebooks(options?: NotebookListOptions): Promise<NotebookPage>;
  getNotebook(notebookId: string): Promise<Notebook>;
  updateNotebook(notebookId: string, input: PatchNotebook): Promise<Notebook>;
  deleteNotebook(notebookId: string): Promise<void>;
  createNotebookFolder(
    notebookId: string,
    input: { name: string },
  ): Promise<NotebookFolder>;
  listNotebookFolders(
    notebookId: string,
    options?: Pagination,
  ): Promise<FolderPage>;
  getNotebookFolder(
    notebookId: string,
    folderId: string,
  ): Promise<NotebookFolder>;
  updateNotebookFolder(
    notebookId: string,
    folderId: string,
    input: { name: string },
  ): Promise<NotebookFolder>;
  deleteNotebookFolder(notebookId: string, folderId: string): Promise<void>;
  createNotebookNote(
    notebookId: string,
    input: CreateNote,
  ): Promise<NotebookSaveJob>;
  listNotebookNotes(
    notebookId: string,
    options?: NoteListOptions,
  ): Promise<NotePage>;
  getNotebookNote(notebookId: string, noteId: string): Promise<PersistedNote>;
  updateNotebookNote(
    notebookId: string,
    noteId: string,
    input: PatchNote,
  ): Promise<NotebookSaveJob>;
  deleteNotebookNote(notebookId: string, noteId: string): Promise<void>;
  getNotebookSaveJob(
    notebookId: string,
    jobId: string,
  ): Promise<NotebookSaveJob>;
  retryNotebookSaveJob(
    notebookId: string,
    jobId: string,
  ): Promise<NotebookSaveJob>;
  subscribeToNotebookSaveJob(
    notebookId: string,
    jobId: string,
    handlers: NotebookSaveHandlers,
  ): () => void;
}
