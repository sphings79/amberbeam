/**
 * The two file panes.
 *
 * Seam 1 again, one level up: a pane holds an endpoint and a path, and neither
 * pane is "the local one". Which side happens to be local is a matter of what
 * it was pointed at, and the transfer engine of M2 will not ask.
 */

import { api, LOCAL, type Connected, type DirEntry, type Listing, type Protocol } from "../bridge";
import type { CommandId } from "../ui/commands";

export type Side = "left" | "right";
export type SortColumn = "name" | "size" | "modified";

export interface PaneState {
  /** Which endpoint this pane reads. */
  endpoint: string;
  /** What this endpoint speaks, so the pane can say when nothing is encrypted. */
  protocol: Protocol;
  /**
   * Whether this connection stands on a certificate the user accepted by hand
   * rather than one an authority vouches for. Shown for as long as it holds:
   * an exception that is invisible is an exception nobody remembers making.
   */
  certificateAccepted: boolean;
  /** Label shown in the pane header: host name, or the local machine. */
  title: string | null;
  /** The quick connect entry behind this connection, for remembering paths. */
  historyId: string | null;
  path: string;
  entries: DirEntry[];
  /**
   * Names the user has marked. Names rather than indexes, because a refresh or
   * a change of sorting moves every row. A new Set is assigned on every change:
   * a Set is not deeply reactive, so mutating it in place would not redraw.
   */
  selected: Set<string>;
  /** The row the keyboard is on. */
  cursor: number;
  sortBy: SortColumn;
  sortAscending: boolean;
  showHidden: boolean;
  /** Whether the folder tree beside the list is shown. */
  showTree: boolean;
  /**
   * How wide it is, in pixels.
   *
   * Per side rather than shared: somebody with a deep tree on the remote and a
   * flat one locally wants two different widths, and making them agree would
   * only mean setting it twice.
   */
  treeWidth: number;
  /**
   * What is typed into the filter, narrowing the list as it is typed.
   *
   * Cleared on every change of directory, and that is deliberate. A filter that
   * quietly survives into the next folder hides files nobody knows are there —
   * and in a program that deletes and overwrites, a list that is silently
   * incomplete is the dangerous kind of wrong.
   */
  filter: string;
  /** Whether the filter field is shown at all. */
  filtering: boolean;
  /** Name of the row being renamed in place, or null. */
  renaming: string | null;
  /**
   * A command the keyboard asked this pane to run, which the pane clears when
   * it has. The same path the toolbar and the menu take, rather than a second
   * one beside it — two ways to delete a file is one too many.
   */
  requested: CommandId | null;
  busy: boolean;
  /** Set when the last attempt failed, so the pane can say so. */
  failure: unknown;
  /** Directories opened in the tree, so it can be drawn without re-asking. */
  expanded: Set<string>;
}

/** The narrowest and widest the tree may be dragged, and where it starts. */
export const TREE_MIN = 120;
export const TREE_MAX = 480;
export const TREE_DEFAULT = 186;

function emptyPane(): PaneState {
  return {
    endpoint: LOCAL,
    protocol: "local",
    certificateAccepted: false,
    title: null,
    historyId: null,
    path: "",
    entries: [],
    selected: new Set<string>(),
    cursor: 0,
    sortBy: "name",
    sortAscending: true,
    // Shown by default. This is a file transfer client, and the files its
    // users came for — .htaccess, .env, .gitignore — all begin with a dot.
    // Hiding them by default would hide the point of the program.
    showHidden: true,
    showTree: true,
    treeWidth: TREE_DEFAULT,
    renaming: null,
    requested: null,
    filter: "",
    filtering: false,
    busy: false,
    failure: null,
    expanded: new Set<string>(),
  };
}

let panes = $state<Record<Side, PaneState>>({
  left: emptyPane(),
  right: emptyPane(),
});

let focused = $state<Side>("left");

export function pane(side: Side): PaneState {
  return panes[side];
}

export function focusedSide(): Side {
  return focused;
}

export function focusPane(side: Side): void {
  focused = side;
}

/** F6 and Tab: the other side. */
export function switchFocus(): void {
  focused = focused === "left" ? "right" : "left";
}

/** Rows in the order the pane shows them: directories first, then the sort. */
export function visibleEntries(side: Side): DirEntry[] {
  const state = panes[side];
  let rows = state.showHidden
    ? state.entries
    : state.entries.filter((entry) => !entry.name.startsWith("."));

  const needle = state.filter.trim().toLowerCase();
  if (needle) {
    rows = rows.filter((entry) => entry.name.toLowerCase().includes(needle));
  }

  const direction = state.sortAscending ? 1 : -1;
  return [...rows].sort((a, b) => {
    // Directories keep to the top whichever way the column is sorted. Mixing
    // them into a size sort is technically consistent and practically useless.
    const aDir = isDirectory(a);
    const bDir = isDirectory(b);
    if (aDir !== bDir) return aDir ? -1 : 1;

    switch (state.sortBy) {
      case "size":
        return direction * ((a.size ?? -1) - (b.size ?? -1));
      case "modified":
        return direction * ((a.modified ?? 0) - (b.modified ?? 0));
      default:
        return direction * a.name.localeCompare(b.name, undefined, { numeric: true });
    }
  });
}

