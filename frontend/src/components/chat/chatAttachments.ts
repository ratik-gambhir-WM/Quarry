// Match Axum chat/upload.rs and the Tauri query relay. Byte signatures remain server-validated.
export const CHAT_FILE_ACCEPT = ".pdf,.txt,.md,.json,.html,.csv,.doc,.docx,.pptx,.xlsx,.png,.jpg,.jpeg,.webp,.gif";
export const MAX_CHAT_FILES = 20;
export const MAX_CHAT_FILE_BYTES = 50 * 1024 * 1024;
const extensions = new Set(CHAT_FILE_ACCEPT.split(","));

export function addChatFiles(current: readonly File[], incoming: readonly File[]) {
  const files = [...current];
  const errors: string[] = [];
  let total = files.reduce((sum, file) => sum + file.size, 0);
  for (const file of incoming) {
    const extension = file.name.slice(file.name.lastIndexOf(".")).toLowerCase();
    if (
      !file.name.trim() ||
      file.name !== file.name.trim() ||
      new TextEncoder().encode(file.name).length > 255 ||
      /[/\\\u0000-\u001f\u007f-\u009f]/u.test(file.name)
    ) {
      errors.push("A file has an unsupported name.");
    } else if (!extensions.has(extension)) {
      errors.push(`${file.name}: this file type isn’t supported.`);
    } else if (file.size === 0) {
      errors.push(`${file.name}: empty files can’t be attached.`);
    } else if (file.size > MAX_CHAT_FILE_BYTES || total + file.size > MAX_CHAT_FILE_BYTES) {
      errors.push("Attachments must total 50 MiB or less.");
    } else if (files.length >= MAX_CHAT_FILES) {
      errors.push("Attach up to 20 files per message.");
    } else {
      files.push(file);
      total += file.size;
    }
  }
  return { files, error: [...new Set(errors)].join(" ") || null };
}

export function formatChatFileSize(size: number) {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${Math.ceil(size / 1024)} KiB`;
  return `${(size / (1024 * 1024)).toFixed(1)} MiB`;
}
