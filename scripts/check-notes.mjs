#!/usr/bin/env node
/**
 * Checks that release notes survive being taken apart.
 *
 * The text comes off the network, so the two things that matter are that
 * nothing in it can ask the window to draw something it did not intend, and
 * that nothing in it goes missing. Both are checked against this project's own
 * CHANGELOG, because that is what the release notes are cut from — an example
 * written to pass would only prove that examples can be written.
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");

/** @typedef {import("../src/lib/ui/notes.ts").Piece} Piece */
/** @typedef {import("../src/lib/ui/notes.ts").Span} Span */

/** @type {{ pieces: (notes: string) => Piece[], inline: (text: string) => Span[] }} */
const { pieces, inline } = await import(join(root, "src", "lib", "ui", "notes.ts"));

let failed = false;

/** @param {string} message */
function complain(message) {
  console.error(message);
  failed = true;
}

/**
 * The newest section of the changelog, which is what a release carries.
 *
 * @returns {string}
 */
function newestSection() {
  const text = readFileSync(join(root, "CHANGELOG.md"), "utf8");
  const from = text.indexOf("\n## ");
  const next = text.indexOf("\n## ", from + 1);
  return text.slice(from + 1, next === -1 ? undefined : next);
}

const notes = newestSection();
const parts = pieces(notes);

if (parts.length === 0) {
  complain("the newest changelog section came apart into nothing.");
}

// Nothing may carry markup through. A piece is text and a flag, and if any
// text still contains a tag then something is being passed along that the
// window would have to decide what to do with.
for (const piece of parts) {
  const texts = piece.kind === "heading" ? [piece.text] : piece.parts.map((p) => p.text);
  for (const text of texts) {
    if (/<[^>]+>/.test(text)) {
      complain(`a piece carries markup through: ${text.slice(0, 60)}`);
    }
  }
}

// Bold that wrapped is still bold. These notes wrap at eighty columns, so a
// `**lead-in**` regularly opens on one line and closes on the next; read line
// by line neither half ever matched and both kept their asterisks on screen.
// Any asterisk left in a piece means a marker went unrecognised.
for (const piece of parts) {
  const texts = piece.kind === "heading" ? [piece.text] : piece.parts.map((p) => p.text);
  for (const text of texts) {
    if (text.includes("**")) {
      complain(`a piece kept its asterisks: ${text.slice(0, 60)}`);
    }
  }
}

// And nothing may be lost. Every word in the notes has to still be somewhere,
// or a bullet that wrapped has quietly become half a sentence.
const said = parts
  .map((piece) => (piece.kind === "heading" ? piece.text : piece.parts.map((p) => p.text).join("")))
  .join(" ");
const words = (/** @type {string} */ text) =>
  text
    .replace(/[#*`_>|[\]()-]/g, " ")
    .split(/\s+/)
    .filter((word) => word.length > 2);

const missing = words(notes).filter((word) => !said.includes(word));
if (missing.length > 0) {
  complain(`words went missing from the notes: ${[...new Set(missing)].slice(0, 8).join(", ")}`);
}

// A heading is a heading, a bullet is a bullet. If our own changelog does not
// produce both, the reader is being shown a wall of lines.
if (!parts.some((piece) => piece.kind === "heading")) {
  complain("no heading was recognised in a changelog section that has them.");
}
if (!parts.some((piece) => piece.kind === "bullet")) {
  complain("no bullet was recognised in a changelog section that has them.");
}

// A second paragraph under a point stays a second paragraph. Running the two
// together makes one sentence out of two thoughts, and the changelog is
// written with those breaks on purpose.
if (!parts.some((piece) => piece.kind === "under")) {
  complain("a paragraph under a point was swallowed by the point above it.");
}

// Bold text, which is how every entry names the thing it is about.
const strong = parts.flatMap((piece) => (piece.kind === "heading" ? [] : piece.parts));
if (!strong.some((span) => span.strong)) {
  complain("nothing came out bold in a section written with bold leads.");
}
if (inline("plain text").some((span) => span.strong)) {
  complain("plain text came out bold.");
}

// Text nobody wrote for us: the parser has to be dull about it rather than
// clever. Whatever this turns into, it must be pieces of text and nothing more.
for (const hostile of [
  "<script>alert(1)</script>",
  "- <img src=x onerror=alert(1)>",
  "### <b>heading</b>",
  "**unclosed",
  "",
  "-",
  "#",
]) {
  for (const piece of pieces(hostile)) {
    const texts = piece.kind === "heading" ? [piece.text] : piece.parts.map((p) => p.text);
    if (texts.some((text) => typeof text !== "string")) {
      complain(`"${hostile}" produced something that is not text.`);
    }
  }
}

if (failed) {
  process.exit(1);
}
console.log(`notes in order: ${parts.length} pieces, nothing lost and no markup through`);
