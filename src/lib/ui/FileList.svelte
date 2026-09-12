<script lang="ts">
  import { elementScroll, observeElementOffset, observeElementRect, Virtualizer } from "@tanstack/virtual-core";

  import type { DirEntry } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import {
    clearSelection,
    isDirectory,
    isUp,
    selectTo,
    pane,
    renaming,
    setCursor,
    setSort,
    stopRename,
    toggleSelection,
    visibleEntries,
    type Side,
    type SortColumn,
  } from "../state/panes.svelte";
  import { formatDate, formatPermissions, formatSize } from "./format";

  interface Props {
    side: Side;
    onenter: (entry: DirEntry) => void;
    oncontext: (entry: DirEntry | null, x: number, y: number) => void;
    onrename: (entry: DirEntry, to: string) => void;
  }

  let { side, onenter, oncontext, onrename }: Props = $props();

  const ROW = 24;

  let scroller = $state<HTMLDivElement | null>(null);
  let rows = $derived(visibleEntries(side));
  let view = $derived(pane(side));

  // Virtualised because the concept paper asks for fifty thousand entries to
  // scroll smoothly, and drawing fifty thousand rows never will.
  let virtualizer = $state<Virtualizer<HTMLDivElement, HTMLDivElement> | null>(null);
  let visible = $state<{ index: number; start: number }[]>([]);
  let total = $state(0);

  $effect(() => {
    if (!scroller) return;
    const instance = new Virtualizer<HTMLDivElement, HTMLDivElement>({
      count: rows.length,
      getScrollElement: () => scroller,
      estimateSize: () => ROW,
      overscan: 12,
      observeElementRect,
      observeElementOffset,
      scrollToFn: elementScroll,
      onChange: (self) => {
        visible = self.getVirtualItems().map((item) => ({ index: item.index, start: item.start }));
        total = self.getTotalSize();
      },
    });
    const stop = instance._didMount();
    instance._willUpdate();
    visible = instance.getVirtualItems().map((item) => ({ index: item.index, start: item.start }));
    total = instance.getTotalSize();
    virtualizer = instance;
    return () => {
      stop();
      virtualizer = null;
    };
  });

  // Row count and cursor both change from outside — keep the window in step.
  $effect(() => {
    const count = rows.length;
    const cursor = view.cursor;
    if (!virtualizer) return;
    virtualizer.setOptions({ ...virtualizer.options, count });
    virtualizer._willUpdate();
    visible = virtualizer.getVirtualItems().map((item) => ({ index: item.index, start: item.start }));
    total = virtualizer.getTotalSize();
    if (count > 0) {
      virtualizer.scrollToIndex(Math.min(cursor, count - 1), { align: "auto" });
    }
  });

  const columns: { key: SortColumn; label: string; className: string }[] = $derived([
    { key: "name", label: t("column.name"), className: "name" },
    { key: "size", label: t("column.size"), className: "size" },
    { key: "modified", label: t("column.modified"), className: "modified" },
  ]);

  function onRowClick(index: number, entry: DirEntry, event: MouseEvent): void {
    // Shift first: holding it means "everything from where I was to here",
    // and that has to be read before the cursor moves, because where I was is
    // exactly what the cursor still knows.
    if (event.shiftKey) {
      selectTo(side, index);
      return;
    }
    setCursor(side, index);
    if (event.metaKey || event.ctrlKey) {
      toggleSelection(side, entry.name);
      return;
    }
    // A plain click starts again. Marks that survive one are marks somebody
    // has to remember making, and the next command would work on rows they
    // cannot see — which is the one thing a program that deletes must not do.
    //
    // Marking with the space bar is untouched: that is the commander way of
    // building a selection up, and it is meant to accumulate.
    clearSelection(side);
  }

  function onRowContext(index: number, entry: DirEntry, event: MouseEvent): void {
    event.preventDefault();
    // A right click that lands outside the marked rows works on the row it hit,
    // which is what every file manager does — and what stops a menu acting on
    // something the user cannot see.
    if (!view.selected.has(entry.name)) {
      setCursor(side, index);
    }
    oncontext(entry, event.clientX, event.clientY);
  }

  function commitRename(entry: DirEntry, value: string): void {
    const trimmed = value.trim();
    if (trimmed && trimmed !== entry.name) {
      onrename(entry, trimmed);
    } else {
      stopRename(side);
    }
  }
</script>

