import { readFile, stat } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

export const APPLICATION_CHUNK_LIMIT_BYTES = 500_000;
export const ENTRY_BUDGET_BYTES = {
  desktop: 350_000,
  web: 350_000,
};

export async function inspectBundle({ distDirectory, entryBudget, target }) {
  const manifestPath = path.join(distDirectory, ".vite", "manifest.json");
  const targetPath = path.join(distDirectory, ".vite", "bundle-target.json");
  const [manifest, targetStamp] = await Promise.all([
    readJson(manifestPath, "Vite manifest"),
    readJson(targetPath, "bundle target stamp"),
  ]);

  if (targetStamp.target !== target) {
    throw new Error(`Bundle target mismatch: expected ${target}, found ${String(targetStamp.target)}. Run the matching build first.`);
  }

  const records = Object.values(manifest);
  const entryRecords = records.filter((record) => record?.isEntry === true && isApplicationChunk(record.file));
  if (entryRecords.length !== 1) {
    throw new Error(`Expected exactly one JavaScript entry in the Vite manifest, found ${entryRecords.length}.`);
  }

  const chunkFiles = [...new Set(records
    .map((record) => record?.file)
    .filter((file) => typeof file === "string" && isApplicationChunk(file)))];
  if (chunkFiles.length === 0) {
    throw new Error("The Vite manifest contains no application JavaScript chunks.");
  }

  const chunks = await Promise.all(chunkFiles.map(async (file) => ({
    bytes: (await stat(path.join(distDirectory, file))).size,
    file,
  })));
  chunks.sort((left, right) => right.bytes - left.bytes);
  const entry = chunks.find((chunk) => chunk.file === entryRecords[0].file);
  if (!entry) {
    throw new Error(`Entry file ${entryRecords[0].file} is missing from the application chunk set.`);
  }

  const failures = [];
  if (entry.bytes > entryBudget) {
    failures.push(`entry ${entry.file} is ${entry.bytes} bytes (budget ${entryBudget})`);
  }
  for (const chunk of chunks) {
    if (chunk.bytes >= APPLICATION_CHUNK_LIMIT_BYTES) {
      failures.push(`application chunk ${chunk.file} is ${chunk.bytes} bytes (limit ${APPLICATION_CHUNK_LIMIT_BYTES - 1})`);
    }
  }

  return { chunks, entry, entryBudget, failures, target };
}

function isApplicationChunk(file) {
  return file.endsWith(".js");
}

async function readJson(filePath, label) {
  let source;
  try {
    source = await readFile(filePath, "utf8");
  } catch (error) {
    throw new Error(`Could not read ${label} at ${filePath}. Run the matching Vite build first.`, { cause: error });
  }

  try {
    return JSON.parse(source);
  } catch (error) {
    throw new Error(`Could not parse ${label} at ${filePath}.`, { cause: error });
  }
}

function parseTarget(args) {
  const targetIndex = args.indexOf("--target");
  const target = targetIndex >= 0 ? args[targetIndex + 1] : undefined;
  if (target !== "web" && target !== "desktop") {
    throw new Error('Pass exactly one supported target: "--target web" or "--target desktop".');
  }
  return target;
}

async function main() {
  const target = parseTarget(process.argv.slice(2));
  const result = await inspectBundle({
    distDirectory: path.resolve("dist"),
    entryBudget: ENTRY_BUDGET_BYTES[target],
    target,
  });
  const largest = result.chunks.slice(0, 8)
    .map((chunk) => `  ${chunk.file}: ${chunk.bytes} bytes`)
    .join("\n");

  console.log(`Bundle size check (${target})`);
  console.log(`Entry: ${result.entry.file} — ${result.entry.bytes} bytes / ${result.entryBudget} byte budget`);
  console.log(`Largest application chunks:\n${largest}`);

  if (result.failures.length > 0) {
    throw new Error(`Bundle size budget failed:\n- ${result.failures.join("\n- ")}`);
  }
  console.log(`Pass: ${result.chunks.length} application chunks are below ${APPLICATION_CHUNK_LIMIT_BYTES} bytes.`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
