#!/usr/bin/env node
/**
 * Checks that a button with no words on it says what it does.
 *
 * An icon is a guess until somebody has learned it, and the only way to learn
 * it without clicking is to rest the pointer on it. A `title` is what makes
 * that happen, and forgetting one is invisible to every other check in this
 * project: the button renders, the types are fine, nothing is missing — it is
 * simply mute.
 *
 * The rule is narrow on purpose. A button with a word on it needs nothing; a
 * button with only a drawing or only a symbol needs a title. The one exception
 * is the invisible sheet behind a menu, which is a button so that clicking
 * anywhere closes the menu — nobody hovers it, and a tooltip spanning the
 * window would be absurd.
 */

import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, relative } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");

/**
 * Every .svelte file under src, wherever it sits.
 *
 * @param {string} from
 * @returns {string[]}
 */
function components(from) {
  /** @type {string[]} */
  const found = [];
  for (const entry of readdirSync(from, { withFileTypes: true })) {
    const path = join(from, entry.name);
    if (entry.isDirectory()) found.push(...components(path));
    else if (entry.name.endsWith(".svelte")) found.push(path);
  }
  return found;
}

/**
 * The opening tags of every `<button`, with the attributes that belong to it.
 *
 * Written by hand rather than by a regular expression because the attributes
 * are full of `=>`, and a pattern that stops at the first `>` cuts an arrow
 * function in half — which is exactly how the first version of this check
 * reported nothing wrong.
 *
 * @param {string} source
 * @returns {{ attributes: string, body: string, line: number }[]}
 */
function buttons(source) {
  /** @type {{ attributes: string, body: string, line: number }[]} */
  const found = [];
  let at = 0;
  while ((at = source.indexOf("<button", at)) !== -1) {
    let depth = 0;
    let end = at;
    while (end < source.length) {
      const c = source[end];
      if (c === "{") depth += 1;
      else if (c === "}") depth -= 1;
      else if (c === ">" && depth === 0) break;
      end += 1;
    }
    const close = source.indexOf("</button>", end);
    if (close === -1) break;
    found.push({
      attributes: source.slice(at + "<button".length, end),
      body: source.slice(end + 1, close),
      line: source.slice(0, at).split("\n").length,
    });
    at = close;
  }
  return found;
}

/**
 * What a person actually reads on the button.
 *
 * @param {string} body
 * @returns {string}
 */
function words(body) {
  return body
    .replace(/<span class="sr">[\s\S]*?<\/span>/g, "") // for readers, not eyes
    .replace(/<[^>]*>/g, "")
    .replace(/[×✕★⋯…⇅▲▼·+\-\s]/g, "");
}

let failed = false;
let checked = 0;

for (const file of components(join(root, "src"))) {
  const source = readFileSync(file, "utf8");
  for (const button of buttons(source)) {
    checked += 1;
    if (words(button.body) !== "") continue;
    if (/\btitle=/.test(button.attributes)) continue;
    if (/class="scrim"/.test(button.attributes)) continue;
    console.error(
      `${relative(root, file)}:${button.line}: a button with no words and no title.`,
    );
    failed = true;
  }
}

if (failed) {
  process.exit(1);
}
console.log(`buttons in order: ${checked} checked, every wordless one says what it does`);
