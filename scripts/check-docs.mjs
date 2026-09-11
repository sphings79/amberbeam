#!/usr/bin/env node
/**
 * Checks that every link between the written pages points at something.
 *
 * Markdown does not complain about a link to a file that is not there; it
 * renders it, and it looks exactly like a working one until somebody clicks.
 * Two languages of four pages each, cross-linked both ways, is more than enough
 * for one rename to go unnoticed.
 *
 * Only relative links. Whether a page on the web still exists is not something
 * a build can answer, and pretending otherwise would make this fail for
 * reasons nobody here can fix.
 */

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");

const pages = [
  join(root, "README.md"),
  join(root, "README.de.md"),
  ...readdirSync(join(root, "docs"))
    .filter((name) => name.endsWith(".md"))
    .map((name) => join(root, "docs", name)),
];

let failed = false;
let checked = 0;

for (const page of pages) {
  const text = readFileSync(page, "utf8");
  for (const match of text.matchAll(/\]\(([^)]+)\)/g)) {
    const target = match[1];
    if (!target || /^(https?:|mailto:|#)/.test(target)) continue;
    checked += 1;
    // A link may carry an anchor; the file is what matters here.
    const [file] = target.split("#");
    if (!file) continue;
    if (!existsSync(resolve(dirname(page), file))) {
      console.error(`${page.slice(root.length + 1)} links to ${target}, which is not there.`);
      failed = true;
    }
  }
}

if (failed) process.exit(1);
console.log(`documentation in order: ${pages.length} pages, ${checked} links`);
