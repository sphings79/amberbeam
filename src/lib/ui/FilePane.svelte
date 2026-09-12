<script lang="ts">
  import { api, LOCAL, type DirEntry, type Measurement, type SearchResult } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import {
    clearRequest,
    enter,
    focusedSide,
    focusPane,
    goUp,
    pane,
    navigate,
    reload,
    setFilter,
    setFiltering,
    startRename,
    stopRename,
    targets,
    toggleHidden,
    browsingTogether,
    setBrowsingTogether,
    isUp,
    setTreeWidth,
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
  import Splitter from "./Splitter.svelte";
  import Icon from "./Icon.svelte";
  import PermissionsDialog from "./PermissionsDialog.svelte";

  interface Props {
    side: Side;
    /** Where the button is, so the menu can open under it. */
    onconnect: (at: { x: number; y: number }) => void;
    onservers: () => void;
    ondisconnect: () => void;
    /** Sends the named entries to the other pane. */
    ontransfer: (names: string[], held?: boolean) => Promise<void>;
    /** Opens a file of this pane's endpoint for editing where it lies. */
    onedit: (path: string) => Promise<void>;
    /** Entries dragged here from the other pane. */
    onreceive: (from: Side, names: string[]) => Promise<void>;
  }

  let { side, onconnect, onservers, ondisconnect, ontransfer, onedit, onreceive }: Props =
    $props();

  /** Set while something is being dragged over this pane. */
  let dropTarget = $state(false);

  /** How many of a dropped batch are still on their way, for showing. */
  let arriving = $state(0);

  /**
   * Whether the window and the files are on different computers.
   *
   * True in a browser looking at a service, false in the desktop program,
   * where the local side is the very disk the window is drawn on.
   */
  let away = $derived(api.shell === "web");

  /**
   * Files dragged in from the computer the browser is running on.
   *
   * Only where that is a different computer from the one holding the files.
   * In the desktop program the two are the same and a drop is already handled
   * by the shell, with real paths rather than copies.
   *
   * One at a time on purpose. Ten at once would be ten sockets competing for
   * the same line, and a browser that decides for itself how many of them to
   * actually start.
   */
  async function receiveFiles(files: File[]): Promise<void> {
    if (remote) {
      // A server is not somewhere a browser can put a file directly. Getting
      // it there is two steps and the queue is the second.
      trouble = { kind: "other", detail: t("upload.local-only") };
      return;
    }
    arriving = files.length;
    try {
      for (const file of files) {
        await api.uploadInto(view.endpoint, view.path, file);
        arriving -= 1;
      }
      await reload(side);
    } catch (problem) {
      trouble = problem;
    } finally {
      arriving = 0;
    }
  }

  let filterField = $state<HTMLInputElement | null>(null);
  /**
   * What is in the address field while somebody is editing it.
   *
   * Null means nobody is: the field then shows where the pane actually is, and
   * follows it when it moves. Holding the text the whole time would mean a
   * pane that navigated by some other route — a double click, the tree, the
   * up arrow — left a stale path sitting in the box.
   */
  let editingPath = $state<string | null>(null);

  /** What a recursive search turned up, or null when none has run. */
  let found = $state<SearchResult | null>(null);
  let searching = $state(false);

  // The cursor goes into the field when the key opens it, and the results of a
  // previous search go away — they belong to what was typed then, not now.
  $effect(() => {
    if (view.filtering) queueMicrotask(() => filterField?.focus());
    else found = null;
  });

  /**
   * Looks through the whole tree below this directory.
   *
   * Separate from the filter above it on purpose: filtering costs nothing and
   * happens as one types, while this reads every directory underneath — over
   * FTP each of those is its own data connection. So it happens when asked and
   * says afterwards what it cost.
   */
  async function searchDeep(): Promise<void> {
    const needle = view.filter.trim();
    if (!needle || searching) return;
    searching = true;
    trouble = null;
    try {
      found = await api.search(view.endpoint, view.path, needle, 500);
    } catch (failure) {
      trouble = failure;
    } finally {
      searching = false;
    }
  }

  /** Goes to where a match sits, and stops searching. */
  async function goTo(match: { path: string; kind: string }): Promise<void> {
    const target =
      match.kind === "directory" ? match.path : await api.parentOf(view.endpoint, match.path);
    if (!target) return;
    setFiltering(side, false);
    await navigate(side, target);
  }

  let view = $derived(pane(side));

  // A key was pressed for one of this pane's commands. Cleared first, so a
  // command that opens a dialog does not run again the moment it closes.
  $effect(() => {
    const wanted = view.requested;
    if (!wanted) return;
    clearRequest(side);
    const command = COMMANDS.find((candidate) => candidate.id === wanted);
    if (command) void invoke(command);
  });
  let active = $derived(focusedSide() === side);
  let remote = $derived(view.endpoint !== LOCAL);
  let rows = $derived(visibleEntries(side));
  /**
   * How many things are actually in this directory.
   *
   * The way up is a row, not an entry. Counting it would mean an empty folder
   * announcing one item, which is the sort of small lie that makes somebody
   * distrust the rest of the numbers.
   */
  let howMany = $derived(rows.filter((entry) => !isUp(entry)).length);
  let chosen = $derived(targets(side));

  let menu = $state<{ x: number; y: number } | null>(null);
  let deleting = $state<DirEntry[] | null>(null);
  let measured = $state<Measurement | null>(null);
  let permissions = $state<DirEntry[] | null>(null);
  /** Something an operation refused, shown above the list. */
  let trouble = $state<unknown>(null);

  let menuItems = $derived(
    menuFor({ remote, targets: chosen, away }).map((command) => {
      const { usable, reason } = availability(command, { remote, targets: chosen, away });
      return { command, usable, reason };
    }),
  );

  let toolbar = $derived(
    COMMANDS.filter((command) => command.inToolbar).map((command) => {
      const { usable, reason } = availability(command, { remote, targets: chosen, away });
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
        await api.removeEntry(view.endpoint, path, view.siteId);
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
      case "enqueue":
        // The same journey, lined up rather than set going.
        await ontransfer(
          chosen.map((entry) => entry.name),
          true,
        );
        break;
      case "download":
        for (const entry of chosen) {
          if (entry.kind === "directory") continue;
          const path = await api.joinPath(view.endpoint, view.path, entry.name);
          const where = api.downloadUrl(view.endpoint, path);
          // An anchor rather than changing the address: a page that navigates
          // away to fetch a file is a page that has to come back, and coming
          // back means signing in and starting over.
          if (!where) continue;
          const link = document.createElement("a");
          link.href = where;
          link.download = entry.name;
          document.body.appendChild(link);
          link.click();
          link.remove();
        }
        break;
      case "edit-remote": {
        const entry = chosen[0];
        if (!entry || entry.kind === "directory") break;
        await run(async () => {
          const path = await api.joinPath(view.endpoint, view.path, entry.name);
          await onedit(path);
        });
        break;
      }
      default:
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
    if (!payload) {
      // Files from the viewer's own computer, in a browser. The desktop shell
      // never sees these — it takes a drag from outside the window itself, so
      // it can hand over real paths instead of copies.
      const dropped = [...(event.dataTransfer?.files ?? [])];
      if (dropped.length > 0) await receiveFiles(dropped);
      return;
    }
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
      <button
        type="button"
        data-connect={side}
        onclick={(event) => {
          const box = event.currentTarget.getBoundingClientRect();
          onconnect({ x: box.left, y: box.bottom + 4 });
        }}
        title={t("pane.connect")}
      >
        <Icon name="connect" />
      </button>
    {/if}
    <button type="button" onclick={onservers} title={t("sites.title")}>
      <Icon name="sites" />
    </button>
    <!-- One switch for a relation between two panes, shown in both of their
         headers. Two switches that could disagree would be a state nobody
         could describe. -->
    <button
      type="button"
      class:on={browsingTogether()}
      onclick={() => setBrowsingTogether(!browsingTogether())}
      title={t("pane.together")}
    >
      <Icon name="together" />
    </button>
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
        title={item.command.notYet
          ? t("cmd.not-yet")
          : t(item.command.key)}
        onclick={() => invoke(item.command)}
      >
        <Icon name={item.command.icon} />
        <span class="sr">{t(item.command.key)}</span>
      </button>
    {/each}
  </div>

  {#if view.filtering}
    <div class="filter">
      <input
        bind:this={filterField}
        value={view.filter}
        oninput={(event) => setFilter(side, event.currentTarget.value)}
        onkeydown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            setFiltering(side, false);
          } else if (event.key === "Enter") {
            event.preventDefault();
            void searchDeep();
          }
        }}
        placeholder={t("filter.placeholder")}
        autocomplete="off"
        autocapitalize="off"
        autocorrect="off"
        spellcheck="false"
      />
      <button
        type="button"
        onclick={() => void searchDeep()}
        disabled={searching || view.filter.trim() === ""}
        title={t("filter.deep.hint")}
      >
        {searching ? t("filter.searching") : t("filter.deep")}
      </button>
      <button
        type="button"
        class="quiet"
        onclick={() => setFiltering(side, false)}
        title={t("filter.close")}
        aria-label={t("filter.close")}
      >×</button>
    </div>

    {#if found}
      <div class="found">
        <div class="summary">
          {found.matches.length === 0
            ? t("filter.none", { directories: found.directories })
            : found.matches.length === 1
              ? t("filter.count.one", { directories: found.directories })
              : t("filter.count", {
                  count: found.matches.length,
                  directories: found.directories,
                })}
          {#if found.truncated}
            <span class="warn">{t("filter.truncated")}</span>
          {/if}
        </div>
        {#each found.matches as match (match.path)}
          <button type="button" class="match" onclick={() => void goTo(match)}>
            <span class="mono">{match.path}</span>
          </button>
        {/each}
      </div>
    {/if}
  {/if}

  <div class="path">
    <button type="button" class="up" onclick={() => goUp(side)} title={t("pane.up")}>↑</button>
    <!--
      A field rather than a label, so a path can be pasted in. It shows where
      the pane is until somebody starts editing, and goes back to showing that
      the moment they give up — a box holding a path the pane is not at is a
      box that lies about where you are.

      Nothing happens while it is being typed in. Navigating on every keystroke
      would walk off to "/v", "/va", "/var" on the way to anywhere, and on a
      remote that is three round trips nobody asked for.
    -->
    <input
      class="mono current"
      value={editingPath ?? view.path}
      title={view.path}
      placeholder={t("pane.path.placeholder")}
      spellcheck="false"
      autocomplete="off"
      autocapitalize="off"
      autocorrect="off"
      oninput={(event) => (editingPath = event.currentTarget.value)}
      onfocus={(event) => event.currentTarget.select()}
      onblur={() => (editingPath = null)}
      onkeydown={(event) => {
        if (event.key === "Enter") {
          event.preventDefault();
          const wanted = (editingPath ?? "").trim();
          editingPath = null;
          event.currentTarget.blur();
          if (wanted !== "" && wanted !== view.path) void navigate(side, wanted);
        } else if (event.key === "Escape") {
          event.preventDefault();
          editingPath = null;
          event.currentTarget.blur();
        }
      }}
    />
    {#if view.busy}<span class="busy">{t("pane.loading")}</span>{/if}
    {#if arriving > 0}<span class="busy">{t("upload.arriving", { count: arriving })}</span>{/if}
    <!-- "1 Einträge" is the sort of thing that makes a program feel machine
         translated, and it takes one key to avoid. -->
    <span class="count">
      {howMany === 1 ? t("pane.count.one") : t("pane.count", { count: howMany })}
    </span>
  </div>

  {#if view.failure}
    <p class="failure">{describe(view.failure)}</p>
  {/if}
  {#if trouble}
    <p class="failure">{describe(trouble)}</p>
  {/if}

  <div class="body">
    {#if view.showTree}
      <div class="tree" style:width="{view.treeWidth}px">
        <FolderTree {side} />
      </div>
      <Splitter
        direction="vertical"
        label={t("splitter.tree")}
        onmove={(delta) => setTreeWidth(side, view.treeWidth + delta)}
      />
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

  .filter {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 8px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-1);
  }

  .filter input {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-size: 0.8rem;
    padding: 4px 8px;
    border-radius: 0.4rem;
    border: 1px solid var(--border-strong);
    background: var(--surface-0);
    color: var(--text);
  }

  .filter button {
    font: inherit;
    font-size: 0.76rem;
    padding: 4px 10px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    background: var(--surface-1);
    color: var(--text);
    cursor: default;
  }

  .filter button:disabled {
    opacity: 0.4;
  }

  .filter button.quiet {
    border: none;
    color: var(--text-faint);
    padding: 2px 6px;
  }

  .found {
    max-height: 180px;
    overflow: auto;
    border-bottom: 1px solid var(--border);
    background: var(--surface-0);
  }

  .summary {
    padding: 4px 10px;
    font-size: 0.74rem;
    color: var(--text-faint);
  }

  .summary .warn {
    color: var(--warn);
  }

  .match {
    display: block;
    width: 100%;
    text-align: left;
    font: inherit;
    font-size: 0.78rem;
    padding: 3px 10px;
    border: none;
    background: none;
    color: var(--text-muted);
    cursor: default;
  }

  .match:hover {
    background: var(--surface-2);
    color: var(--text);
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

  /* Looks like the label it replaced until somebody puts the cursor in it.
     A box drawn around the path all day would turn the place you are into a
     form to be filled in. */
  .current {
    flex: 1;
    min-width: 0;
    font-size: 0.76rem;
    color: var(--text-muted);
    border: 1px solid transparent;
    border-radius: 0.35rem;
    padding: 1px 5px;
    background: transparent;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    /* No right-to-left trick to cut long paths from the front: it moves the
       leading slash to the end, so /Users/sphings reads as Users/sphings/. A
       path that lies about its own shape is worse than one that is cut off. */
  }

  .current:hover {
    border-color: var(--border);
  }

  .current:focus {
    border-color: var(--accent);
    background: var(--surface-1);
    color: var(--text);
    outline: none;
    text-overflow: clip;
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

  /* The width lives here rather than in FolderTree, because it is now a thing
     somebody drags rather than a number the component decided for itself. */
  .tree {
    flex: none;
    min-width: 0;
    display: flex;
  }
</style>
