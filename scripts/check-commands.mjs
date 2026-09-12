#!/usr/bin/env node
/**
 * Checks that every command a bridge asks for is a command something answers.
 *
 * The shared crate holds one list of the commands both shells offer, and the
 * desktop shell adds a handful of its own — windows, and paths a person chose
 * in a dialog. Between them that is the whole vocabulary. A bridge asking for
 * anything else is asking into the dark: it compiles, it type-checks, and it
 * fails at the moment somebody presses the button.
 *
 * It checks the other direction too. A command nothing calls is either dead or
 * a bridge that forgot it, and both are worth being told about rather than
 * finding out later.
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");

/**
 * The shared list, read out of the Rust source.
 *
 * Parsed rather than duplicated here. A copy of a list is a list that will
 * disagree, which is the whole reason the shared crate exists.
 *
 * @returns {string[]}
 */
function shared() {
  const source = readFileSync(join(root, "crates", "amberbeam-commands", "src", "lib.rs"), "utf8");
  const from = source.indexOf("pub const COMMANDS: &[&str] = &[");
  if (from === -1) throw new Error("amberbeam-commands no longer publishes a COMMANDS list");
  const to = source.indexOf("];", from);
  return [...source.slice(from, to).matchAll(/"([a-z_]+)"/g)].map((match) => match[1] ?? "");
}

/**
 * What the desktop shell answers to beyond the shared list.
 *
 * Taken from its `generate_handler!`, which is the actual register of what
 * that shell exposes — not from a note about it.
 *
 * @returns {string[]}
 */
function desktopOnly() {
  const source = readFileSync(join(root, "src-tauri", "src", "lib.rs"), "utf8");
  const from = source.indexOf("tauri::generate_handler![");
  if (from === -1) throw new Error("the desktop shell no longer registers commands");
  const to = source.indexOf("])", from);
  return source
    .slice(from, to)
    .split("\n")
    .slice(1)
    .map((line) => line.trim().replace(/,$/, ""))
    .filter((name) => /^[a-z_]+$/.test(name));
}

/**
 * Every command name a bridge asks for, and which bridge asked.
 *
 * @returns {Map<string, Set<string>>}
 */
function asked() {
  /** @type {Map<string, Set<string>>} */
  const found = new Map();
  /** @type {[string, RegExp][]} */
  const bridges = [
    ["src/lib/bridge/tauri.ts", /(?:send|invoke)(?:<[^>]*>)?\("([a-z_]+)"/g],
    ["src/lib/bridge/http.ts", /call(?:<[^>]*>)?\("([a-z_]+)"/g],
  ];
  for (const [file, pattern] of bridges) {
    const source = readFileSync(join(root, file), "utf8");
    for (const match of source.matchAll(pattern)) {
      const name = match[1] ?? "";
      if (!found.has(name)) found.set(name, new Set());
      found.get(name)?.add(file);
    }
  }
  return found;
}

let failed = false;

/** @param {string} message */
function complain(message) {
  console.error(message);
  failed = true;
}

const core = shared();
const shell = desktopOnly();
const known = new Set([...core, ...shell]);
const wanted = asked();

for (const [name, files] of wanted) {
  if (!known.has(name)) {
    complain(`${[...files].join(", ")}: asks for "${name}", which nothing answers to.`);
  }
}

// The one door the desktop bridge goes through is not itself a command anybody
// names; it carries the others.
const carrier = "run_command";
for (const name of core) {
  if (!wanted.has(name)) {
    complain(`"${name}" is offered by the shared crate and no bridge ever calls it.`);
  }
}
for (const name of shell) {
  if (name !== carrier && !wanted.has(name)) {
    complain(`"${name}" is registered by the desktop shell and no bridge ever calls it.`);
  }
}

// Both bridges have to reach the same core. One of them quietly missing a
// command is how the container build ends up being a lesser program.
for (const name of core) {
  const files = wanted.get(name);
  if (files && files.size !== 2) {
    complain(`"${name}" is called by ${[...files].join(", ")} alone; both bridges need it.`);
  }
}

if (failed) {
  process.exit(1);
}
console.log(
  `commands in order: ${core.length} shared, ${shell.length - 1} for the desktop alone, all reached`,
);
