import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { Window } from "happy-dom";

// Execute the emitted modules, rather than Vite's development transforms: chunk
// cycles can pass source tests and compilation while breaking editor initialization.
const target = process.argv[process.argv.indexOf("--target") + 1];
assert.ok(["web", "desktop"].includes(target), "Pass --target web or --target desktop.");
const dist = path.resolve("dist");
const manifest = JSON.parse(await readFile(path.join(dist, ".vite/manifest.json"), "utf8"));
const stamp = JSON.parse(await readFile(path.join(dist, ".vite/bundle-target.json"), "utf8"));
assert.equal(stamp.target, target, "Run the matching Vite build first.");
const entry = Object.values(manifest).find((record) => record.isEntry);
const editor = manifest["src/components/notebook/NotebookEditor.tsx"];
assert.ok(entry && editor, "The bundle must contain the app and deferred notebook editor.");

const window = new Window({
  url: "http://localhost/",
  settings: { disableCSSFileLoading: true, disableJavaScriptFileLoading: true },
});
window.document.body.innerHTML = '<div id="root"></div>';
for (const key of [
  "window", "document", "navigator", "Node", "HTMLElement", "Element", "DOMParser",
  "MutationObserver", "getComputedStyle", "requestAnimationFrame", "cancelAnimationFrame", "ResizeObserver",
]) {
  const value = ["getComputedStyle", "requestAnimationFrame", "cancelAnimationFrame"].includes(key)
    ? window[key].bind(window) : key === "window" ? window : window[key];
  Object.defineProperty(globalThis, key, { value, configurable: true });
}

try {
  // Match the application path: startup executes before the editor is requested.
  await import(pathToFileURL(path.join(dist, entry.file)).href);
  const notebook = await import(pathToFileURL(path.join(dist, editor.file)).href);
  assert.equal(typeof notebook.NotebookEditor, "function");
  console.log(`Pass: ${target} production notebook editor initializes.`);
} catch (error) {
  console.error(error);
  process.exitCode = 1;
} finally {
  await window.happyDOM.abort();
  window.close();
}
// The app's React scheduler remains alive; this check only owns module evaluation.
process.exit(process.exitCode ?? 0);
