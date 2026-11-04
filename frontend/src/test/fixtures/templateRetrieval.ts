import type { TemplateRetrieval } from "@/contracts/quarryApi";

export function testTemplateRetrieval(): TemplateRetrieval {
  return {
    templateId: "template-one",
    retrievalStatus: "ready",
    classification: { status: "ready", attemptCount: 1, nextAttemptAt: null, lastErrorCode: null },
    embedding: { status: "ready", attemptCount: 1, nextAttemptAt: null, lastErrorCode: null },
    metadata: {
      subject: { summary: "Cloud architecture overview", domains: [{ id: "cloud", relevance: "primary", topics: ["migration"] }], otherTopics: [], technologies: ["Kubernetes"], entities: [], claims: [], synonyms: ["platform"] },
      communication: { intents: ["explanation"], informationTypes: ["architecture"], audience: ["engineers"] },
      templateFit: { archetype: "architecture diagram", contentSlots: [{ elementId: "title-1", role: "title", capacity: "short-text" }] },
      visual: { layoutType: "diagram-led", visualElements: ["diagram"], contentDensity: "low", structuralFeatures: ["arrows"] },
      retrievalKeywords: ["cloud", "architecture"],
    },
    embeddings: { status: "ready", model: "text-embedding-3-small", dimensions: 3, subjectVector: [0.12, -0.23, 0.34], capabilityVector: [-0.45, 0.56, 0.67] },
  };
}
