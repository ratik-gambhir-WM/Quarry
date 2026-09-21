export const queryChatSuggestions = [
  {
    title: "Summarize deal risks",
    label: "from the available context",
    prompt: "Summarize the most important deal risks and open questions.",
  },
  {
    title: "Draft diligence questions",
    label: "for the management team",
    prompt: "Draft a concise set of management diligence questions for this deal.",
  },
  {
    title: "Build an executive brief",
    label: "with decisions and next steps",
    prompt: "Create an executive brief with key findings, decisions, and next steps.",
  },
  {
    title: "Challenge the thesis",
    label: "with counterarguments",
    prompt: "Challenge the investment thesis and identify the strongest counterarguments.",
  },
] satisfies Array<{ label: string; prompt: string; title: string }>;
