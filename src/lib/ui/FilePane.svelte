<script lang="ts">
  import { api, LOCAL, type DirEntry, type Measurement } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import {
    enter,
    focusedSide,
    focusPane,
    goUp,
    pane,
    reload,
    startRename,
    stopRename,
    targets,
    toggleHidden,
    toggleTree,
    visibleEntries,
    type Side,
  } from "../state/panes.svelte";
  import { availability, COMMANDS, menuFor, type Command } from "./commands";
  import ContextMenu from "./ContextMenu.svelte";
  import DeleteDialog from "./DeleteDialog.svelte";
  import { describe } from "./errors";
  import FileList from "./FileList.svelte";
  import FolderTree from "./FolderTree.svelte";
  import Icon from "./Icon.svelte";
  import PermissionsDialog from "./PermissionsDialog.svelte";

  interface Props {
    side: Side;
    onquickconnect: () => void;
    ondisconnect: () => void;
    /** Sends the named entries to the other pane. */
    ontransfer: (names: string[]) => Promise<void>;
    /** Entries dragged here from the other pane. */
    onreceive: (from: Side, names: string[]) => Promise<void>;
  }

  let { side, onquickconnect, ondisconnect, ontransfer, onreceive }: Props = $props();

  /** Set while something is being dragged over this pane. */
  let dropTarget = $state(false);

  let view = $derived(pane(side));
  let active = $derived(focusedSide() === side);
  let remote = $derived(view.endpoint !== LOCAL);
  let rows = $derived(visibleEntries(side));
  let chosen = $derived(targets(side));

  let menu = $state<{ x: number; y: number } | null>(null);
  let deleting = $state<DirEntry[] | null>(null);
  let measured = $state<Measurement | null>(null);
  let permissions = $state<DirEntry[] | null>(null);
  /** Something an operation refused, shown above the list. */
  let trouble = $state<unknown>(null);

  let menuItems = $derived(
    menuFor({ remote, targets: chosen }).map((command) => {
      const { usable, reason } = availability(command, { remote, targets: chosen });
      return { command, usable, reason };
    }),
  );

  let toolbar = $derived(
    COMMANDS.filter((command) => command.inToolbar).map((command) => {
      const { usable, reason } = availability(command, { remote, targets: chosen });
      return { command, usable, reason };
    }),
  );

  async function open(entry: DirEntry): Promise<void> {
    await enter(side, entry);
  }

  /** Asks for a name, then makes the thing. */
  async function make(kind: "dir" | "file"): Promise<void> {
    const name = window.prompt(kind === "dir" ? t("cmd.new-folder.ask") : t("cmd.new-file.ask"));
    if (!name) return;
    await run(async () => {
      if (kind === "dir") {
        await api.createDir(view.endpoint, view.path, name);
      } else {
        await api.createFile(view.endpoint, view.path, name);
      }
      await reload(side);
    });
  }

  async function rename(entry: DirEntry, to: string): Promise<void> {
    stopRename(side);
    await run(async () => {
      await api.renameEntry(view.endpoint, view.path, entry.name, to);
      await reload(side);
    });
  }

  async function askDelete(): Promise<void> {
    if (chosen.length === 0) return;
    deleting = chosen;
    measured = null;
    // Counted before anything is removed, so the warning can say how much.
    const totals: Measurement = {
      files: 0,
      directories: 0,
      symlinks: 0,
      bytes: 0,
      truncated: false,
    };
    for (const entry of chosen) {
      try {
        const path = await api.joinPath(view.endpoint, view.path, entry.name);
        const part = await api.measure(view.endpoint, path);
        totals.files += part.files;
        totals.directories += part.directories;
        totals.symlinks += part.symlinks;
        totals.bytes += part.bytes;
        totals.truncated ||= part.truncated;
      } catch {
        // A row that cannot be counted is still going to be attempted; the
        // warning simply cannot include it.
        totals.truncated = true;
      }
    }
    measured = totals;
  }

  async function confirmDelete(): Promise<void> {
    const entries = deleting ?? [];
    deleting = null;
    await run(async () => {
      for (const entry of entries) {
        const path = await api.joinPath(view.endpoint, view.path, entry.name);
        await api.removeEntry(view.endpoint, path);
      }
      await reload(side);
    });
  }

  async function applyPermissions(mode: number, recursive: boolean): Promise<void> {
    const entries = permissions ?? [];
    permissions = null;
    await run(async () => {
      for (const entry of entries) {
        const path = await api.joinPath(view.endpoint, view.path, entry.name);
        await api.setPermissions(view.endpoint, path, mode, recursive);
      }
      await reload(side);
    });
  }

  async function run(work: () => Promise<void>): Promise<void> {
    trouble = null;
    try {
      await work();
    } catch (failure) {
      trouble = failure;
    }
  }

  async function invoke(command: Command): Promise<void> {
    menu = null;
    switch (command.id) {
      case "reload":
        await reload(side);
        break;
      case "new-folder":
        await make("dir");
        break;
      case "new-file":
        await make("file");
        break;
      case "rename":
        if (chosen[0]) startRename(side, chosen[0].name);
        break;
      case "permissions":
        if (chosen.length > 0) permissions = chosen;
        break;
      case "delete":
        await askDelete();
        break;
      case "transfer":
        await ontransfer(chosen.map((entry) => entry.name));
        break;
      default:
        // transfer and remote editing arrive with the transfer engine.
        break;
    }
  }

  function openMenu(_entry: DirEntry | null, x: number, y: number): void {
    focusPane(side);
    menu = { x, y };
  }
</script>

