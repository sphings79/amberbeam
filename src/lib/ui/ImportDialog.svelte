<script lang="ts">
  /**
   * Taking somebody else's server list over.
   *
   * Three steps, and the middle one is the point: look at what was found, tick
   * what to keep, and answer one plain question about the passwords before any
   * of it is written. An import that quietly takes thirty servers and drops
   * half their passwords is worse than one that asks.
   */
  import {
    api,
    type BundlePreview,
    type ImportCandidate,
    type ImportPreview,
    type ImportSource,
    type Unsubscribe,
  } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import { trap } from "./trap";
  import { describe } from "./errors";

  interface Props {
    /** The folder in the tree the import drops into. */
    into: string;
    onclose: () => void;
    ondone: (taken: number) => void;
  }

  let { into, onclose, ondone }: Props = $props();

  let candidates = $state<ImportCandidate[]>([]);
  let looking = $state(true);
  let preview = $state<ImportPreview | null>(null);
  /** An AmberBeam export of our own, which is read a different way. */
  let bundle = $state<BundlePreview | null>(null);
  let bundlePath = $state("");
  let passphrase = $state("");
  let chosen = $state(new Set<number>());
  let takePasswords = $state(false);
  let busy = $state(false);
  let failure = $state<unknown>(null);
  /** Set while a file is being dragged over the window. */
  let dropping = $state(false);

  $effect(() => {
    void api
      .importCandidates()
      .then((found) => (candidates = found))
      .catch((problem) => (failure = problem))
      .finally(() => (looking = false));
  });

  // A file dragged onto the window is read like one that was found. It is also
  // the only way in for a list that lives somewhere nobody would think to look.
  $effect(() => {
    let stop: Unsubscribe | undefined;
    void api
      .onFileDrop((paths) => {
        dropping = false;
        const path = paths[0];
        if (!path) return;
        if (isOurs(path)) void lookIntoBundle(path);
        else void look(guess(path), path);
      })
      .then((off) => (stop = off));
    return () => stop?.();
  });

  /** Our own export, told apart by its name before anything is read. */
  function isOurs(path: string): boolean {
    return path.toLowerCase().endsWith(".amberbeam-sites");
  }

  /** Which reader a dropped file wants, judged by its name. */
  function guess(path: string): ImportSource {
    const name = (path.split(/[\\/]/).pop() ?? "").toLowerCase();
    if (name === "sitemanager.xml") return "filezilla";
    if (name === "winscp.ini") return "winscp";
    if (name === "wcx_ftp.ini") return "wcx-ftp";
    if (name === "sites.dat") return "sites-dat";
    if (name === "config") return "ssh-config";
    if (name.endsWith(".xml") || name.endsWith(".ftp")) return "sites-xml";
    return "ssh-config";
  }

  /**
   * Our own export.
   *
   * Whether it is sealed can be seen from its first bytes, so a plain one opens
   * straight away and a sealed one asks — rather than everybody being asked for
   * a passphrase that may not exist.
   */
  async function lookIntoBundle(path: string): Promise<void> {
    busy = true;
    failure = null;
    bundlePath = path;
    try {
      const read = await api.bundlePreview(path, passphrase || null);
      bundle = read;
      chosen = new Set(read.entries.map((_, index) => index));
    } catch (problem) {
      failure = problem;
      bundle = null;
    } finally {
      busy = false;
    }
  }

  async function applyBundle(): Promise<void> {
    busy = true;
    failure = null;
    try {
      const taken = await api.bundleApply(
        bundlePath,
        passphrase || null,
        [...chosen].sort((a, b) => a - b),
        into,
      );
      ondone(taken);
    } catch (problem) {
      failure = problem;
    } finally {
      busy = false;
    }
  }

  async function look(source: ImportSource, path: string): Promise<void> {
    busy = true;
    failure = null;
    try {
      preview = await api.importPreview(source, path);
      // Everything ticked to begin with: somebody who opened this wants their
      // servers, and unticking two is less work than ticking twenty-eight.
      chosen = new Set(preview.entries.map((_, index) => index));
      takePasswords = preview.entries.some((entry) => entry.hasPassword);
    } catch (problem) {
      failure = problem;
      preview = null;
    } finally {
      busy = false;
    }
  }

  function toggle(index: number): void {
    const next = new Set(chosen);
    if (next.has(index)) next.delete(index);
    else next.add(index);
    chosen = next;
  }

  async function apply(): Promise<void> {
    if (!preview) return;
    busy = true;
    failure = null;
    try {
      const taken = await api.importApply(
        preview.source,
        preview.path,
        [...chosen].sort((a, b) => a - b),
        preview.entries.length,
        takePasswords,
        into,
      );
      ondone(taken);
    } catch (problem) {
      failure = problem;
    } finally {
      busy = false;
    }
  }

  let withPasswords = $derived(
    preview ? preview.entries.filter((entry) => entry.hasPassword).length : 0,
  );
