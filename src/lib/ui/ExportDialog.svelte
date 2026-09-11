<script lang="ts">
  /**
   * Taking the list with you.
   *
   * The one question that matters is the first one, and it is asked as a
   * choice between two things rather than as a switch: a file without
   * passwords, which anybody can read and which is fine to put in a backup, or
   * a file with them, which is sealed and needs a passphrase. There is no third
   * option, because a file that carries passwords and is merely obscured is
   * precisely what the importers spend their time undoing.
   */
  import { api } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import { describe } from "./errors";

  interface Props {
    onclose: () => void;
    ondone: (count: number, path: string) => void;
  }

  let { onclose, ondone }: Props = $props();

  let withPasswords = $state(false);
  let passphrase = $state("");
  let again = $state("");
  let busy = $state(false);
  let failure = $state<unknown>(null);

  let mismatch = $derived(withPasswords && passphrase !== "" && passphrase !== again);
  let ready = $derived(
    !busy && (!withPasswords || (passphrase.length >= 8 && passphrase === again)),
  );

  async function write(): Promise<void> {
    busy = true;
    failure = null;
    try {
      const path = await api.chooseSaveFile(t("export.title"), "server.amberbeam-sites");
      if (!path) return;
      const count = await api.exportSites(path, withPasswords, withPasswords ? passphrase : null);
      ondone(count, path);
    } catch (problem) {
      failure = problem;
    } finally {
      busy = false;
    }
  }
</script>

<div class="backdrop" role="presentation">
  <div class="dialog" role="dialog" aria-modal="true" aria-label={t("export.title")}>
    <h2>{t("export.title")}</h2>

    <div class="choices">
      <button type="button" class:active={!withPasswords} onclick={() => (withPasswords = false)}>
        {t("export.without")}
      </button>
      <button type="button" class:active={withPasswords} onclick={() => (withPasswords = true)}>
        {t("export.with")}
      </button>
    </div>

    <p class="hint">{withPasswords ? t("export.with.hint") : t("export.without.hint")}</p>

    {#if withPasswords}
      <label>
        <span>{t("export.passphrase")}</span>
        <input
          type="password"
          bind:value={passphrase}
          autocomplete="off"
          autocapitalize="off"
          autocorrect="off"
          spellcheck="false"
        />
      </label>
      <label>
        <span>{t("export.passphrase.again")}</span>
        <input
          type="password"
          bind:value={again}
          autocomplete="off"
          autocapitalize="off"
          autocorrect="off"
          spellcheck="false"
        />
      </label>
      {#if mismatch}
        <p class="warning">{t("export.passphrase.mismatch")}</p>
      {/if}
      <p class="hint">{t("export.passphrase.hint")}</p>
    {/if}

    {#if failure}
      <p class="failure">{describe(failure)}</p>
    {/if}

    <div class="actions">
      <span class="spacer"></span>
      <button type="button" onclick={onclose}>{t("action.cancel")}</button>
      <button type="button" class="primary" disabled={!ready} onclick={() => void write()}>
        {t("export.write")}
      </button>
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
    width: min(480px, 94vw);
    display: flex;
    flex-direction: column;
    gap: 10px;
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-radius: 1rem;
    box-shadow: var(--shadow-lg);
    padding: 18px 20px;
  }

  h2 {
    margin: 0;
    font-size: 1rem;
  }

  p {
    margin: 0;
    font-size: 0.85rem;
    color: var(--text-muted);
  }

  .hint {
    color: var(--text-faint);
    font-size: 0.8rem;
  }

  .choices {
    display: flex;
    gap: 6px;
  }

  .choices button {
    flex: 1;
    font: inherit;
    font-size: 0.84rem;
    padding: 7px 12px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    background: var(--surface-0);
    color: var(--text-muted);
    cursor: default;
  }

  .choices button.active {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 0.78rem;
    color: var(--text-muted);
  }

  input {
    font: inherit;
    font-size: 0.86rem;
    padding: 5px 8px;
    border-radius: 0.4rem;
    border: 1px solid var(--border-strong);
    background: var(--surface-0);
    color: var(--text);
    width: 100%;
  }

  .warning {
    color: var(--warn);
    font-size: 0.8rem;
  }

  .failure {
    color: var(--danger);
    background: var(--danger-soft);
    padding: 8px 10px;
    border-radius: 0.5rem;
    font-size: 0.8rem;
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

  .actions button.primary {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .actions button:disabled {
    opacity: 0.5;
  }
</style>
