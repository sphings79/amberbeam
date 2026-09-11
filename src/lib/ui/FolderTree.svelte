<script lang="ts">
  import { api, type DirEntry } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import { isDirectory, navigate, pane, type Side } from "../state/panes.svelte";

  interface Props {
    side: Side;
  }

  let { side }: Props = $props();

  let view = $derived(pane(side));

  /** Directories read so far, by path. Nothing is read before it is opened. */
  let children = $state<Record<string, DirEntry[]>>({});
  let loading = $state<Set<string>>(new Set());
  let open = $state<Set<string>>(new Set());

  interface Row {
    path: string;
    name: string;
    depth: number;
    expanded: boolean;
  }

  /**
   * The chain from the root down to the current directory, plus whatever the
   * user opened. Subfolders are read on expanding and never before: a server
   * with thousands of directories is unusable any other way, which the concept
   * paper says in so many words.
   */
  let rows = $derived.by<Row[]>(() => {
    const path = view.path;
    if (!path) return [];

    const separator = path.includes("\\") && !path.startsWith("/") ? "\\" : "/";
    const parts = path.split(separator).filter((part) => part.length > 0);
    const chain: Row[] = [];
    let walked = separator === "/" ? "" : "";

    // The root itself, so there is always something to click back to.
    chain.push({
      path: separator === "/" ? "/" : parts[0] ?? path,
      name: separator === "/" ? "/" : (parts[0] ?? path),
      depth: 0,
      expanded: true,
    });

    parts.forEach((part, index) => {
      walked = `${walked}${separator}${part}`;
      if (separator === "/" || index > 0) {
        chain.push({ path: walked, name: part, depth: index + 1, expanded: true });
      }
    });

    // Under the deepest opened directory, its own subfolders.
    const out: Row[] = [];
    for (const row of chain) {
      out.push(row);
    }
    const deepest = chain[chain.length - 1];
    if (deepest) {
      for (const entry of children[deepest.path] ?? []) {
        out.push({
          path: `${deepest.path === "/" ? "" : deepest.path}${separator}${entry.name}`,
          name: entry.name,
          depth: deepest.depth + 1,
          expanded: open.has(entry.name),
        });
      }
    }
    return out;
  });

  // The directory a pane is showing has its subfolders read once, so the tree
  // is not empty below the current level.
  $effect(() => {
    const path = view.path;
    const endpoint = view.endpoint;
    if (!path || children[path] || loading.has(path)) return;
    loading = new Set(loading).add(path);
    api
      .listDir(endpoint, path)
      .then((listing) => {
        children = { ...children, [path]: listing.entries.filter(isDirectory) };
      })
      .catch(() => {
        children = { ...children, [path]: [] };
      })
      .finally(() => {
        const next = new Set(loading);
        next.delete(path);
        loading = next;
      });
  });

  // A different endpoint means a different tree.
  $effect(() => {
    void view.endpoint;
    children = {};
    open = new Set();
  });
</script>

<nav class="tree" aria-label={t("pane.tree")}>
  {#each rows as row (row.path + row.depth)}
    <button
      type="button"
      class="row"
      class:current={row.path === view.path}
      style:padding-left="{8 + row.depth * 11}px"
      onclick={() => navigate(side, row.path)}
      title={row.path}
    >
      <span class="glyph" aria-hidden="true">{row.path === view.path ? "▾" : "▸"}</span>
      <span class="name">{row.name}</span>
    </button>
  {/each}
</nav>

<style>
  .tree {
    width: 170px;
    flex: none;
    overflow: auto;
    background: var(--surface-2);
    border-right: 1px solid var(--border);
    padding: 4px 0;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 4px;
    width: 100%;
    border: none;
    background: none;
    font: inherit;
    font-size: 0.8rem;
    color: var(--text-muted);
    padding: 3px 8px;
    cursor: default;
    text-align: left;
  }

  .row:hover {
    background: var(--surface-3);
  }

  .row.current {
    color: var(--accent);
  }

  .glyph {
    color: var(--text-faint);
    font-size: 0.6rem;
    flex: none;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
