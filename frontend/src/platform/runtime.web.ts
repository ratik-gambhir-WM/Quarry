import { httpQuarryApi } from "../api/httpQuarryApi";
import type { QuarryRuntime, SaveFileInput } from "../contracts/quarryApi";

function saveFile({ contents, mimeType, suggestedName }: SaveFileInput) {
  const blob = new Blob([contents], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.download = suggestedName;
  anchor.href = url;
  anchor.click();
  URL.revokeObjectURL(url);
  return Promise.resolve(true);
}

export const runtime: QuarryRuntime = {
  api: httpQuarryApi,
  platform: { saveFile },
  target: "web",
};
