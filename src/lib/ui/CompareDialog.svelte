<script lang="ts">
  /**
   * Comparing the two sides, and deciding what to do about it.
   *
   * Two halves in one window. First what to compare and how — asked before
   * anything runs, because a recursive walk of two trees is many listings and
   * a checksum pass reads every file on both sides. Then what was found, with
   * ticks, because a comparison that acted on its own findings would be a
   * program deciding for you what "the same" means.
   */
  import { untrack } from "svelte";

  import { api, type Comparison, type CoreEvent, type How } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import Switch from "./Switch.svelte";
  import { pane, type Side } from "../state/panes.svelte";
  import { describe } from "./errors";
  import { formatSize } from "./format";
  import Icon from "./Icon.svelte";
  import { trap } from "./trap";

  interface Props {
    /** The side the files would travel from. */
    from: Side;
    onclose: () => void;
    /**
     * Puts the chosen entries into the queue, held — and removes the ones
     * that were ticked for deletion, which only happens when the settings
     * allow it at all.
     */
    onsend: (
      jobs: { directory: string; names: string[] }[],
      remove: string[],
      into: string,
    ) => Promise<void>;
  }

  let { from, onclose, onsend }: Props = $props();

  // Where it starts, not what it follows. The window is opened on the focused
  // side and the swap button is what changes it after that; tracking the prop
  // would turn a click in the other pane into a silent change of direction.
  let source = $state<Side>(untrack(() => from));
  let target = $derived<Side>(source === "left" ? "right" : "left");

  let recursive = $state(false);
  let how = $state<How>("size-and-time");
  /**
   * The patterns, taken from the server entry when one of the sides is a
   * saved server, and editable here for this one comparison.
   *
   * Filled rather than merely obeyed: somebody should be able to see what is
   * being skipped, and change it for one run without changing the entry.
   */
  let excludes = $state("");
  /** What the settings say about showing the list, and about deletions. */
  let review = $state(true);
  let deleteAlong = $state(false);

  $effect(() => {
    void (async () => {
      const settings = await api.settings().catch(() => null);
      if (settings) {
        review = settings.reviewComparison;
        deleteAlong = settings.deleteAlong;
      }
      // The entry's own list, from whichever side came from one.
      const id = pane(source).siteId ?? pane(target).siteId;
      if (!id) return;
      const site = (await api.sites().catch(() => [])).find((one) => one.id === id);
      if (site && site.excludes.length > 0) excludes = site.excludes.join(", ");
    })();
  });

  let running = $state(false);
  let progress = $state<{ directories: number; rows: number } | null>(null);
  let found = $state<Comparison | null>(null);
  let failure = $state<unknown>(null);
  /** Paths the person wants dealt with. Paths, not indexes: the list is
      rebuilt by every run and an index would point at a different row. */
  let ticked = $state(new Set<string>());

  /**
   * What a transfer would touch — and, when deletions are carried across, what
   * a deletion would touch as well.
   */
  let movableStates = $derived(
    deleteAlong ? ["only-here", "different", "only-there"] : ["only-here", "different"],
  );

  let movable = $derived((found?.rows ?? []).filter((row) => movableStates.includes(row.state)));
  let chosen = $derived(movable.filter((row) => ticked.has(row.path)));

  $effect(() => {
    let stop: (() => void) | undefined;
    void api
      .subscribe((event: CoreEvent) => {
        if (event.event === "comparing") {
          progress = { directories: event.directories, rows: event.rows };
        }
      })
      .then((off) => (stop = off));
    return () => stop?.();
  });

  async function run(): Promise<void> {
    running = true;
    failure = null;
    found = null;
    progress = { directories: 0, rows: 0 };
    try {
      const answer = await api.compare({
        hereEndpoint: pane(source).endpoint,
        herePath: pane(source).path,
        thereEndpoint: pane(target).endpoint,
        therePath: pane(target).path,
        recursive,
        how,
        excludes: excludes
          .split(/[\n,]+/)
          .map((one) => one.trim())
          .filter((one) => one !== ""),
      });
      found = answer;
      if (!review) {
        // The settings say not to show the list. Everything that differs goes
        // into the queue held, which is where it would have gone anyway —
        // held, so starting it is still somebody's decision.
        ticked = new Set(
          answer.rows.filter((row) => movableStates.includes(row.state)).map((r) => r.path),
        );
        await send();
        return;
      }
      // Everything a transfer would touch, ticked. It is what somebody asking
      // "what differs" is about to say yes to anyway, and unticking three
      // rows is less work than ticking ninety.
      ticked = new Set(answer.rows.filter((row) => movableStates.includes(row.state)).map((r) => r.path));
    } catch (why) {
      failure = why;
    } finally {
      running = false;
      progress = null;
    }
  }

  function toggle(path: string): void {
    const next = new Set(ticked);
    if (!next.delete(path)) next.add(path);
    ticked = next;
  }

  function tickAll(on: boolean): void {
    ticked = on ? new Set(movable.map((row) => row.path)) : new Set();
  }

  /**
   * The ticked rows as work for the queue.
   *
   * Two rules. A ticked directory carries everything below it — the queue
   * walks a directory by itself — so its descendants are dropped rather than
   * queued a second time. And rows are grouped by the directory they sit in,
   * because one job per row would mean a thousand round trips for a thousand
   * files.
   */
  function plan(): { directory: string; names: string[] }[] {
    const sending = chosen.filter((row) => row.state !== "only-there");
    const carried = sending.filter((row) => row.kind === "directory").map((row) => `${row.path}/`);
    const wanted = sending.filter((row) => !carried.some((under) => row.path.startsWith(under)));

    const byDirectory = new Map<string, string[]>();
    for (const row of wanted) {
      const at = row.path.lastIndexOf("/");
      const below = at === -1 ? "" : row.path.slice(0, at);
      const names = byDirectory.get(below) ?? [];
      names.push(row.name);
      byDirectory.set(below, names);
    }
    return [...byDirectory].map(([directory, names]) => ({ directory, names }));
  }

  /**
   * What was ticked for deletion, with anything inside a ticked directory
   * left out: removing the directory takes it along.
   */
  function toRemove(): string[] {
    const going = chosen.filter((row) => row.state === "only-there");
    const carried = going.filter((row) => row.kind === "directory").map((row) => `${row.path}/`);
    return going
      .filter((row) => !carried.some((under) => row.path.startsWith(under)))
      .map((row) => row.path);
  }

  async function send(): Promise<void> {
    running = true;
    try {
      await onsend(plan(), toRemove(), pane(target).path);
      onclose();
    } catch (why) {
      failure = why;
      running = false;
    }
  }
