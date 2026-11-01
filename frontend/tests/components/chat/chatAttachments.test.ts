import { describe, expect, it } from "vitest";
import { addChatFiles, MAX_CHAT_FILE_BYTES } from "@/components/chat/chatAttachments";

function file(name: string, size = 3) {
  const value = new File(["abc"], name, { type: "text/plain" });
  Object.defineProperty(value, "size", { value: size });
  return value;
}

describe("chat attachments", () => {
  it("accepts supported files while explaining empty, unsupported, and unsafe names", () => {
    const valid = file("notes.txt");
    const result = addChatFiles([], [valid, file("script.exe"), file("empty.txt", 0), file("../notes.txt")]);
    expect(result.files).toEqual([valid]);
    expect(result.error).toContain("this file type isn’t supported");
    expect(result.error).toContain("empty files");
    expect(result.error).toContain("unsupported name");
  });

  it("enforces the total byte and file count bounds without discarding accepted files", () => {
    const full = file("large.txt", MAX_CHAT_FILE_BYTES);
    expect(addChatFiles([full], [file("extra.txt")])).toEqual({ files: [full], error: "Attachments must total 50 MiB or less." });
    const twenty = Array.from({ length: 20 }, (_, index) => file(`note-${index}.txt`));
    expect(addChatFiles(twenty, [file("extra.txt")])).toEqual({ files: twenty, error: "Attach up to 20 files per message." });
  });
});
