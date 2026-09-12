<script lang="ts">
  /**
   * AmberBeam's own editor, for a file that lives on a server.
   *
   * Deliberately small. It is here so that changing a line in a config file
   * does not mean installing something else, not to compete with the editor
   * somebody already has — which is what the table of file types is for.
   *
   * What it has to get right is not the editing. It is that saving means the
   * file on the server changed, and that a save which cannot go through says
   * so instead of leaving somebody to find out later.
   */
  import { api, type Edit } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import { label } from "../keys/schemes";
  import { describe } from "./errors";

  interface Props {
    id: string;
    /** What to do when there is nothing left to show. */
    onclose: () => void;
  }

  let { id, onclose }: Props = $props();

  let edit = $state<Edit | null>(null);
  let text = $state("");
  /** What the copy held when it was last read or written. */
  let saved = $state("");
  let busy = $state(true);
  let failure = $state<unknown>(null);
  /** The server's file moved on; the answer decides whether to write anyway. */
  let asking = $state(false);
  /** Closing with typing that was never saved, waiting to be confirmed. */
  let leaving = $state(false);
  let box = $state<HTMLTextAreaElement | null>(null);
  let gutter = $state<HTMLDivElement | null>(null);

  let dirty = $derived(text !== saved);
  let lines = $derived(text.split("\n").length);

  $effect(() => {
    void open();
  });

  async function open(): Promise<void> {
    busy = true;
    failure = null;
    try {
      const all = await api.openEdits();
      const found = all.find((one) => one.id === id) ?? null;
      if (!found) {
        // Nothing to edit: the copy was ended elsewhere, or this window
        // outlived the program that opened it.
        onclose();
        return;
      }
      edit = found;
      const content = await api.editText(id);
      text = content;
      saved = content;
    } catch (why) {
      failure = why;
    } finally {
      busy = false;
    }
  }

  async function save(anyway = false): Promise<void> {
    if (busy) return;
    busy = true;
    failure = null;
    const sending = text;
    try {
      // The copy is written even when the server refuses, so the typing is
      // never the price of asking a question about it.
      edit = anyway ? await api.pushEdit(id, true) : await api.saveEdit(id, sending);
      saved = sending;
      asking = false;
    } catch (why) {
      if (kindOf(why) === "edit-changed-on-server") {
        // Saved in the copy already; only the write-back stopped.
        saved = sending;
        asking = true;
      } else {
        failure = why;
      }
    } finally {
      busy = false;
    }
  }

  function kindOf(failure: unknown): string | null {
    const error = failure as { kind?: string } | null;
    return error && typeof error === "object" ? (error.kind ?? null) : null;
  }

  /**
   * Closing means this file is done with, and the copy goes.
   *
   * The one place where this window knows more than the core does: nobody else
   * can tell whether somebody has finished, and here somebody said so by
   * closing the window they were typing in.
   *
   * Unsaved typing is asked about in the window rather than through the
   * browser's own confirm box. Every other question in this program is a piece
   * of this interface, and a system box in the middle of them reads as
   * something else talking.
   */
  async function close(force = false): Promise<void> {
    if (dirty && !force) {
      leaving = true;
      return;
    }
    try {
      await api.endEdit(id, true);
    } catch {
      // A copy that could not be removed is not a reason to keep a window open.
    }
    onclose();
  }

  // --- Finding --------------------------------------------------------------

  let finding = $state(false);
  let needle = $state("");
  let field = $state<HTMLInputElement | null>(null);

  function find(backwards = false): void {
    if (!box || needle === "") return;
    const hay = text.toLowerCase();
    const want = needle.toLowerCase();
    const from = backwards ? box.selectionStart : box.selectionEnd;

    // Wrapping rather than stopping, and in both directions. A search that
    // silently does nothing at the end of a file reads as a broken search.
    let at = backwards ? hay.lastIndexOf(want, from - want.length - 1) : hay.indexOf(want, from);
    if (at === -1) at = backwards ? hay.lastIndexOf(want) : hay.indexOf(want);
    if (at === -1) return;

    box.focus();
    box.setSelectionRange(at, at + needle.length);
    scrollToCursor(at);
  }

  /** Puts the line the match is on into view, which selecting alone does not. */
  function scrollToCursor(at: number): void {
    if (!box) return;
    const before = text.slice(0, at).split("\n").length - 1;
    const height = box.scrollHeight / Math.max(lines, 1);
    const wanted = before * height - box.clientHeight / 2;
    box.scrollTop = Math.max(0, wanted);
    syncGutter();
  }

  function syncGutter(): void {
    if (gutter && box) gutter.scrollTop = box.scrollTop;
  }

  function onKey(event: KeyboardEvent): void {
    const mod = event.metaKey || event.ctrlKey;
    if (mod && event.key.toLowerCase() === "s") {
      event.preventDefault();
      void save();
      return;
    }
    if (mod && event.key.toLowerCase() === "f") {
      event.preventDefault();
      finding = true;
      queueMicrotask(() => field?.select());
      return;
    }
    if (event.key === "Escape" && finding) {
      event.preventDefault();
      finding = false;
      box?.focus();
      return;
    }
    // A tab in a text file is a tab. Letting it move the focus out of the box
    // is how an editor loses somebody's place mid-line.
    if (event.key === "Tab" && event.target === box && box) {
      event.preventDefault();
      const at = box.selectionStart;
      const end = box.selectionEnd;
      text = `${text.slice(0, at)}\t${text.slice(end)}`;
      queueMicrotask(() => box?.setSelectionRange(at + 1, at + 1));
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="editor">
  <header>
    <div class="who">
      <span class="name">{edit?.name ?? ""}{dirty ? " •" : ""}</span>
      <span class="where mono">{edit?.remotePath ?? ""}</span>
    </div>
    <span class="encoding mono">{edit ? t(`editor.encoding.${edit.encoding}`) : ""}</span>
    <button type="button" class="save" disabled={busy || !dirty} onclick={() => void save()}>
      {t("editor.save")}
      <span class="shortcut mono">{label("Mod+S")}</span>
    </button>
    <button
      type="button"
      class="close"
      onclick={() => void close(false)}
      title={t("action.close")}
      aria-label={t("action.close")}
    >×</button>
  </header>

  {#if finding}
    <div class="find">
      <input
        bind:this={field}
        bind:value={needle}
        placeholder={t("editor.find")}
        spellcheck="false"
        autocapitalize="off"
        autocorrect="off"
        onkeydown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            find(event.shiftKey);
          }
        }}
      />
      <button type="button" onclick={() => find(true)}>{t("editor.find.back")}</button>
      <button type="button" onclick={() => find(false)}>{t("editor.find.on")}</button>
      <button
        type="button"
        class="drop"
        onclick={() => (finding = false)}
        title={t("editor.find.close")}
        aria-label={t("editor.find.close")}
      >×</button>
    </div>
  {/if}

  {#if failure}
    <p class="bad">{describe(failure)}</p>
  {/if}

  {#if asking}
    <div class="ask">
      <p>{t("editor.changed")}</p>
      <div class="choices">
        <button type="button" class="primary" onclick={() => void save(true)}>
          {t("editor.changed.overwrite")}
        </button>
        <button type="button" onclick={() => (asking = false)}>
          {t("editor.changed.leave")}
        </button>
      </div>
    </div>
  {/if}

  {#if leaving}
    <div class="ask">
      <p>{t("editor.unsaved")}</p>
      <div class="choices">
        <button type="button" class="primary" onclick={() => void save().then(() => close(true))}>
          {t("editor.unsaved.save")}
        </button>
        <button type="button" onclick={() => void close(true)}>{t("editor.unsaved.discard")}</button>
        <button type="button" onclick={() => (leaving = false)}>{t("action.cancel")}</button>
      </div>
    </div>
  {/if}

  <div class="sheet">
    <div class="numbers mono" bind:this={gutter} aria-hidden="true">
      {#each { length: lines } as _, line (line)}
        <div>{line + 1}</div>
      {/each}
    </div>
    <textarea
      class="mono"
      bind:this={box}
      bind:value={text}
      onscroll={syncGutter}
      spellcheck="false"
      autocapitalize="off"
      aria-label={edit?.name ?? ""}
    ></textarea>
  </div>

  <footer>
    <span>{t("editor.lines", { lines })}</span>
    <span class="grow"></span>
    <span>{dirty ? t("editor.unsaved.short") : t("editor.saved")}</span>
  </footer>
</div>

<style>
  .editor {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    background: var(--surface-0);
    color: var(--text);
  }

  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-1);
  }

  .who {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .name {
    font-size: 0.86rem;
    font-weight: 600;
  }

  .where {
    font-size: 0.7rem;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .encoding {
    font-size: 0.68rem;
    color: var(--text-faint);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .save {
    display: flex;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--accent);
    border-radius: 0.5rem;
    padding: 4px 12px;
    background: var(--accent-soft);
    color: var(--accent);
    font: inherit;
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
  }

  .save:disabled {
    border-color: var(--border);
    background: transparent;
    color: var(--text-faint);
    cursor: default;
  }

  .shortcut {
    font-size: 0.68rem;
    opacity: 0.7;
  }

  .close {
    border: none;
    background: none;
    font-size: 1.2rem;
    color: var(--text-faint);
    cursor: pointer;
    padding: 0 4px;
  }

  .find {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-1);
  }

  .find input {
    flex: 1;
    font: inherit;
    font-size: 0.8rem;
    padding: 4px 8px;
    border: 1px solid var(--border-strong);
    border-radius: 0.45rem;
    background: var(--surface-2);
    color: var(--text);
  }

  .find button,
  .ask button {
    border: 1px solid var(--border-strong);
    border-radius: 0.45rem;
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-size: 0.78rem;
    padding: 4px 10px;
    cursor: pointer;
  }

  .find .drop {
    border: none;
    font-size: 1rem;
    padding: 0 6px;
  }

  .bad {
    margin: 0;
    padding: 8px 12px;
    font-size: 0.78rem;
    color: var(--danger);
    background: var(--surface-1);
    border-bottom: 1px solid var(--border);
  }

  .ask {
    padding: 10px 12px;
    background: var(--surface-1);
    border-bottom: 1px solid var(--border);
  }

  .ask p {
    margin: 0 0 8px;
    font-size: 0.78rem;
    color: var(--text);
  }

  .choices {
    display: flex;
    gap: 8px;
  }

  .ask .primary {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }

  .sheet {
    flex: 1;
    display: flex;
    min-height: 0;
    overflow: hidden;
  }

  .numbers {
    overflow: hidden;
    padding: 10px 8px 10px 12px;
    text-align: right;
    font-size: 0.78rem;
    line-height: 1.5;
    color: var(--text-faint);
    background: var(--surface-1);
    border-right: 1px solid var(--border);
    user-select: none;
  }

  textarea {
    flex: 1;
    border: none;
    outline: none;
    resize: none;
    padding: 10px 12px;
    font-size: 0.78rem;
    line-height: 1.5;
    background: var(--surface-0);
    color: var(--text);
    /* Wrapping would put the numbers out of step with the lines they count. */
    white-space: pre;
    overflow: auto;
  }

  footer {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 12px;
    border-top: 1px solid var(--border);
    background: var(--surface-1);
    font-size: 0.7rem;
    color: var(--text-faint);
  }

  .grow {
    flex: 1;
  }
</style>
