import type { KeywordFileChunkHit } from "@/contracts/quarryApi";

export function keywordHit(overrides: Partial<KeywordFileChunkHit> = {}): KeywordFileChunkHit {
  return {
    chunk_id: "chunk-1", deal_id: "DEAL-1", file_id: "file-1",
    version_id: "version-1", index_generation: "generation-1",
    chunk_index: 1, chunk_sha256: "0".repeat(64),
    text: "The liability review covers environmental risk.", token_count: 8,
    page_start: 2, page_end: 2, char_start: 0, char_end: 49,
    section_path: "Risk review", created_at: "2026-11-06T00:00:00Z", score: 1,
    ...overrides,
  };
}