<section
  class="pane"
  class:active
  class:dropTarget
  onpointerdown={() => focusPane(side)}
  ondragover={(event) => {
    // Both kinds of drag land here: rows from the other pane, and files from
    // the Finder. Which one it is only matters on drop.
    event.preventDefault();
    dropTarget = true;
  }}
  ondragleave={(event) => {
    if (event.target === event.currentTarget) dropTarget = false;
  }}
  ondrop={async (event) => {
    event.preventDefault();
    dropTarget = false;
    // A drag from the other pane carries which pane it came from and what was
    // held. A drag from outside the window never reaches here — the desktop
    // shell takes those so it can hand over real paths.
    const payload = event.dataTransfer?.getData("application/x-amberbeam");
    if (!payload) return;
    try {
      const { side: from, names } = JSON.parse(payload) as { side: Side; names: string[] };
      if (from !== side) {
        await onreceive(from, names);
      }
    } catch {
      // Something else was dropped. Nothing to do, and nothing to complain
      // about either.
    }
  }}
  data-side={side}
  aria-label={remote ? t("pane.server") : t("pane.local")}
>
  <header>
    <span class="label">{remote ? t("pane.server") : t("pane.local")}</span>
    <span class="where mono" title={view.title ?? ""}>
      {view.title ?? t("pane.this-machine")}
    </span>
    {#if view.protocol === "ftp"}
      <span class="mark bad" title={t("connection.unencrypted.title")}>
        {t("connection.unencrypted")}
      </span>
    {:else if view.certificateAccepted}
      <span class="mark warn" title={t("connection.exception.title")}>
        {t("connection.exception")}
      </span>
    {/if}
    <span class="spacer"></span>
    {#if remote}
      <button type="button" onclick={ondisconnect} title={t("pane.disconnect")}>
        <Icon name="disconnect" />
      </button>
    {:else}
      <button type="button" onclick={onquickconnect} title={t("pane.connect")}>
        <Icon name="connect" />
      </button>
    {/if}
    <button
      type="button"
      class:on={view.showTree}
      onclick={() => toggleTree(side)}
      title={t("pane.tree.toggle")}
    >
      <Icon name="tree" />
    </button>
    <button
      type="button"
      class:on={view.showHidden}
      onclick={() => toggleHidden(side)}
      title={t("pane.hidden")}
    >
      <Icon name={view.showHidden ? "hidden" : "hidden-off"} />
    </button>
  </header>

  <div class="tools">
    {#each toolbar as item (item.command.id)}
      <button
        type="button"
        disabled={!item.usable}
        title={item.command.comingIn
          ? t("cmd.coming", { milestone: item.command.comingIn })
          : t(item.command.key)}
        onclick={() => invoke(item.command)}
      >
        <Icon name={item.command.icon} />
        <span class="sr">{t(item.command.key)}</span>
      </button>
    {/each}
  </div>

  <div class="path">
    <button type="button" class="up" onclick={() => goUp(side)} title={t("pane.up")}>↑</button>
    <span class="mono current" title={view.path}>{view.path || "—"}</span>
    {#if view.busy}<span class="busy">{t("pane.loading")}</span>{/if}
    <span class="count">{t("pane.count", { count: rows.length })}</span>
  </div>

  {#if view.failure}
    <p class="failure">{describe(view.failure)}</p>
  {/if}
  {#if trouble}
    <p class="failure">{describe(trouble)}</p>
  {/if}

  <div class="body">
    {#if view.showTree}
      <FolderTree {side} />
    {/if}
    <FileList {side} onenter={open} oncontext={openMenu} onrename={rename} />
  </div>
</section>

{#if menu}
  <ContextMenu
    x={menu.x}
    y={menu.y}
    items={menuItems}
    onpick={invoke}
    onclose={() => (menu = null)}
  />
{/if}

{#if deleting}
  <DeleteDialog
    entries={deleting}
    {measured}
    onconfirm={confirmDelete}
    oncancel={() => ((deleting = null), (measured = null))}
  />
{/if}

{#if permissions}
  <PermissionsDialog
    entries={permissions}
    initialMode={permissions[0]?.permissions ?? 0o644}
    hasDirectory={permissions.some((entry) => entry.kind === "directory")}
    onapply={applyPermissions}
    oncancel={() => (permissions = null)}
  />
{/if}

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

  .pane.dropTarget {
    box-shadow: inset 0 0 0 2px var(--accent);
  }

  header,
  .tools {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 8px;
    background: var(--surface-2);
    border-bottom: 1px solid var(--border);
  }

  .tools {
    gap: 2px;
    background: var(--surface-1);
  }

  .mark {
    font-size: 0.68rem;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    padding: 2px 8px;
    border-radius: 999px;
    white-space: nowrap;
  }

  .mark.bad {
    color: var(--danger);
    background: var(--danger-soft);
  }

  .mark.warn {
    color: var(--warn);
    background: var(--warn-soft);
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
  .tools button,
  .up {
    font: inherit;
    display: flex;
    align-items: center;
    border: none;
    background: none;
    color: var(--text-faint);
    cursor: default;
    padding: 3px 6px;
    border-radius: 4px;
    font-size: 0.86rem;
  }

  header button:hover,
  .tools button:hover:not(:disabled),
  .up:hover {
    background: var(--surface-3);
    color: var(--accent);
  }

  .tools button:disabled {
    opacity: 0.35;
  }

  header button.on {
    color: var(--accent);
  }

  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
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
    /* No right-to-left trick to cut long paths from the front: it moves the
       leading slash to the end, so /Users/sphings reads as Users/sphings/. A
       path that lies about its own shape is worse than one that is cut off. */
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
