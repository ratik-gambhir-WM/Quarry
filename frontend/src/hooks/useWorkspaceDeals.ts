import { useCallback, useEffect, useState } from "react";
import { runtime } from "@quarry/runtime";
import { buildWorkspaceDealFromPersisted } from "../data/dealExtraction";
import type { WorkspaceDeal } from "../data/workspace";

export type WorkspaceDataSource = "api" | "demo";

export type WorkspaceDealsResource =
  | { status: "loading" }
  | { deals: WorkspaceDeal[]; source: "server"; status: "success" }
  | { deals: WorkspaceDeal[]; source: "demo"; status: "success" }
  | { message: string; status: "error" };

export function getWorkspaceDataSource(
  value: string | undefined = import.meta.env.VITE_WORKSPACE_DATA_SOURCE,
): WorkspaceDataSource {
  if (value === undefined || value === "" || value === "api") {
    return "api";
  }
  if (value === "demo") {
    return "demo";
  }
  throw new Error(`Invalid VITE_WORKSPACE_DATA_SOURCE value: ${value}. Expected "api" or "demo".`);
}

export function useWorkspaceDeals(
  dataSourceValue: string | undefined = import.meta.env.VITE_WORKSPACE_DATA_SOURCE,
) {
  const [attempt, setAttempt] = useState(0);
  const [resource, setResource] = useState<WorkspaceDealsResource>({ status: "loading" });
  const retry = useCallback(() => setAttempt((current) => current + 1), []);

  useEffect(() => {
    let active = true;
    setResource({ status: "loading" });

    let dataSource: WorkspaceDataSource;
    try {
      dataSource = getWorkspaceDataSource(dataSourceValue);
    } catch (error) {
      setResource({
        message: error instanceof Error ? error.message : String(error),
        status: "error",
      });
      return () => {
        active = false;
      };
    }

    const request = dataSource === "demo"
      ? import("../fixtures/workspace/portfolio").then(({ workspaceDeals }) => ({
          deals: workspaceDeals,
          source: "demo" as const,
          status: "success" as const,
        }))
      : runtime.api.listDeals().then((persisted) => ({
          deals: persisted.map((deal) => buildWorkspaceDealFromPersisted(deal, deal.metadata)),
          source: "server" as const,
          status: "success" as const,
        }));

    void request.then((nextResource) => {
      if (active) setResource(nextResource);
    }).catch((error: unknown) => {
      if (active) {
        setResource({
          message: error instanceof Error ? error.message : String(error),
          status: "error",
        });
      }
    });

    return () => {
      active = false;
    };
  }, [attempt, dataSourceValue]);

  return { resource, retry };
}
