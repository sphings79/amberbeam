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

/** Which keyboard somebody is sitting at, as far as the shortcuts care. */
export type Platform = "mac" | "other";

/**
 * Worked out once, from the only thing a plain module can ask.
 *
 * Outside a browser — the guard script — this answers "other", and the guard
 * then checks both on purpose rather than trusting the answer.
 */
function detect(): Platform {
  const nav = (globalThis as { navigator?: { userAgentData?: { platform?: string }; platform?: string; userAgent?: string } }).navigator;
  const name = nav?.userAgentData?.platform ?? nav?.platform ?? nav?.userAgent ?? "";
  return /mac/i.test(name) ? "mac" : "other";
}

export const PLATFORM: Platform = detect();

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
  settings: ["F10", "Mod+,"],
  fullscreen: ["F11"],
  "connect-toggle": ["F12"],
  sites: ["Mod+S"],
  delete: ["Delete"],
};

/**
 * No function key anywhere.
 *
 * For somebody who would rather not touch their system settings at all. It
 * costs the habit and works the minute it is chosen.
 */
const MAC: Bindings = {
  help: ["Mod+Shift+H"],
  rename: ["Mod+Shift+R"],
  search: ["Mod+F"],
  raw: ["Mod+Shift+K"],
  refresh: ["Mod+R"],
  "switch-focus": ["Mod+ArrowRight", "Tab"],
  "queue-toggle": ["Mod+Shift+U"],
  "queue-start": ["Mod+Enter"],
  settings: ["Mod+,"],
  fullscreen: ["Mod+Control+F"],
  "connect-toggle": ["Mod+Shift+D"],
  sites: ["Mod+S"],
  delete: ["Mod+Backspace"],
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
  search: ["Mod+F"],
  raw: ["Mod+Shift+K"],
  fullscreen: ["Mod+Control+F"],
};

/**
 * What the tables look like away from a Mac.
 *
 * Only the two places where the Mac answer is unreachable rather than merely
 * unfamiliar: `Mod+Control+F` asks for the same key twice once Mod *is*
 * Control, and nothing on Windows has ever deleted a file with Backspace and a
 * modifier. Everything else carries over untouched — Ctrl+F for searching is,
 * if anything, more at home there than F3 ever was.
 */
const ELSEWHERE: Record<SchemeName, Bindings> = {
  classic: {},
  mixed: { fullscreen: ["F11"] },
  mac: { fullscreen: ["Mod+Shift+F"], delete: ["Delete"] },
};

const TABLE: Record<SchemeName, Bindings> = {
  classic: CLASSIC,
  mac: MAC,
  mixed: MIXED,
};

/** The scheme somebody gets who has not chosen one. */
export const FALLBACK: SchemeName = "mixed";

export function schemeBindings(name: SchemeName, platform: Platform = PLATFORM): Bindings {
  const base = TABLE[name] ?? TABLE[FALLBACK];
  if (platform === "mac") return base;
  return { ...base, ...(ELSEWHERE[name] ?? {}) };
}

/**
 * A scheme with the user's own changes on top.
 *
 * An action set to an empty list is unbound on purpose, which is different from
 * an action the scheme never mentioned — so the override wins either way.
 */
export function resolve(name: SchemeName, own: Bindings, platform: Platform = PLATFORM): Bindings {
  return { ...schemeBindings(name, platform), ...own };
}

/**
 * How a key press is written down.
 *
 * Modifiers first, always in the same order, so two spellings of one
 * combination cannot both exist. The key itself is whatever the browser calls
 * it, upper-cased when it is a single character — otherwise `Mod+s` and
 * `Mod+S` would be two different bindings for one key.
 *
 * The command modifier is written `Mod` and never `Meta`, because which
 * physical key that is depends on the machine: Command on a Mac, Control
 * everywhere else. Writing `Meta` meant Windows read it as the Windows key —
 * which opens the Start menu and reaches no program at all.
 */
export function bindingOf(
  event: {
    key: string;
    metaKey: boolean;
    ctrlKey: boolean;
    altKey: boolean;
    shiftKey: boolean;
  },
  platform: Platform = PLATFORM,
): string {
  const mac = platform === "mac";
  const parts: string[] = [];
  if (mac ? event.metaKey : event.ctrlKey) parts.push("Mod");
  if (mac ? event.ctrlKey : event.metaKey) parts.push(mac ? "Control" : "Meta");
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

/** The symbols a Mac keyboard actually has printed on it. */
const MAC_KEYS: Record<string, string> = {
  Mod: "⌘",
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

/**
 * The same keys as Windows and Linux name them.
 *
 * Not the Mac symbols with Ctrl swapped in. Outside macOS nobody reads ⇧⌫ as a
 * key, and a menu that shows it is showing off rather than explaining.
 */
const WORD_KEYS: Record<string, string> = {
  Mod: "Ctrl",
  Control: "Ctrl",
  Meta: "Win",
  Alt: "Alt",
  Shift: "Shift",
  Enter: "Enter",
  Backspace: "Backspace",
  Delete: "Del",
  ArrowRight: "Right",
  ArrowLeft: "Left",
  ArrowUp: "Up",
  ArrowDown: "Down",
  Space: "Space",
  Escape: "Esc",
  Tab: "Tab",
};

/** How a binding is shown, in the words of the keyboard in front of somebody. */
export function label(binding: string, platform: Platform = PLATFORM): string {
  const mac = platform === "mac";
  const keys = mac ? MAC_KEYS : WORD_KEYS;
  return binding
    .split("+")
    .map((part) => keys[part] ?? part)
    .join(mac ? "" : "+");
}

/**
 * Which system settings a scheme needs before it works.
 *
 * Two separate hurdles, and they fall in this order: without the first, no
 * function key reaches the program at all; the second only concerns the three
 * the system keeps for itself.
 */
export function needs(
  name: SchemeName,
  platform: Platform = PLATFORM,
): { functionKeys: boolean; shortcuts: boolean } {
  // Both hurdles are macOS's. Elsewhere F1 to F12 are function keys and no
  // part of the system is waiting behind them.
  if (platform !== "mac") return { functionKeys: false, shortcuts: false };
  const bindings = schemeBindings(name, platform);
  const all = Object.values(bindings).flat();
  return {
    functionKeys: all.some((binding) => /^F\d+$/.test(binding)),
    shortcuts: all.some((binding) => ["F3", "F4", "F11"].includes(binding)),
  };
}
