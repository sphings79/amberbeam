#!/usr/bin/env node
/**
 * Guards seam 2 (concept paper, section 09): the user interface never reaches
 * for Tauri directly.
 *
 * Only files below `src/lib/bridge/` may import `@tauri-apps/…`, and only
 * `src/lib/bridge/index.ts` may import the build time alias `@bridge-impl`.
 * Everything else goes through `src/lib/bridge`. Without this check the rule
 * would hold for exactly as long as everyone remembers it.
 */

import { readFileSync, readdirSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, relative, sep } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");
const source = join(root, "src");

const BRIDGE_DIR = join("src", "lib", "bridge");
const BRIDGE_ENTRY = join(BRIDGE_DIR, "index.ts");

/**
 * @param {string} directory
 * @returns {Generator<string>}
 */
function* walk(directory) {
  for (const entry of readdirSync(directory)) {
    const path = join(directory, entry);
    if (statSync(path).isDirectory()) {
      yield* walk(path);
    } else if (/\.(ts|svelte)$/.test(entry)) {
      yield path;
    }
  }
}

let failed = false;

for (const path of walk(source)) {
  const shown = relative(root, path);
  const text = readFileSync(path, "utf8");
  const insideBridge = shown.startsWith(BRIDGE_DIR + sep);

  if (!insideBridge && /["']@tauri-apps\//.test(text)) {
    console.error(`${shown}: imports @tauri-apps directly. Go through src/lib/bridge instead.`);
    failed = true;
  }
  if (shown !== BRIDGE_ENTRY && /["']@bridge-impl["']/.test(text)) {
    console.error(`${shown}: imports @bridge-impl. Only ${BRIDGE_ENTRY} may do that.`);
    failed = true;
  }
}

if (failed) {
  process.exit(1);
}

console.log("seams in order: no direct access to Tauri outside the bridge");
