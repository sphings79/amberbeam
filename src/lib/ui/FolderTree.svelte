<script lang="ts">
  import { t } from "../i18n/index.svelte";
  import { isDirectory, navigate, pane, type Side } from "../state/panes.svelte";

  interface Props {
    side: Side;
  }

  let { side }: Props = $props();

  let view = $derived(pane(side));

  interface Row {
    path: string;
    name: string;
    depth: number;
    current: boolean;
  }

  /**
   * The chain from the root down to the current directory, and under it the
   * subfolders of that directory.
   *
   * Everything here comes from what the pane already read. Asking the server a
   * second time for a listing it just fetched doubled every line in the server
   * log and every round trip on the wire — and a directory with fifty thousand
   * entries is not something to fetch twice for decoration.
   */
  let rows = $derived.by<Row[]>(() => {
    const path = view.path;
    if (!path) return [];

    const windowsStyle = /^[A-Za-z]:/.test(path);
    const separator = windowsStyle || path.includes("\\") ? "\\" : "/";
    const parts = path.split(separator).filter((part) => part.length > 0);

    const out: Row[] = [];
    // The root: "/" on POSIX, the drive on Windows. Always there, so there is
    // always something to climb back to.
    const root = windowsStyle ? (parts.shift() ?? path) : separator;
    out.push({ path: root, name: root, depth: 0, current: root === path });

    let walked = windowsStyle ? root : "";
    parts.forEach((part, index) => {
      walked = windowsStyle && index === 0 ? `${root}${separator}${part}` : `${walked}${separator}${part}`;
      out.push({ path: walked, name: part, depth: index + 1, current: walked === path });
    });

    const depth = out.length;
    for (const entry of view.entries.filter(isDirectory)) {
      if (!view.showHidden && entry.name.startsWith(".")) continue;
      const child = path.endsWith(separator)
        ? `${path}${entry.name}`
        : `${path}${separator}${entry.name}`;
      out.push({ path: child, name: entry.name, depth, current: false });
    }
    return out;
  });
</script>

<nav class="tree" aria-label={t("pane.tree")}>
  {#each rows as row (row.path + row.depth)}
    <button
      type="button"
      class="row"
      class:current={row.current}
      style:padding-left="{8 + row.depth * 11}px"
      onclick={() => navigate(side, row.path)}
      title={row.path}
    >
      <span class="glyph" aria-hidden="true">{row.current ? "▾" : "▸"}</span>
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
