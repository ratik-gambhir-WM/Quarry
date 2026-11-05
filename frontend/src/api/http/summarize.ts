import type { QuarryApi } from "../../contracts/quarryApi";

import { postForm, appendFiles } from "./transport";

function summarizeUpload(files: File[]) {
  const form = new FormData();
  appendFiles(form, files);
  return postForm<{ summary: string }>("/api/v1/summarize/upload", form).then(
    (response) => response.summary,
  );
}

export const httpSummarizeApi: Pick<QuarryApi, "summarizeUpload"> = {
  summarizeUpload,
};
