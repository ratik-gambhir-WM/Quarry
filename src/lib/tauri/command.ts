import { invoke, type InvokeArgs } from "@tauri-apps/api/core";
import type { TauriCommandName } from "../constants";
import { beginIpcRequest, finishIpcRequest } from "../activityLog";

export class TauriCommandError extends Error {
  command: TauriCommandName;
  originalError: unknown;

  constructor(command: TauriCommandName, originalError: unknown) {
    super(`Tauri command "${command}" failed: ${getTauriErrorMessage(originalError)}`);
    this.name = "TauriCommandError";
    this.command = command;
    this.originalError = originalError;
    Object.setPrototypeOf(this, TauriCommandError.prototype);
  }
}

export type DesktopError = {
  code: "conflict" | "internal" | "not_found" | "permission" | "service_unavailable" | "validation";
  message: string;
  operationId?: string;
  retryable?: boolean;
};

export async function execute<TResponse = unknown, TArgs extends InvokeArgs = InvokeArgs>(
  command: TauriCommandName,
  args?: TArgs,
): Promise<TResponse> {
  const activityId = beginIpcRequest(command, args);
  const startedAt = performance.now();
  try {
    const response = await invoke<TResponse>(command, args);
    finishIpcRequest(activityId, {
      details: response,
      durationMs: performance.now() - startedAt,
      status: "success",
    });
    return response;
  } catch (error) {
    finishIpcRequest(activityId, {
      details: error,
      durationMs: performance.now() - startedAt,
      message: getTauriErrorMessage(error),
      status: "error",
    });
    throw new TauriCommandError(command, error);
  }
}

function getTauriErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "string") {
    return error;
  }

  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message: unknown }).message;
    if (typeof message === "string") {
      return message;
    }
  }

  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}
