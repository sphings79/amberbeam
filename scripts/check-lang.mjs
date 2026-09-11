#!/usr/bin/env node
/**
 * Reports missing and surplus keys per language file.
 *
 * English is the fallback and therefore the reference: every other catalogue is
 * measured against it. A missing key is an error, because it would make the
 * English text appear in a German window; a surplus key is an error too,
 * because it is dead weight nobody will ever see. Runs on every build.
 */

import { readFileSync, readdirSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const catalogueDir = join(here, "..", "src", "lib", "i18n");
const REFERENCE = "en";

/**
 * @param {string} name
 * @returns {Record<string, unknown>}
 */
function load(name) {
  return JSON.parse(readFileSync(join(catalogueDir, `${name}.json`), "utf8"));
}

const reference = load(REFERENCE);
const referenceKeys = new Set(Object.keys(reference));

/**
 * Every key the interface actually asks for.
 *
 * Comparing the catalogues against each other only proves they agree. It does
 * not prove either of them answers what the code asks — a key that exists in
 * neither comes out on screen as itself, which is how `action.delete` sat in a
 * button reading "action.delete" until somebody looked at it.
 *
 * The boundary in front of `t(` matters: without it `split("/")` counts as a
 * translation of "/".
 */
function keysUsedInSource() {
  const sourceDir = join(here, "..", "src");
  const used = new Map();

  /** @param {string} directory */
  function walk(directory) {
    for (const name of readdirSync(directory)) {
      const path = join(directory, name);
      if (statSync(path).isDirectory()) {
        walk(path);
        continue;
      }
      if (!name.endsWith(".svelte") && !name.endsWith(".ts")) continue;
      const text = readFileSync(path, "utf8");
      for (const match of text.matchAll(/(?<![A-Za-z0-9_$.])t\(\s*"([^"]+)"/g)) {
        used.set(match[1], path);
      }
    }
  }

  walk(sourceDir);
  return used;
}

const others = readdirSync(catalogueDir)
  .filter((file) => file.endsWith(".json"))
  .map((file) => file.slice(0, -".json".length))
  .filter((name) => name !== REFERENCE);

let failed = false;

for (const [key, where] of keysUsedInSource()) {
  if (!referenceKeys.has(key)) {
    console.error(`${where} asks for "${key}", which no catalogue has.`);
    failed = true;
  }
}

if (referenceKeys.size === 0) {
  console.error(`${REFERENCE}.json holds no keys at all.`);
  failed = true;
}

for (const [key, value] of Object.entries(reference)) {
  if (typeof value !== "string") {
    console.error(`${REFERENCE}.json: "${key}" is not a string. Catalogues are flat.`);
    failed = true;
  }
}

for (const name of others) {
  const catalogue = load(name);
  const keys = new Set(Object.keys(catalogue));

  for (const key of referenceKeys) {
    if (!keys.has(key)) {
      console.error(`${name}.json: missing key "${key}"`);
      failed = true;
    }
  }
  for (const key of keys) {
    if (!referenceKeys.has(key)) {
      console.error(`${name}.json: surplus key "${key}" — not in ${REFERENCE}.json`);
      failed = true;
    }
  }

  // Placeholders have to survive translation, or the text loses its value.
  for (const [key, text] of Object.entries(catalogue)) {
    if (typeof text !== "string") {
      console.error(`${name}.json: "${key}" is not a string.`);
      failed = true;
      continue;
    }
    const expected = [...String(reference[key] ?? "").matchAll(/\{(\w+)\}/g)].map((m) => m[1]);
    const actual = [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]);
    for (const placeholder of expected) {
      if (!actual.includes(placeholder)) {
        console.error(`${name}.json: "${key}" lost the placeholder {${placeholder}}`);
        failed = true;
      }
    }
  }
}

if (failed) {
  process.exit(1);
}

console.log(
  `language files in order: ${referenceKeys.size} keys in ${[REFERENCE, ...others].join(", ")}`,
);
