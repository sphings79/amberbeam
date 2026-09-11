/**
 * The keyboard, as a table rather than as a switch.
 *
 * Section 04 of the concept paper asks for two things that only work together:
 * a default layout people already know, and every key freely changeable. A
 * `switch` in the window can do the first and never the second, so the layout
 * lives here as data — three schemes to choose between, and anything the user
 * moves on top of one.
 *
 * There are no runes in this file on purpose. It is plain TypeScript so the
 * guard in `scripts/check-keys.mjs` can run it directly and check what no type
 * can: that no two actions in a scheme answer to the same key.
 */

/** Everything a key can be pointed at. */
export type Action =
  | "help"
  | "rename"
  | "search"
  | "raw"
  | "refresh"
  | "switch-focus"
  | "queue-toggle"
  | "queue-start"
  | "settings"
  | "fullscreen"
  | "connect-toggle"
  | "sites"
  | "delete"
  | "transfer"
  | "new-folder"
  | "new-file"
  | "permissions"
  | "toggle-hidden"
  | "toggle-tree";

/** In the order the settings window lists them. */
export const ACTIONS: Action[] = [
  "help",
  "switch-focus",
  "refresh",
  "rename",
  "delete",
  "new-file",
  "new-folder",
  "permissions",
  "transfer",
  "search",
  "raw",
  "queue-toggle",
  "queue-start",
  "connect-toggle",
  "sites",
  "settings",
  "toggle-hidden",
  "toggle-tree",
  "fullscreen",
];

export type SchemeName = "classic" | "mac" | "mixed";

export const SCHEMES: SchemeName[] = ["classic", "mac", "mixed"];

/**
 * What each scheme answers to. An action may have more than one key — Tab and
 * F6 have both switched sides for thirty years, and taking one away to tidy the
 * table would be tidying away the point.
 *
 * An action with no key is not an oversight. The ones AmberBeam added have no
 * tradition to follow, and inventing a default for them would put a key in
 * somebody's way that they never asked for.
 */
export type Bindings = Partial<Record<Action, string[]>>;

/**
 * The layout of the older Windows clients, unchanged.
 *
 * Needs both system settings: the function keys turned on, and Mission
 * Control, Spotlight and "show desktop" turned off for F3, F4 and F11.
 */
const CLASSIC: Bindings = {
  help: ["F1"],
  rename: ["F2"],
  search: ["F3"],
  raw: ["F4"],
  refresh: ["F5"],
  "switch-focus": ["F6", "Tab"],
  "queue-toggle": ["F8"],
  "queue-start": ["F9"],
  settings: ["F10", "Meta+,"],
  fullscreen: ["F11"],
  "connect-toggle": ["F12"],
  sites: ["Meta+S"],
  delete: ["Delete"],
};

/**
 * No function key anywhere.
 *
 * For somebody who would rather not touch their system settings at all. It
 * costs the habit and works the minute it is chosen.
 */
const MAC: Bindings = {
  help: ["Meta+Shift+H"],
  rename: ["Meta+Shift+R"],
  search: ["Meta+F"],
  raw: ["Meta+Shift+K"],
  refresh: ["Meta+R"],
  "switch-focus": ["Meta+ArrowRight", "Tab"],
  "queue-toggle": ["Meta+Shift+U"],
  "queue-start": ["Meta+Enter"],
  settings: ["Meta+,"],
  fullscreen: ["Meta+Control+F"],
  "connect-toggle": ["Meta+Shift+D"],
  sites: ["Meta+S"],
  delete: ["Meta+Backspace"],
};

/**
 * The compromise, and what somebody gets who closed the dialog without
 * choosing.
 *
 * Every function key the system does not want for itself, and the three it does
 * — F3, F4, F11 — on combinations instead. Nothing has to be switched off in
 * System Settings for this to work.
 */
const MIXED: Bindings = {
  ...CLASSIC,
  search: ["Meta+F"],
  raw: ["Meta+Shift+K"],
  fullscreen: ["Meta+Control+F"],
};

const TABLE: Record<SchemeName, Bindings> = {
  classic: CLASSIC,
  mac: MAC,
  mixed: MIXED,
};

/** The scheme somebody gets who has not chosen one. */
export const FALLBACK: SchemeName = "mixed";

export function schemeBindings(name: SchemeName): Bindings {
  return TABLE[name] ?? TABLE[FALLBACK];
}

/**
 * A scheme with the user's own changes on top.
 *
 * An action set to an empty list is unbound on purpose, which is different from
 * an action the scheme never mentioned — so the override wins either way.
 */
export function resolve(name: SchemeName, own: Bindings): Bindings {
  return { ...schemeBindings(name), ...own };
}

/**
 * How a key press is written down.
 *
 * Modifiers first, always in the same order, so two spellings of one
 * combination cannot both exist. The key itself is whatever the browser calls
 * it, upper-cased when it is a single character — otherwise `Meta+s` and
 * `Meta+S` would be two different bindings for one key.
 */
export function bindingOf(event: {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}): string {
  const parts: string[] = [];
  if (event.metaKey) parts.push("Meta");
  if (event.ctrlKey) parts.push("Control");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");

  let key = event.key;
  if (key === " ") key = "Space";
  if (key.length === 1) key = key.toUpperCase();
  parts.push(key);
  return parts.join("+");
}

/** Which action a key press means, if any. */
export function actionFor(bindings: Bindings, binding: string): Action | null {
  for (const action of ACTIONS) {
    if (bindings[action]?.includes(binding)) return action;
  }
  return null;
}

/** How a binding is shown: the symbols a Mac keyboard actually has on it. */
export function label(binding: string): string {
  const symbols: Record<string, string> = {
    Meta: "⌘",
    Control: "⌃",
    Alt: "⌥",
    Shift: "⇧",
    Enter: "↵",
    Backspace: "⌫",
    Delete: "⌦",
    ArrowRight: "→",
    ArrowLeft: "←",
    ArrowUp: "↑",
    ArrowDown: "↓",
    Space: "␣",
    Escape: "⎋",
    Tab: "⇥",
  };
  return binding
    .split("+")
    .map((part) => symbols[part] ?? part)
    .join("");
}

/**
 * Which system settings a scheme needs before it works.
 *
 * Two separate hurdles, and they fall in this order: without the first, no
 * function key reaches the program at all; the second only concerns the three
 * the system keeps for itself.
 */
export function needs(name: SchemeName): { functionKeys: boolean; shortcuts: boolean } {
  const bindings = schemeBindings(name);
  const all = Object.values(bindings).flat();
  return {
    functionKeys: all.some((binding) => /^F\d+$/.test(binding)),
    shortcuts: all.some((binding) => ["F3", "F4", "F11"].includes(binding)),
  };
}
