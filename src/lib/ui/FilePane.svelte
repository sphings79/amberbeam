<script lang="ts">
  import { LOCAL, type DirEntry } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import {
    enter,
    focusedSide,
    focusPane,
    goUp,
    pane,
    reload,
    toggleHidden,
    toggleTree,
    visibleEntries,
    type Side,
  } from "../state/panes.svelte";
  import { describe } from "./errors";
  import FileList from "./FileList.svelte";
  import FolderTree from "./FolderTree.svelte";

  interface Props {
    side: Side;
    onquickconnect: () => void;
    ondisconnect: () => void;
  }

  let { side, onquickconnect, ondisconnect }: Props = $props();

  let view = $derived(pane(side));
  let active = $derived(focusedSide() === side);
  let remote = $derived(view.endpoint !== LOCAL);
  let rows = $derived(visibleEntries(side));

  async function open(entry: DirEntry): Promise<void> {
    await enter(side, entry);
  }
</script>

<section
  class="pane"
  class:active
  onpointerdown={() => focusPane(side)}
  aria-label={remote ? t("pane.server") : t("pane.local")}
>
  <header>
    <span class="label">{remote ? t("pane.server") : t("pane.local")}</span>
    <span class="where mono" title={view.title ?? ""}>
      {view.title ?? t("pane.this-machine")}
    </span>
    <span class="spacer"></span>
    {#if remote}
      <button type="button" onclick={ondisconnect} title={t("pane.disconnect")}>⏻</button>
    {:else}
      <button type="button" onclick={onquickconnect} title={t("pane.connect")}>＋</button>
    {/if}
    <button
      type="button"
      class:on={view.showTree}
      onclick={() => toggleTree(side)}
      title={t("pane.tree.toggle")}
    >
      ⊞
    </button>
    <button type="button" onclick={() => toggleHidden(side)} title={t("pane.hidden")}>
      {view.showHidden ? "◉" : "◎"}
    </button>
    <button type="button" onclick={() => reload(side)} title={t("pane.reload")}>⟳</button>
  </header>

  <div class="path">
    <button type="button" class="up" onclick={() => goUp(side)} title={t("pane.up")}>↑</button>
    <span class="mono current" title={view.path}>{view.path || "—"}</span>
    {#if view.busy}<span class="busy">{t("pane.loading")}</span>{/if}
    <span class="count">{t("pane.count", { count: rows.length })}</span>
  </div>

  {#if view.failure}
    <p class="failure">{describe(view.failure)}</p>
  {/if}

  <div class="body">
    {#if view.showTree}
      <FolderTree {side} />
    {/if}
    <FileList {side} onenter={open} />
  </div>
</section>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    min-height: 0;
    background: var(--surface-1);
    border-top: 2px solid transparent;
  }

  .pane.active {
    border-top-color: var(--accent);
  }

  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 8px;
    background: var(--surface-2);
    border-bottom: 1px solid var(--border);
  }

  .label {
    font-size: 0.66rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-faint);
    font-weight: 600;
  }

  .where {
    font-size: 0.74rem;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .spacer {
    flex: 1;
  }

  header button,
  .up {
    font: inherit;
    border: none;
    background: none;
    color: var(--text-faint);
    cursor: default;
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 0.86rem;
  }

  header button:hover,
  .up:hover {
    background: var(--surface-3);
    color: var(--accent);
  }

  header button.on {
    color: var(--accent);
  }

  .path {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 8px;
    border-bottom: 1px solid var(--border);
  }

  .current {
    flex: 1;
    font-size: 0.76rem;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }

  .busy,
  .count {
    font-size: 0.7rem;
    color: var(--text-faint);
    white-space: nowrap;
  }

  .failure {
    margin: 0;
    padding: 6px 10px;
    font-size: 0.78rem;
    color: var(--danger);
    background: var(--danger-soft);
    border-bottom: 1px solid var(--border);
  }

  .body {
    flex: 1;
    display: flex;
    min-height: 0;
    overflow: hidden;
  }
</style>
