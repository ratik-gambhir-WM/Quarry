import { afterEach, describe, expect, it, vi } from "vitest";
import { uploadDocument } from "@/api/http/upload";
import { parseUploadProgress } from "@/api/uploadProgress";

afterEach(() => vi.unstubAllGlobals());

function streamResponse(frames: string, split = 1) {
  const bytes = new TextEncoder().encode(frames);
  return new Response(new ReadableStream<Uint8Array>({
    start(controller) {
      for (let offset = 0; offset < bytes.length; offset += split) controller.enqueue(bytes.slice(offset, offset + split));
      controller.close();
    },
  }), { headers: { "content-type": "text/event-stream" } });
}

function frame(status: string) {
  return `event: ${status}\r\ndata: ${JSON.stringify({ jobId: "job", filename: "café.pdf", status })}\r\n\r\n`;
}

describe("upload admission stream", () => {
  it("streams fragmented UTF-8 progress and resolves at acceptance without embedding events", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(streamResponse(': keep-alive\r\n\r\n' + frame("parsing") + frame("classifying") + frame("accepted"))));
    const progress = vi.fn();
    await expect(uploadDocument("DEAL", new File(["bytes"], "café.pdf"), progress)).resolves.toEqual({ jobId: "job", filename: "café.pdf" });
    expect(progress.mock.calls.map(([event]) => event.status)).toEqual(["parsing", "classifying", "accepted"]);
  });

  it.each([frame("parsing"), frame("failed"), frame("embedding"), 'data: {}\n\n'])("rejects interrupted, failed, or invalid streams", async frames => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(streamResponse(frames, 8)));
    await expect(uploadDocument("DEAL", new File(["bytes"], "a.pdf"))).rejects.toThrow();
  });

  it("rejects incomplete event identities", () => {
    expect(() => parseUploadProgress({ jobId: "", filename: "file.pdf", status: "accepted" })).toThrow();
  });
});
