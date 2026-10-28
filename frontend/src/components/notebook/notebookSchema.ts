import { BlockNoteSchema, defaultBlockSpecs } from "@blocknote/core";

// Asset blocks stay unavailable until Notebook has a validated upload lifecycle.
export const notebookSchema = BlockNoteSchema.create({
  blockSpecs: {
    paragraph: defaultBlockSpecs.paragraph,
    heading: defaultBlockSpecs.heading,
    bulletListItem: defaultBlockSpecs.bulletListItem,
    numberedListItem: defaultBlockSpecs.numberedListItem,
    checkListItem: defaultBlockSpecs.checkListItem,
    quote: defaultBlockSpecs.quote,
    codeBlock: defaultBlockSpecs.codeBlock,
    table: defaultBlockSpecs.table,
  },
});

export type NotebookBlock = typeof notebookSchema.Block;