</script>

<div
  class="backdrop"
  role="presentation"
  ondragover={(event) => {
    event.preventDefault();
    dropping = true;
  }}
  ondragleave={() => (dropping = false)}
>
  <div class="dialog" class:dropping use:trap role="dialog" aria-modal="true" aria-label={t("import.title")}>
    <header>
      <h2>{t("import.title")}</h2>
      <button
        type="button"
        class="close"
        onclick={onclose}
        title={t("action.close")}
        aria-label={t("action.close")}
      >×</button>
    </header>

    {#if bundle}
      <p class="where mono">{bundlePath}</p>

      {#if bundle.sealed && bundle.entries.length === 0}
        <p>{t("import.sealed")}</p>
        <label class="field">
          <span>{t("export.passphrase")}</span>
          <!-- svelte-ignore a11y_autofocus -->
          <input
            type="password"
            bind:value={passphrase}
            autofocus
            autocomplete="off"
            autocapitalize="off"
            autocorrect="off"
            spellcheck="false"
            onkeydown={(event) => {
              if (event.key === "Enter") void lookIntoBundle(bundlePath);
            }}
          />
        </label>
      {:else}
        <div class="rows">
          {#each bundle.entries as entry, index (index)}
            <label class="row">
              <input type="checkbox" checked={chosen.has(index)} onchange={() => toggle(index)} />
              <span class="name">
                {#if entry.folder}<span class="folder">{entry.folder}/</span>{/if}{entry.name}
              </span>
              <span class="where mono">{entry.user}@{entry.host}:{entry.port}</span>
              {#if entry.hasPassword}
                <span class="tag">{t("import.has-password")}</span>
              {/if}
            </label>
          {/each}
        </div>
      {/if}
    {:else if !preview}
      <p>{t("import.body")}</p>

      {#if looking}
        <p class="quiet">{t("import.looking")}</p>
      {:else if candidates.length === 0}
        <p class="quiet">{t("import.none")}</p>
      {:else}
        <ul class="found">
          {#each candidates as candidate (candidate.path)}
            <li>
              <button type="button" onclick={() => void look(candidate.source, candidate.path)}>
                <span class="what">{t(`import.source.${candidate.source}`)}</span>
                <span class="where mono">{candidate.path}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}

      <div class="pick">
        <button
          type="button"
          onclick={async () => {
            const path = await api.chooseFile(t("import.choose"));
            if (!path) return;
            if (isOurs(path)) await lookIntoBundle(path);
            else await look(guess(path), path);
          }}
        >
          {t("import.choose")}
        </button>
        <p class="hint">{t("import.drop")}</p>
      </div>
    {:else}
      <p class="where mono">{preview.path}</p>

      {#each preview.warnings as warning (warning)}
        <p class="warning">{t(warning)}</p>
      {/each}

      {#if preview.entries.length === 0}
        <p class="quiet">{t("import.empty")}</p>
      {:else}
        <div class="rows">
          {#each preview.entries as entry, index (index)}
            <label class="row">
              <input type="checkbox" checked={chosen.has(index)} onchange={() => toggle(index)} />
              <span class="name">
                {#if entry.folder}<span class="folder">{entry.folder}/</span>{/if}{entry.name}
              </span>
              <span class="where mono">{entry.user}@{entry.host}:{entry.port}</span>
              {#if entry.hasPassword}
                <span class="tag">{t("import.has-password")}</span>
              {/if}
            </label>
          {/each}
        </div>

        {#if withPasswords > 0}
          <label class="check">
            <input type="checkbox" bind:checked={takePasswords} />
            <span>{t("import.take-passwords", { count: withPasswords })}</span>
          </label>
          <p class="hint">{t("import.take-passwords.hint")}</p>
        {/if}
      {/if}
    {/if}

    {#if failure}
      <p class="failure">{describe(failure)}</p>
    {/if}

    <div class="actions">
      {#if preview || bundle}
        <button
          type="button"
          onclick={() => {
            preview = null;
            bundle = null;
            passphrase = "";
          }}
        >
          {t("import.back")}
        </button>
      {/if}
      <span class="spacer"></span>
      <button type="button" onclick={onclose}>{t("action.cancel")}</button>
      {#if bundle}
        {#if bundle.sealed && bundle.entries.length === 0}
          <button
            type="button"
            class="primary"
            disabled={busy || passphrase.length === 0}
            onclick={() => void lookIntoBundle(bundlePath)}
          >
            {t("import.unseal")}
          </button>
        {:else}
          <button
            type="button"
            class="primary"
            disabled={busy || chosen.size === 0}
            onclick={() => void applyBundle()}
          >
            {t("import.take", { count: chosen.size })}
          </button>
        {/if}
      {:else if preview}
        <button
          type="button"
          class="primary"
          disabled={busy || chosen.size === 0}
          onclick={() => void apply()}
        >
          {t("import.take", { count: chosen.size })}
        </button>
      {/if}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 45%);
    display: grid;
    place-items: center;
    z-index: 60;
  }

  .dialog {
    width: min(640px, 94vw);
    max-height: 86vh;
    display: flex;
    flex-direction: column;
    gap: 10px;
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-radius: 1rem;
    box-shadow: var(--shadow-lg);
    padding: 18px 20px;
  }

  .dialog.dropping {
    border-color: var(--accent);
  }

  header {
    display: flex;
    align-items: center;
  }

  h2 {
    margin: 0;
    font-size: 1rem;
    flex: 1;
  }

  .close {
    background: none;
    border: none;
    color: var(--text-faint);
    font-size: 1.2rem;
    cursor: default;
  }

  p {
    margin: 0;
    font-size: 0.85rem;
    color: var(--text-muted);
  }

  .quiet,
  .hint {
    color: var(--text-faint);
    font-size: 0.8rem;
  }

  .found {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .found button {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    width: 100%;
    text-align: left;
    font: inherit;
    padding: 7px 10px;
    border-radius: 0.5rem;
    border: 1px solid var(--border);
    background: var(--surface-0);
    color: var(--text);
    cursor: default;
  }

  .found button:hover {
    border-color: var(--accent);
    background: var(--surface-2);
  }

  .what {
    font-size: 0.85rem;
  }

  .where {
    font-size: 0.72rem;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }

  .rows {
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    background: var(--surface-0);
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 10px;
    font-size: 0.82rem;
  }

  .row:hover {
    background: var(--surface-2);
  }

  .row input {
    width: auto;
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .folder {
    color: var(--text-faint);
  }

  .tag {
    font-size: 0.66rem;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--accent);
    background: var(--accent-soft);
    padding: 1px 6px;
    border-radius: 999px;
  }

  .pick {
    display: flex;
    flex-direction: column;
    gap: 6px;
    align-items: flex-start;
  }

  .pick button {
    font: inherit;
    font-size: 0.84rem;
    padding: 5px 14px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    background: var(--surface-1);
    color: var(--text);
    cursor: default;
  }

  .pick button:hover {
    background: var(--surface-2);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 0.78rem;
    color: var(--text-muted);
  }

  .field input {
    font: inherit;
    font-size: 0.86rem;
    padding: 5px 8px;
    border-radius: 0.4rem;
    border: 1px solid var(--border-strong);
    background: var(--surface-0);
    color: var(--text);
    width: 100%;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.84rem;
    color: var(--text);
  }

  .check input {
    width: auto;
  }

  .warning {
    color: var(--warn);
    background: var(--warn-soft);
    padding: 8px 10px;
    border-radius: 0.5rem;
  }

  .failure {
    color: var(--danger);
    background: var(--danger-soft);
    padding: 8px 10px;
    border-radius: 0.5rem;
  }

  .actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .spacer {
    flex: 1;
  }

  .actions button {
    font: inherit;
    font-size: 0.84rem;
    padding: 5px 14px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    background: var(--surface-1);
    color: var(--text);
    cursor: default;
  }

  .actions button:hover {
    background: var(--surface-2);
  }

  .actions button.primary {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .actions button:disabled {
    opacity: 0.5;
  }
</style>
