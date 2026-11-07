import type { ChatFileReference, QuarryApi } from "../../contracts/quarryApi";

export type MentionCandidate = ChatFileReference & { dealName: string };

/** Reuse the authorized catalog with bounded parallel requests; never substitute fixtures. */
export async function loadMentionFiles(
  api: Pick<QuarryApi, "listDeals" | "listDealDocuments">,
  dealId: string | null,
  signal?: AbortSignal,
): Promise<MentionCandidate[]> {
  const deals = dealId
    ? [{ dealId, dealName: dealId }]
    : await api.listDeals();
  const files: MentionCandidate[] = [];
  let next = 0;
  let failed = false;
  await Promise.all(Array.from({ length: Math.min(4, deals.length) }, async () => {
    while (!failed && !signal?.aborted && next < deals.length) {
      const deal = deals[next++];
      const documents = await api.listDealDocuments(deal.dealId).catch((error: unknown) => {
        failed = true;
        throw error;
      });
      files.push(...documents.map((document) => ({
        dealId: deal.dealId,
        dealName: deal.dealName,
        fileId: document.fileId,
        name: document.displayName,
      })));
    }
  }));
  return files.sort((a, b) => a.name.localeCompare(b.name) || a.dealId.localeCompare(b.dealId));
}

export function mentionAt(value: string, caret: number) {
  const before = value.slice(0, caret);
  const match = /(?:^|\s)@([^@\n]*)$/.exec(before);
  return match ? { start: caret - match[1].length - 1, end: caret, query: match[1] } : null;
}