</script>

<div class="backdrop" role="presentation">
  <div class="dialog" use:trap role="dialog" aria-modal="true" aria-label={t("compare.title")}>
    <header>
      <h2>{t("compare.title")}</h2>
      <button
        type="button"
        class="close"
        onclick={onclose}
        title={t("action.close")}
        aria-label={t("action.close")}
      >×</button>
    </header>

    <div class="body">
      <div class="ends">
        <span class="end mono">{pane(source).title ?? t("pane.local")}: {pane(source).path}</span>
        <button
          type="button"
          class="swap"
          onclick={() => ((source = target), (found = null))}
          title={t("compare.swap")}
        >
          <Icon name="transfer" size={14} />
        </button>
        <span class="end mono">{pane(target).title ?? t("pane.local")}: {pane(target).path}</span>
      </div>

      {#if !found}
        <Switch
          bind:checked={recursive}
          label={t("compare.recursive")}
          hint={t("compare.recursive.hint")}
        />

        <label class="row">
          <span class="label">{t("compare.how")}</span>
          <select bind:value={how}>
            <option value="size">{t("compare.how.size")}</option>
            <option value="size-and-time">{t("compare.how.time")}</option>
            <option value="checksum">{t("compare.how.checksum")}</option>
          </select>
        </label>
        <p class="hint">{t(`compare.how.${how}.hint`)}</p>

        <label class="row grow">
          <span class="label">{t("compare.excludes")}</span>
          <input
            bind:value={excludes}
            placeholder=".git, node_modules, *.log"
            spellcheck="false"
            autocapitalize="off"
            autocorrect="off"
          />
        </label>
      {/if}

      {#if failure}
        <p class="bad">{describe(failure)}</p>
      {/if}

      {#if running && progress}
        <p class="hint">
          {t("compare.working", { directories: progress.directories, rows: progress.rows })}
        </p>
      {/if}

      {#if found}
        {#if found.cutShort}
          <p class="bad">{t("compare.cut-short")}</p>
        {/if}

        <div class="summary">
          <span>{t("compare.found", { rows: found.rows.length, directories: found.directories })}</span>
          <span class="gap"></span>
          <button type="button" class="plain" onclick={() => tickAll(true)}>
            {t("compare.all")}
          </button>
          <button type="button" class="plain" onclick={() => tickAll(false)}>
            {t("compare.none")}
          </button>
        </div>

        <div class="rows">
          {#each found.rows as row (row.path)}
            {@const canMove = movableStates.includes(row.state)}
            <div class="line" class:same={row.state === "same"}>
              <input
                type="checkbox"
                checked={ticked.has(row.path)}
                disabled={!canMove}
                onchange={() => toggle(row.path)}
                aria-label={row.path}
              />
              <span class="glyph" class:folder={row.kind === "directory"} aria-hidden="true">
                <Icon name={row.kind === "directory" ? "folder" : "file"} size={12} />
              </span>
              <span class="path mono">{row.path}</span>
              <span class="state {row.state}">{t(`compare.state.${row.state}`)}</span>
              <span class="size mono">
                {row.here?.size != null ? formatSize(row.here.size) : ""}
              </span>
            </div>
          {/each}
          {#if found.rows.length === 0}
            <p class="hint">{t("compare.nothing")}</p>
          {/if}
        </div>

        <p class="hint">
          {deleteAlong ? t("compare.only-there.deleting") : t("compare.only-there.hint")}
        </p>
      {/if}
    </div>

    <footer>
      {#if found}
        <button type="button" onclick={() => (found = null)}>{t("compare.again")}</button>
        <span class="gap"></span>
        <button
          type="button"
          class="primary"
          disabled={running || chosen.length === 0}
          onclick={() => void send()}
        >
          {toRemove().length === 0
            ? t("compare.send", { files: chosen.length })
            : t("compare.send.and-remove", {
                files: chosen.length - toRemove().length,
                gone: toRemove().length,
              })}
        </button>
      {:else}
        <span class="gap"></span>
        <button type="button" class="primary" disabled={running} onclick={() => void run()}>
          {running ? t("compare.working.short") : t("compare.run")}
        </button>
      {/if}
    </footer>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 45%);
    display: grid;
    place-items: center;
    z-index: 52;
  }

  .dialog {
    width: min(760px, 94vw);
    max-height: min(80vh, 700px);
    display: flex;
    flex-direction: column;
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-radius: 1rem;
    box-shadow: var(--shadow-lg);
  }

  header {
    display: flex;
    align-items: center;
    padding: 14px 18px;
    border-bottom: 1px solid var(--border);
  }

  h2 {
    margin: 0;
    flex: 1;
    font-size: 1rem;
  }

  .close {
    border: none;
    background: none;
    font-size: 1.2rem;
    color: var(--text-faint);
    cursor: pointer;
    padding: 0 6px;
  }

  .body {
    padding: 14px 18px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .ends {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-bottom: 10px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 8px;
  }

  .end {
    flex: 1;
    min-width: 0;
    font-size: 0.74rem;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .swap {
    border: 1px solid var(--border-strong);
    border-radius: 0.5rem;
    background: transparent;
    color: var(--accent);
    padding: 3px 8px;
    cursor: pointer;
    flex: none;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.84rem;
    margin-top: 8px;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-top: 10px;
  }

  .row.grow input {
    flex: 1;
  }

  .label {
    font-size: 0.84rem;
  }

  select,
  .row input {
    font: inherit;
    font-size: 0.82rem;
    padding: 4px 8px;
    border: 1px solid var(--border-strong);
    border-radius: 0.45rem;
    background: var(--surface-2);
    color: var(--text);
  }

  .summary {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
    font-size: 0.76rem;
    color: var(--text-muted);
  }

  .gap {
    flex: 1;
  }

  .plain {
    border: none;
    background: none;
    color: var(--accent);
    font: inherit;
    font-size: 0.76rem;
    cursor: pointer;
    padding: 2px 4px;
  }

  .rows {
    margin-top: 6px;
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    overflow: auto;
    max-height: 42vh;
  }

  .line {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 8px;
    font-size: 0.78rem;
  }

  .line + .line {
    border-top: 1px solid var(--border);
  }

  /* Rows nothing will happen to step back rather than disappear: a list that
     shows only differences cannot be checked against what is actually there. */
  .line.same {
    color: var(--text-faint);
  }

  .glyph {
    color: var(--text-faint);
    display: inline-flex;
    flex: none;
  }

  .glyph.folder {
    color: var(--accent);
  }

  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .state {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    flex: none;
  }

  .state.only-here {
    color: var(--accent);
  }

  .state.only-there {
    color: var(--text-faint);
  }

  .state.different {
    color: var(--warn);
  }

  .state.same {
    color: var(--text-faint);
  }

  .size {
    width: 68px;
    text-align: right;
    flex: none;
    color: var(--text-faint);
    font-size: 0.72rem;
  }

  .hint {
    margin: 2px 0 0;
    font-size: 0.74rem;
    color: var(--text-faint);
  }

  .bad {
    margin: 8px 0 0;
    font-size: 0.78rem;
    color: var(--danger);
  }

  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 18px;
    border-top: 1px solid var(--border);
  }

  footer button {
    border: 1px solid var(--border-strong);
    border-radius: 0.5rem;
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-size: 0.8rem;
    padding: 5px 14px;
    cursor: pointer;
  }

  footer .primary {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }

  footer button:disabled {
    border-color: var(--border);
    color: var(--text-faint);
    background: transparent;
    cursor: default;
  }
</style>
