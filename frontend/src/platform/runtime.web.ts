import { httpQuarryApi } from "../api/httpQuarryApi";
import {
  POWERPOINT_CONTENT_TYPE,
  type QuarryRuntime,
  type SaveFileInput,
  type SavePowerPointInput,
} from "../contracts/quarryApi";
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

function savePowerPoint({ dataBase64, suggestedName }: SavePowerPointInput) {
  const binary = atob(dataBase64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  const blob = new Blob([bytes], { type: POWERPOINT_CONTENT_TYPE });
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
  platform: { readDealSourceFiles, saveFile, savePowerPoint, selectDealDataRoom },
  target: "web",
};