export function isDirectory(entry: DirEntry): boolean {
  return (
    entry.kind === "directory" ||
    (entry.kind === "symlink" && entry.kindOfTarget === "directory")
  );
}

/** Shows or hides the filter field, and clears it on the way out. */
export function setFiltering(side: Side, on: boolean): void {
  panes[side].filtering = on;
  if (!on) panes[side].filter = "";
}

export function setFilter(side: Side, text: string): void {
  panes[side].filter = text;
  // The cursor goes to the top of whatever is left, or it would point at a row
  // the filter has just taken away.
  panes[side].cursor = 0;
}

/** Asks a pane to run one of its own commands. */
export function requestCommand(side: Side, id: CommandId): void {
  panes[side].requested = id;
}

export function clearRequest(side: Side): void {
  panes[side].requested = null;
}

/** Points a pane at an endpoint and reads its starting directory. */
export async function openSession(
  side: Side,
  session: Connected,
  title: string | null,
  historyId: string | null,
  startPath?: string | null,
  certificateAccepted = false,
): Promise<void> {
  const state = panes[side];
  // Endpoint and path change together. Setting the endpoint first and reading
  // the directory afterwards leaves a moment where the pane claims to be on
  // the new server while still showing the old path — and anything deriving
  // from the pane in that moment asks the new server for the old path. That
  // really happened: a freshly connected server was asked for /Users/sphings.
  state.endpoint = session.endpoint;
  state.protocol = session.protocol;
  state.certificateAccepted = certificateAccepted;
  state.title = title;
  state.historyId = historyId;
  state.path = "";
  state.entries = [];
  state.selected = new Set<string>();
  state.cursor = 0;
  state.failure = null;
  state.expanded = new Set<string>();
  await navigate(side, startPath || session.home);
}

/** Reads a directory into a pane. */
/**
 * Whether the two panes walk together.
 *
 * One switch for both sides rather than one each: "together" is a relation and
 * not a property of either pane, and two switches that could disagree would
 * mean a state nobody could describe.
 */
let together = $state(false);

/** Set while a pane is being moved by the other, so it does not move it back. */
let following = false;

export function browsingTogether(): boolean {
  return together;
}

export function setBrowsingTogether(on: boolean): void {
  together = on;
}

/**
 * How the window asks whether to create a directory that is not there.
 *
 * A callback rather than a dialog here: this module knows about paths and
 * listings and has no business drawing anything. The window installs its own
 * way of asking, and a window that installs none simply never creates
 * anything.
 */
let askToCreate: ((side: Side, path: string) => Promise<boolean>) | null = null;

export function whenDirectoryMissing(ask: (side: Side, path: string) => Promise<boolean>): void {
  askToCreate = ask;
}

/**
 * Where the other side should go, given where this one went.
 *
 * Down: the same names appended to where the other side is. Up: the same
 * number of levels up. Anywhere else — a path typed in that is not above or
 * below where we were — has no counterpart by name, and the honest answer is
 * to leave the other side where it is rather than guess.
 */
function steps(before: string, after: string): { down: string[] } | { up: number } | null {
  const cut = (path: string) => path.split("/").filter((part) => part !== "");
  const from = cut(before);
  const to = cut(after);

  if (to.length > from.length && from.every((part, at) => to[at] === part)) {
    return { down: to.slice(from.length) };
  }
  if (from.length > to.length && to.every((part, at) => from[at] === part)) {
    return { up: from.length - to.length };
  }
  return null;
}

async function follow(mover: Side, before: string, after: string): Promise<void> {
  const other: Side = mover === "left" ? "right" : "left";
  const move = steps(before, after);
  if (!move) return;

  const state = panes[other];
  const started = state.path;
  following = true;

  try {
    if ("up" in move) {
      let target = started;
      for (let step = 0; step < move.up; step += 1) {
        const parent = await api.parentOf(state.endpoint, target);
        if (!parent || parent === target) break;
        target = parent;
      }
      if (target !== started) await navigate(other, target);
      return;
    }

    // One level at a time, because each of them can be the one that is not
    // there, and because creating a directory takes a name and a place to put
    // it rather than a finished path — which is what keeps a name with a
    // slash in it from landing somewhere else entirely.
    let at = started;
    for (const name of move.down) {
      const next = await api.joinPath(state.endpoint, at, name);
      await navigate(other, next);

      if (state.failure) {
        // The question replaces the complaint rather than joining it. A red
        // line saying a directory is missing, under a dialog asking whether to
        // create it, is the same thing said twice — and the first one is not
        // this pane's news to report.
        state.failure = null;

        // Asked only after the move failed: a directory that is already there
        // is the ordinary case and must not produce a question.
        const make = askToCreate ? await askToCreate(other, next) : false;
        if (!make) {
          // Back where it was. A pane showing an error because the *other*
          // pane moved is a pane reporting somebody else's problem.
          await navigate(other, started);
          return;
        }
        await api.createDir(state.endpoint, at, name);
        await navigate(other, next);
        if (state.failure) {
          await navigate(other, started);
          return;
        }
      }
      at = next;
    }
  } finally {
    following = false;
  }
}

