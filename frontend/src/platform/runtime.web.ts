import { httpQuarryApi } from "../api/httpQuarryApi";
import type { QuarryRuntime, SaveFileInput } from "../contracts/quarryApi";
import type { ReadDealSourceFilesInput } from "../data/dealExtraction";

function readDealSourceFiles(_input: ReadDealSourceFilesInput): Promise<never> {
  return Promise.reject(new Error("Local data-room access is only available in the desktop app."));
}

function selectDealDataRoom() {
  return Promise.resolve(null);
}

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
  platform: { readDealSourceFiles, saveFile, selectDealDataRoom },
  target: "web",
};
