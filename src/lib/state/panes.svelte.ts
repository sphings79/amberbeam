/**
 * The two file panes.
 *
 * Seam 1 again, one level up: a pane holds an endpoint and a path, and neither
 * pane is "the local one". Which side happens to be local is a matter of what
 * it was pointed at, and the transfer engine of M2 will not ask.
 */

import { api, LOCAL, type Connected, type DirEntry, type Listing } from "../bridge";

export type Side = "left" | "right";
export type SortColumn = "name" | "size" | "modified";

export interface PaneState {
  /** Which endpoint this pane reads. */
  endpoint: string;
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
  busy: boolean;
  /** Set when the last attempt failed, so the pane can say so. */
  failure: unknown;
  /** Directories opened in the tree, so it can be drawn without re-asking. */
  expanded: Set<string>;
}

function emptyPane(): PaneState {
  return {
    endpoint: LOCAL,
    title: null,
    historyId: null,
    path: "",
    entries: [],
    selected: new Set<string>(),
    cursor: 0,
    sortBy: "name",
    sortAscending: true,
    showHidden: false,
    showTree: true,
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
  const rows = state.showHidden
    ? state.entries
    : state.entries.filter((entry) => !entry.name.startsWith("."));

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

/** Points a pane at an endpoint and reads its starting directory. */
export async function openSession(
  side: Side,
  session: Connected,
  title: string | null,
  historyId: string | null,
  startPath?: string | null,
): Promise<void> {
  const state = panes[side];
  // Endpoint and path change together. Setting the endpoint first and reading
  // the directory afterwards leaves a moment where the pane claims to be on
  // the new server while still showing the old path — and anything deriving
  // from the pane in that moment asks the new server for the old path. That
  // really happened: a freshly connected server was asked for /Users/sphings.
  state.endpoint = session.endpoint;
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
export async function navigate(side: Side, path: string): Promise<void> {
  const state = panes[side];
  state.busy = true;
  state.failure = null;
  try {
    const listing: Listing = await api.listDir(state.endpoint, path);
    state.path = listing.path;
    state.entries = listing.entries;
    state.selected = new Set<string>();
    state.cursor = 0;
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

export function toggleTree(side: Side): void {
  panes[side].showTree = !panes[side].showTree;
}

export function setTreeVisible(side: Side, visible: boolean): void {
  panes[side].showTree = visible;
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
