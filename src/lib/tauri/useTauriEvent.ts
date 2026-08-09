import { useEffect, useRef } from "react";
import { listen, type EventCallback } from "@tauri-apps/api/event";
import { logTauriEvent } from "../activityLog";

export function useTauriEvent<T>(eventName: string, handler: EventCallback<T>): void {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    let mounted = true;
    let removeListener: (() => void) | undefined;

    void listen<T>(eventName, (event) => {
      try {
        const payloadStatus =
          event.payload && typeof event.payload === "object" && "status" in event.payload
            ? String((event.payload as { status?: unknown }).status)
            : "";
        logTauriEvent(eventName, payloadStatus === "failed" ? "error" : "info", event.payload);
      } catch {
        // Event logging must never interrupt the subscribed workflow.
      }
      handlerRef.current(event);
    }).then((unlisten) => {
      if (mounted) {
        removeListener = unlisten;
      } else {
        unlisten();
      }
    }).catch((error: unknown) => {
      try {
        logTauriEvent(eventName, "error", error);
      } catch {
        // Event logging must never interrupt the subscribed workflow.
      }
    });

    return () => {
      mounted = false;
      removeListener?.();
    };
  }, [eventName]);
}
