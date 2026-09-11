<script lang="ts">
  import { api, type DirEntry } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import { isDirectory, navigate, pane, type Side } from "../state/panes.svelte";

  interface Props {
    side: Side;
  }

  let { side }: Props = $props();

  let view = $derived(pane(side));

  /**
   * Folders per directory, read once and kept.
   *
   * The directory the pane is showing is never fetched here — the pane already
   * has it, and asking twice doubled every line in the server log. Everything
   * above and beside it is fetched once and remembered, which is what lets the
   * whole path stay open with its neighbours in view.
   */
  let children = $state<Record<string, DirEntry[]>>({});
  let loading = $state<Set<string>>(new Set());
  /** Folders the user opened by hand, beyond the ones on the current path. */
  let opened = $state<Set<string>>(new Set());
  /** Which endpoint the cache belongs to. */
  let cachedFor = $state<string | null>(null);

  let separator = $derived(
    /^[A-Za-z]:/.test(view.path) || view.path.includes("\\") ? "\\" : "/",
  );

  function join(directory: string, name: string): string {
    return directory.endsWith(separator)
      ? `${directory}${name}`
      : `${directory}${separator}${name}`;
  }

  /** Root first, then every directory down to the one being shown. */
  let chain = $derived.by<string[]>(() => {
    const path = view.path;
    if (!path) return [];
    const windowsStyle = /^[A-Za-z]:/.test(path);
    const parts = path.split(separator).filter((part) => part.length > 0);
    const root = windowsStyle ? (parts.shift() ?? path) : separator;

    const out = [root];
    let walked = windowsStyle ? root : "";
    for (const part of parts) {
      walked = `${walked}${separator}${part}`;
      out.push(walked);
    }
    return out;
  });

  function childrenOf(path: string): DirEntry[] | null {
    if (path === view.path) {
      return view.entries.filter(isDirectory);
    }
    return children[path] ?? null;
  }

  /** A different connection means a different tree. */
  $effect(() => {
    const endpoint = view.endpoint;
    if (cachedFor !== endpoint) {
      children = {};
      opened = new Set();
      cachedFor = endpoint;
    }
  });

  // Everything that should be visible gets read once: the directories along
  // the current path, and whatever the user opened by hand.
  $effect(() => {
    const endpoint = view.endpoint;
    const wanted = [...chain, ...opened].filter(
      (path) => path !== view.path && !(path in children) && !loading.has(path),
    );
    for (const path of wanted) {
      loading = new Set(loading).add(path);
      api
        .listDir(endpoint, path)
        .then((listing) => {
          children = { ...children, [path]: listing.entries.filter(isDirectory) };
        })
        .catch(() => {
          // A directory that cannot be read shows as empty rather than
          // retrying forever.
          children = { ...children, [path]: [] };
        })
        .finally(() => {
          const next = new Set(loading);
          next.delete(path);
          loading = next;
        });
    }
  });

  interface Row {
    path: string;
    name: string;
    depth: number;
    current: boolean;
    expandable: boolean;
    expanded: boolean;
  }

  function build(path: string, depth: number, out: Row[]): void {
    const rows = childrenOf(path);
    if (!rows) return;
    for (const entry of rows) {
      if (!view.showHidden && entry.name.startsWith(".")) continue;
      const childPath = join(path, entry.name);
      const onPath = chain.includes(childPath);
      const isOpen = onPath || opened.has(childPath);
      out.push({
        path: childPath,
        name: entry.name,
        depth,
        current: childPath === view.path,
        expandable: true,
        expanded: isOpen,
      });
      if (isOpen) {
        build(childPath, depth + 1, out);
      }
    }
  }

  let rows = $derived.by<Row[]>(() => {
    const root = chain[0];
    if (!root) return [];
    const out: Row[] = [
      {
        path: root,
        name: root,
        depth: 0,
        current: root === view.path,
        expandable: true,
        expanded: true,
      },
    ];
    build(root, 1, out);
    return out;
  });

  function toggle(row: Row, event: MouseEvent): void {
    event.stopPropagation();
    // A folder on the way to the one being shown cannot be folded away: it
    // would hide the current directory, which is the one thing the tree has to
    // keep in view.
    if (chain.includes(row.path) && row.path !== view.path) return;
    const next = new Set(opened);
    if (next.has(row.path)) {
      next.delete(row.path);
    } else {
      next.add(row.path);
    }
    opened = next;
  }
</script>

<nav class="tree" aria-label={t("pane.tree")}>
  {#each rows as row (row.path)}
    <div class="row" class:current={row.current} style:padding-left="{4 + row.depth * 12}px">
      <button
        type="button"
        class="twist"
        onclick={(event) => toggle(row, event)}
        aria-label={row.expanded ? t("tree.collapse") : t("tree.expand")}
        aria-expanded={row.expanded}
      >
        {row.expanded ? "▾" : "▸"}
      </button>
      <button type="button" class="name" onclick={() => navigate(side, row.path)} title={row.path}>
        {row.name}
      </button>
    </div>
  {/each}
</nav>

<style>
  .tree {
    width: 186px;
    flex: none;
    overflow: auto;
    background: var(--surface-2);
    border-right: 1px solid var(--border);
    padding: 4px 0;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 2px;
    padding-right: 6px;
  }

  .row:hover {
    background: var(--surface-3);
  }

  .row.current {
    background: var(--accent-soft);
  }

  .twist,
  .name {
    border: none;
    background: none;
    font: inherit;
    cursor: default;
    color: var(--text-muted);
    padding: 3px 2px;
  }

  .twist {
    font-size: 0.6rem;
    color: var(--text-faint);
    width: 14px;
    flex: none;
  }

  .name {
    flex: 1;
    min-width: 0;
    font-size: 0.8rem;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row.current .name {
    color: var(--accent);
    font-weight: 600;
  }
</style>
