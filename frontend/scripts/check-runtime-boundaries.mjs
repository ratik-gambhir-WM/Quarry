import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const root = new URL("..", import.meta.url).pathname;
const sourceRoot = join(root, "src");
const allowedTauriFile = "src/platform/runtime.desktop.ts";
const violations = [];

for (const file of walk(sourceRoot)) {
  if (!/\.[cm]?[jt]sx?$/.test(file) || /\.(test|spec)\.[cm]?[jt]sx?$/.test(file)) {
    continue;
  }
  const projectPath = relative(root, file);
  const source = readFileSync(file, "utf8");
  if (source.includes("@tauri-apps/") && projectPath !== allowedTauriFile) {
    violations.push(`${projectPath}: Tauri imports belong only in ${allowedTauriFile}`);
  }
  if ((source.includes("fetch(") || source.includes("new EventSource(")) && !projectPath.startsWith("src/api/")) {
    violations.push(`${projectPath}: raw HTTP transports belong under src/api`);
  }
}

if (process.argv.includes("--web-bundle")) {
  for (const file of walk(join(root, "dist"))) {
    if (file.endsWith(".js") && readFileSync(file, "utf8").includes("__TAURI_INTERNALS__")) {
      violations.push(`${relative(root, file)}: web bundle contains the Tauri runtime`);
    }
  }
}

if (violations.length > 0) {
  console.error(violations.join("\n"));
  process.exitCode = 1;
}

function* walk(directory) {
  for (const name of readdirSync(directory)) {
    const path = join(directory, name);
    if (statSync(path).isDirectory()) {
      yield* walk(path);
    } else {
      yield path;
    }
  }
}
