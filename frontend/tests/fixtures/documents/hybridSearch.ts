import type { HybridDocumentResponse, KeywordFileChunkHit } from "@/contracts/quarryApi";
import { keywordHit } from "./keywordSearch";
export function hybridResponse(hits: KeywordFileChunkHit[] = [keywordHit()]): HybridDocumentResponse {
  return { results: hits.map((hit) => ({ chunkId: hit.chunk_id, dealId: hit.deal_id,
    fileId: hit.file_id, versionId: hit.version_id, text: hit.text, chunkIndex: hit.chunk_index,
    pageStart: hit.page_start, pageEnd: hit.page_end, sectionPath: hit.section_path,
    matchSources: ["keyword", "semantic"],
  })) };
}

export function mixedSourceResponse(): HybridDocumentResponse {
  const response = hybridResponse([
    keywordHit({ chunk_id: "literal", text: "Literal liability provision.", page_start: 1 }),
    keywordHit({ chunk_id: "related", text: "Related meaning about contractual responsibility.", page_start: 2 }),
    keywordHit({ chunk_id: "overlap", text: "Overlapping liability passage.", page_start: 3 }),
  ]);
  response.results[0].matchSources = ["keyword"];
  response.results[1].matchSources = ["semantic"];
  return response;
}
