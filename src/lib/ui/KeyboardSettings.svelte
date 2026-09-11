<script lang="ts">
  /**
   * Changing the keys, one at a time.
   *
   * A row is clicked, the next key pressed becomes that action's. Nothing is
   * typed and no combination has to be spelled out — the program already knows
   * how to read a key press, and asking somebody to write "Meta+Shift+K" would
   * be asking them to do the machine's job.
   */
  import { api } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import { trap } from "./trap";
  import {
    ACTIONS,
    assign,
    bindingOf,
    changed,
    currentScheme,
    fromFile,
    keysFor,
    label,
    resetAll,
    SCHEMES,
    setScheme,
    toFile,
    unassign,
    type Action,
    type SchemeName,
  } from "../keys/index.svelte";

  interface Props {
    onclose: () => void;
  }

  let { onclose }: Props = $props();

  /** The row waiting for a key, or null. */
  let listening = $state<Action | null>(null);
  /** What the last assignment took the key away from, so the swap is visible. */
  let displaced = $state<Action | null>(null);
  let note = $state<string | null>(null);

  function onKey(event: KeyboardEvent): void {
    if (!listening) return;
    // Escape leaves the row alone; a modifier on its own is not a key yet.
    if (event.key === "Escape") {
      event.preventDefault();
      listening = null;
      return;
    }
    if (["Meta", "Control", "Alt", "Shift"].includes(event.key)) return;

    event.preventDefault();
    displaced = assign(listening, bindingOf(event));
    listening = null;
  }

  async function save(): Promise<void> {
    const path = await api.chooseSaveFile(t("keys.export"), "tasten.amberbeam-keys");
    if (!path) return;
    await api.writeTextFile(path, toFile());
    note = t("keys.exported");
  }

  async function load(): Promise<void> {
    const path = await api.chooseFile(t("keys.import"));
    if (!path) return;
    const text = await api.readTextFile(path);
    note = fromFile(text) ? t("keys.imported") : t("keys.import.failed");
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="backdrop" role="presentation">
  <div class="dialog" use:trap role="dialog" aria-modal="true" aria-label={t("keys.title")}>
    <header>
      <h2>{t("keys.title")}</h2>
      <button type="button" class="close" onclick={onclose} aria-label={t("action.cancel")}>×</button>
    </header>

    <div class="choices">
      {#each SCHEMES as name (name)}
        <button
          type="button"
          class:active={currentScheme() === name}
          onclick={() => setScheme(name as SchemeName)}
        >
          {t(`scheme.${name}`)}
        </button>
      {/each}
    </div>

    {#if changed()}
      <p class="hint">
        {t("keys.changed")}
        <button type="button" class="inline" onclick={resetAll}>{t("keys.reset-all")}</button>
      </p>
    {/if}

    <div class="rows">
      {#each ACTIONS as action (action)}
        {@const keys = keysFor(action)}
        <div class="row" class:listening={listening === action}>
          <span class="what">{t(`action.${action}`)}</span>
          <button
            type="button"
            class="key mono"
            class:unbound={keys.length === 0}
            onclick={() => {
              displaced = null;
              listening = action;
            }}
          >
            {#if listening === action}
              {t("keys.press")}
            {:else if keys.length === 0}
              {t("help.unbound")}
            {:else}
              {keys.map(label).join("  ")}
            {/if}
          </button>
          <button
            type="button"
            class="clear"
            onclick={() => unassign(action)}
            title={t("keys.clear")}
            disabled={keys.length === 0}>×</button
          >
        </div>
      {/each}
    </div>

    {#if displaced}
      <p class="hint">{t("keys.taken-from", { action: t(`action.${displaced}`) })}</p>
    {/if}
    {#if note}
      <p class="hint">{note}</p>
    {/if}

    <div class="actions">
      <button type="button" onclick={() => void load()}>{t("keys.import")}</button>
      <button type="button" onclick={() => void save()}>{t("keys.export")}</button>
      <span class="spacer"></span>
      <button type="button" class="primary" onclick={onclose}>{t("action.close")}</button>
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
    z-index: 65;
  }

  .dialog {
    width: min(540px, 94vw);
    max-height: 88vh;
    display: flex;
    flex-direction: column;
    gap: 10px;
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-radius: 1rem;
    box-shadow: var(--shadow-lg);
    padding: 18px 20px;
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

  .hint {
    margin: 0;
    font-size: 0.78rem;
    color: var(--text-faint);
  }

  .choices {
    display: flex;
    gap: 6px;
  }

  .choices button {
    flex: 1;
  }

  .choices button.active {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .rows {
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 2px 0;
  }

  .what {
    flex: 1;
    font-size: 0.84rem;
    color: var(--text-muted);
  }

  .key {
    min-width: 110px;
    text-align: center;
    font-size: 0.8rem;
    color: var(--accent);
  }

  .key.unbound {
    color: var(--text-faint);
  }

  .row.listening .key {
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  .clear {
    padding: 3px 9px;
    color: var(--text-faint);
  }

  button {
    font: inherit;
    font-size: 0.84rem;
    padding: 5px 14px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    background: var(--surface-1);
    color: var(--text);
    cursor: default;
  }

  button:hover {
    background: var(--surface-2);
  }

  button:disabled {
    opacity: 0.4;
  }

  button.inline {
    padding: 1px 8px;
    font-size: 0.74rem;
  }

  .actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .spacer {
    flex: 1;
  }

  .actions button.primary {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }
</style>
