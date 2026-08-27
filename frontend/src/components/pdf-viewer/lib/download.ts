import type { PdfSource } from "../types";

function clickAnchor(href: string, filename: string): void {
  const a = document.createElement("a");
  a.href = href;
  a.download = filename;
  a.rel = "noopener";
  a.style.display = "none";
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
}

export async function downloadAsFile(
  source: PdfSource | null,
  filename: string,
): Promise<void> {
  if (source == null) return;
  const finalName = filename.toLowerCase().endsWith(".pdf")
    ? filename
    : `${filename}.pdf`;

  if (typeof source === "string") {
    clickAnchor(source, finalName);
    return;
  }

  let blob: Blob;
  if (source instanceof Blob) {
    blob = source;
  } else if (source instanceof ArrayBuffer) {
    blob = new Blob([source], { type: "application/pdf" });
  } else if (source instanceof Uint8Array) {
    blob = new Blob([new Uint8Array(source)], { type: "application/pdf" });
  } else {
    return;
  }

  const objectUrl = URL.createObjectURL(blob);
  try {
    clickAnchor(objectUrl, finalName);
  } finally {
    setTimeout(() => URL.revokeObjectURL(objectUrl), 5000);
  }
}