<div class="list">
  <div class="head" role="row">
    {#each columns as column (column.key)}
      <button
        type="button"
        class={column.className}
        class:sorted={view.sortBy === column.key}
        onclick={() => setSort(side, column.key)}
      >
        {column.label}
        {#if view.sortBy === column.key}
          <span class="caret">{view.sortAscending ? "▲" : "▼"}</span>
        {/if}
      </button>
    {/each}
    <span class="rights">{t("column.permissions")}</span>
    <span class="owner">{t("column.owner")}</span>
  </div>

  <div
    class="scroller"
    bind:this={scroller}
    role="listbox"
    tabindex="-1"
    aria-label={view.path}
    oncontextmenu={(event) => {
      // A right click on empty space still offers the commands that need no
      // row — making a folder, for instance.
      if (event.target === event.currentTarget) {
        event.preventDefault();
        oncontext(null, event.clientX, event.clientY);
      }
    }}
  >
    {#if rows.length === 0}
      <p class="empty">{view.busy ? t("pane.loading") : t("pane.empty")}</p>
    {:else}
      <div class="canvas" style:height="{total}px">
        {#each visible as item (item.index)}
          {@const entry = rows[item.index]}
          {#if entry}
            <div
              class="row"
              class:cursor={view.cursor === item.index}
              class:marked={view.selected.has(entry.name)}
              class:directory={isDirectory(entry)}
              class:up={isUp(entry)}
              style:transform="translateY({item.start}px)"
              style:height="{ROW}px"
              role="option"
              aria-selected={view.selected.has(entry.name)}
              tabindex="-1"
              draggable={!isUp(entry)}
              ondragstart={(event) => {
                // What is dragged is what an operation would work on: the
                // marked rows, or the one under the pointer.
                if (!view.selected.has(entry.name)) {
                  setCursor(side, item.index);
                }
                const names = view.selected.size > 0 ? [...view.selected] : [entry.name];
                event.dataTransfer?.setData(
                  "application/x-amberbeam",
                  JSON.stringify({ side, names }),
                );
                if (event.dataTransfer) event.dataTransfer.effectAllowed = "copy";
              }}
              onclick={(event) => onRowClick(item.index, entry, event)}
              ondblclick={() => onenter(entry)}
              oncontextmenu={(event) => onRowContext(item.index, entry, event)}
              onkeydown={() => {}}
            >
              <span class="name" title={entry.linkTarget ?? entry.name}>
                <span class="glyph" aria-hidden="true">
                  {isDirectory(entry) ? "▸" : entry.kind === "symlink" ? "↗" : "·"}
                </span>
                {#if renaming(side) === entry.name}
                  <!-- svelte-ignore a11y_autofocus -->
                  <input
                    class="rename"
                    value={entry.name}
                    autofocus
                    autocomplete="off"
                    autocapitalize="off"
                    autocorrect="off"
                    spellcheck="false"
                    onclick={(event) => event.stopPropagation()}
                    ondblclick={(event) => event.stopPropagation()}
                    onblur={(event) => commitRename(entry, event.currentTarget.value)}
                    onkeydown={(event) => {
                      event.stopPropagation();
                      if (event.key === "Enter") {
                        commitRename(entry, event.currentTarget.value);
                      } else if (event.key === "Escape") {
                        stopRename(side);
                      }
                    }}
                  />
                {:else}
                  {entry.name}
                {/if}
              </span>
              <span class="size">{isDirectory(entry) ? "—" : formatSize(entry.size)}</span>
              <span class="modified">{formatDate(entry.modified)}</span>
              <span class="rights mono">{formatPermissions(entry.permissions)}</span>
              <span class="owner">{entry.owner ?? ""}</span>
            </div>
          {/if}
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .list {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
    background: var(--surface-1);
  }

  .head,
  .row {
    display: grid;
    grid-template-columns: minmax(120px, 1fr) 92px 132px 82px 70px;
    align-items: center;
    gap: 0;
  }

  .head {
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
    position: sticky;
    top: 0;
    z-index: 1;
  }

  .head button,
  .head span {
    font: inherit;
    font-size: 0.68rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-faint);
    background: none;
    border: none;
    text-align: left;
    padding: 5px 8px;
    cursor: default;
  }

  .head button:hover {
    color: var(--text-muted);
  }

  .head button.sorted {
    color: var(--accent);
  }

  .caret {
    font-size: 0.6rem;
  }

  .scroller {
    flex: 1;
    overflow: auto;
    position: relative;
    outline: none;
  }

  .canvas {
    position: relative;
    width: 100%;
  }

  .row {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    font-size: 0.82rem;
    color: var(--text-muted);
    border-bottom: 1px solid transparent;
  }

  .row:hover {
    background: var(--surface-2);
  }

  /* Quieter than a real directory. It is a way out, not a thing in the list,
     and giving it the same weight would put it in competition with the names
     somebody is actually reading. */
  .row.up .name {
    color: var(--text-faint);
  }

  .row.directory .name {
    color: var(--text);
  }

  .row.marked {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .row.cursor {
    box-shadow: inset 2px 0 0 var(--accent);
    background: var(--surface-3);
  }

  .row.marked.cursor {
    background: var(--accent-soft);
  }

  .row > span,
  .head > span {
    padding: 0 8px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .glyph {
    display: inline-block;
    width: 12px;
    color: var(--accent);
  }

  .size,
  .modified,
  .rights {
    font-variant-numeric: tabular-nums;
    font-size: 0.78rem;
  }

  .size {
    text-align: right;
  }

  .empty {
    margin: 0;
    padding: 18px;
    color: var(--text-faint);
    font-size: 0.84rem;
  }

  .rename {
    font: inherit;
    font-size: 0.82rem;
    width: calc(100% - 16px);
    padding: 1px 4px;
    border: 1px solid var(--accent);
    border-radius: 3px;
    background: var(--surface-1);
    color: var(--text);
  }

  .rename:focus {
    outline: none;
    box-shadow: 0 0 0 2px var(--accent-ring);
  }
</style>
