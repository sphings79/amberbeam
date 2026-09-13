/**
 * Release notes, taken apart.
 *
 * The text arrives from the network and is treated as nothing but text: this
 * returns pieces the window draws itself, never markup. Whatever a release
 * says, the worst it can produce here is a plain line — there is no shape it
 * can ask for that this does not already know how to draw.
 *
 * Plain TypeScript with no runes, so `scripts/check-notes.mjs` can run it and
 * check it against a real release rather than against a hopeful example.
 */

export interface Span {
  text: string;
  strong: boolean;
  /**
   * Where this piece of text leads, for the few that lead anywhere.
   *
   * Only ever `http://` or `https://`. The text comes off the network, and a
   * link is the one shape here that does something rather than sits there —
   * so the scheme is checked rather than trusted, and anything else stays the
   * characters somebody wrote.
   */
  href?: string;
}

export type Piece =
  | { kind: "heading"; text: string; level: number }
  | { kind: "bullet"; parts: Span[] }
  /** A second paragraph belonging to the point above it. */
  | { kind: "under"; parts: Span[] }
  | { kind: "line"; parts: Span[] }
  /** A line across, which is all a `---` on its own line ever means. */
  | { kind: "rule" };

/**
 * `**bold**` and `[a link](https://…)`. Everything else is left alone.
 *
 * Deliberately the only two inline shapes understood. Every further shape is
 * one more thing to get wrong on text somebody else wrote — and these two are
 * here because our own notes use them and were showing their punctuation.
 *
 * A link whose address is not http or https is not a link. It stays the
 * characters it was written as, which is the dullest possible answer to
 * `javascript:` and to everything like it.
 */
const SHAPES = /(\*\*[^*]+\*\*)|\[([^\]\n]+)\]\((https?:\/\/[^\s)]+)\)/g;

export function inline(text: string): Span[] {
  const out: Span[] = [];
  let at = 0;

  for (const found of text.matchAll(SHAPES)) {
    const start = found.index ?? 0;
    if (start > at) {
      out.push({ text: text.slice(at, start), strong: false });
    }
    if (found[1] !== undefined) {
      out.push({ text: found[1].slice(2, -2), strong: true });
    } else {
      out.push({ text: found[2] ?? "", strong: false, href: found[3] });
    }
    at = start + found[0].length;
  }

  if (at < text.length) {
    out.push({ text: text.slice(at), strong: false });
  }
  return out.filter((span) => span.text !== "");
}

/**
 * One piece before its text has been looked at for bold.
 *
 * The two steps are separate on purpose. These notes wrap at eighty columns,
 * so a bullet arrives as several lines and its `**bold**` regularly opens on
 * one and closes on the next — taken apart line by line, neither half ever
 * matched and both kept their asterisks on screen. The lines are joined first
 * and read afterwards.
 */
type Draft =
  | { kind: "heading"; text: string; level: number }
  | { kind: "rule" }
  | { kind: "bullet" | "under" | "line"; text: string };

export function pieces(notes: string): Piece[] {
  const drafts: Draft[] = [];
  let blankBefore = false;

  for (const raw of notes.split("\n")) {
    const line = raw.trimEnd();
    if (line.trim() === "") {
      blankBefore = true;
      continue;
    }
    const broken = blankBefore;
    blankBefore = false;

    const heading = /^(#{1,6})\s+(.*)$/.exec(line);
    if (heading) {
      // The level is kept because a set of notes covering several releases
      // has two kinds of heading in it: the version, and the sections inside
      // it. Drawn the same size they read as one long list.
      drafts.push({
        kind: "heading",
        text: heading[2] ?? "",
        level: (heading[1] ?? "#").length,
      });
      continue;
    }

    // Before the bullet, which wants a space after its dash and so cannot
    // match this anyway — but the order is the part worth being sure of.
    if (/^\s*(-{3,}|\*{3,}|_{3,})\s*$/.test(line)) {
      drafts.push({ kind: "rule" });
      continue;
    }

    const bullet = /^\s*[-*]\s+(.*)$/.exec(line);
    if (bullet) {
      drafts.push({ kind: "bullet", text: bullet[1] ?? "" });
      continue;
    }

    // An indented line belongs to the bullet above it. Joining them back up is
    // the difference between a paragraph and a stack of fragments.
    //
    // Unless a blank line came between them: then the writer meant a second
    // paragraph under the same point, and running the two together makes one
    // sentence out of two thoughts.
    const last = drafts[drafts.length - 1];
    const indented = /^\s+\S/.test(raw);
    if (last && last.kind !== "heading" && last.kind !== "rule" && indented && !broken) {
      last.text += " " + line.trim();
      continue;
    }

    drafts.push({ kind: indented ? "under" : "line", text: line.trim() });
  }

  return drafts.map((draft) =>
    draft.kind === "heading" || draft.kind === "rule"
      ? draft
      : { kind: draft.kind, parts: inline(draft.text) },
  );
}