export async function navigate(side: Side, path: string): Promise<void> {
  const state = panes[side];
  const before = state.path;
  state.busy = true;
  state.failure = null;
  try {
    const listing: Listing = await api.listDir(state.endpoint, path);
    state.path = listing.path;
    state.entries = listing.entries;
    state.selected = new Set<string>();
    state.cursor = 0;
    state.renaming = null;
    // A filter belongs to the directory it was typed in. Carrying it into the
    // next one hides files nobody knows are there.
    state.filter = "";
    if (state.historyId) {
      // Remembering where a server was left is what makes reconnecting feel
      // like coming back rather than starting over.
      void api.rememberPath(state.historyId, listing.path).catch(() => undefined);
    }
  } catch (failure) {
    state.failure = failure;
  } finally {
    state.busy = false;
  }

  // After the move and outside the try, so a pane that could not move does not
  // drag the other one anywhere.
  if (together && !following && !state.failure && state.path !== before) {
    await follow(side, before, state.path);
  }
}

export async function reload(side: Side): Promise<void> {
  await navigate(side, panes[side].path);
}

/** Enter: into a directory, or nothing for a file until M2 gives files meaning. */
export async function enter(side: Side, entry: DirEntry): Promise<void> {
  if (!isDirectory(entry)) return;
  const state = panes[side];
  const path = await api.joinPath(state.endpoint, state.path, entry.name);
  await navigate(side, path);
}

/** Backspace: one directory up, if there is one. */
export async function goUp(side: Side): Promise<void> {
  const state = panes[side];
  const parent = await api.parentOf(state.endpoint, state.path);
  if (parent && parent !== state.path) {
    await navigate(side, parent);
  }
}

export function moveCursor(side: Side, delta: number, rowCount: number): void {
  const state = panes[side];
  state.cursor = Math.max(0, Math.min(rowCount - 1, state.cursor + delta));
}

export function setCursor(side: Side, index: number): void {
  panes[side].cursor = index;
}

/** Space or Insert: mark a row and step on, the way a commander does. */
export function toggleSelection(side: Side, name: string): void {
  const state = panes[side];
  const next = new Set(state.selected);
  if (next.has(name)) {
    next.delete(name);
  } else {
    next.add(name);
  }
  state.selected = next;
}

export function setSort(side: Side, column: SortColumn): void {
  const state = panes[side];
  if (state.sortBy === column) {
    state.sortAscending = !state.sortAscending;
  } else {
    state.sortBy = column;
    state.sortAscending = true;
  }
}

export function toggleHidden(side: Side): void {
  panes[side].showHidden = !panes[side].showHidden;
}

export function setHiddenVisible(side: Side, visible: boolean): void {
  panes[side].showHidden = visible;
}

/** The row the keyboard is on, if there is one. */
export function currentEntry(side: Side): DirEntry | null {
  const rows = visibleEntries(side);
  return rows[panes[side].cursor] ?? null;
}

/**
 * What an operation applies to: the marked rows, or the one under the cursor.
 *
 * Marking a row and then operating on the one under the cursor instead is the
 * kind of surprise that deletes the wrong thing, so marks win whenever there
 * are any.
 */
export function targets(side: Side): DirEntry[] {
  const state = panes[side];
  if (state.selected.size > 0) {
    return visibleEntries(side).filter((entry) => state.selected.has(entry.name));
  }
  const entry = currentEntry(side);
  return entry ? [entry] : [];
}

/** The name being renamed in place, or null. */
export function renaming(side: Side): string | null {
  return panes[side].renaming;
}

export function startRename(side: Side, name: string): void {
  panes[side].renaming = name;
}

export function stopRename(side: Side): void {
  panes[side].renaming = null;
}

export function toggleTree(side: Side): void {
  panes[side].showTree = !panes[side].showTree;
}

export function setTreeVisible(side: Side, visible: boolean): void {
  panes[side].showTree = visible;
}

export function setTreeWidth(side: Side, width: number): void {
  panes[side].treeWidth = Math.min(TREE_MAX, Math.max(TREE_MIN, Math.round(width)));
}

export function toggleExpanded(side: Side, path: string): void {
  const state = panes[side];
  const next = new Set(state.expanded);
  if (next.has(path)) {
    next.delete(path);
  } else {
    next.add(path);
  }
  state.expanded = next;
}
