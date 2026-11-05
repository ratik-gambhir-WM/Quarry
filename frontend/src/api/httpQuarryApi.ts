import type { QuarryApi } from "../contracts/quarryApi";
import { createDealMembershipApi } from "./dealMembershipApi";
import { createNotebookApi } from "./notebookApi";
import { subscribeNotebookSse } from "./notebookSse";
import { API_BASE_URL, requestPrivateJson } from "./http/transport";
import { httpAuthApi } from "./http/auth";
import { httpCompaniesApi } from "./http/companies";
import { httpDealsApi } from "./http/deals";
import { httpDocumentsApi } from "./http/documents";
import { httpTemplatesApi } from "./http/templates";
import { httpAssistantApi } from "./http/assistant";
import { httpSummarizeApi } from "./http/summarize";

export { BackendApiError } from "./http/transport";

export const httpQuarryApi: QuarryApi = {
  ...httpAuthApi,
  ...httpCompaniesApi,
  ...httpDealsApi,
  ...httpDocumentsApi,
  ...httpTemplatesApi,
  ...httpAssistantApi,
  ...httpSummarizeApi,
  ...createDealMembershipApi(requestPrivateJson),
  ...createNotebookApi({
    request: requestPrivateJson,
    subscribe: (notebookId, jobId, handlers) =>
      subscribeNotebookSse(API_BASE_URL, notebookId, jobId, handlers),
  }),
};
