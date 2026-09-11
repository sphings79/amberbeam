<script lang="ts">
  /**
   * What every key does, right now.
   *
   * Not a copy of the documentation: it reads the same table the keys
   * themselves come from, so it cannot say one thing while the keyboard does
   * another. An action somebody rebound shows the key they gave it.
   */
  import { api } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import { trap } from "./trap";
  import { ACTIONS, currentScheme, keysFor, label } from "../keys/index.svelte";

  interface Props {
    onclose: () => void;
    onsetup: () => void;
  }

  let { onclose, onsetup }: Props = $props();

  /** These are what a list is, not a matter of taste, and never in the table. */
  const FIXED: [string, string][] = [
    ["↑ ↓", "help.fixed.move"],
    ["↵", "help.fixed.open"],
    ["⌫", "help.fixed.up"],
    ["␣", "help.fixed.select"],
    ["⇞ ⇟ ⇱ ⇲", "help.fixed.jump"],
  ];
</script>

<div class="backdrop" role="presentation">
  <div class="dialog" use:trap role="dialog" aria-modal="true" aria-label={t("help.title")}>
    <header>
      <h2>{t("help.title")}</h2>
      <span class="scheme">{t(`scheme.${currentScheme()}`)}</span>
      <button type="button" class="close" onclick={onclose} aria-label={t("action.cancel")}>×</button>
    </header>

    <div class="rows">
      {#each ACTIONS as action (action)}
        {@const keys = keysFor(action)}
        <div class="row" class:unbound={keys.length === 0}>
          <span class="what">{t(`action.${action}`)}</span>
          <span class="keys mono">
            {keys.length === 0 ? t("help.unbound") : keys.map(label).join("  ")}
          </span>
        </div>
      {/each}

      <div class="divider">{t("help.fixed")}</div>
      {#each FIXED as [keys, key] (key)}
        <div class="row">
          <span class="what">{t(key)}</span>
          <span class="keys mono">{keys}</span>
        </div>
      {/each}
    </div>

    <div class="actions">
      <button type="button" onclick={() => api.openUrl("https://github.com/sphings79/amberbeam/tree/main/docs")}>
        {t("help.docs")}
      </button>
      <span class="spacer"></span>
      <button type="button" onclick={onsetup}>{t("help.change")}</button>
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
    z-index: 60;
  }

  .dialog {
    width: min(520px, 94vw);
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

  header {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }

  h2 {
    margin: 0;
    font-size: 1rem;
    flex: 1;
  }

  .scheme {
    font-size: 0.76rem;
    color: var(--accent);
  }

  .close {
    background: none;
    border: none;
    color: var(--text-faint);
    font-size: 1.2rem;
    cursor: default;
  }

  .rows {
    overflow: auto;
    display: flex;
    flex-direction: column;
  }

  .row {
    display: flex;
    align-items: baseline;
    gap: 12px;
    padding: 3px 2px;
    font-size: 0.84rem;
  }

  .row.unbound .keys {
    color: var(--text-faint);
  }

  .what {
    flex: 1;
    color: var(--text-muted);
  }

  .keys {
    font-size: 0.8rem;
    color: var(--accent);
    white-space: nowrap;
  }

  .divider {
    margin: 10px 0 4px;
    font-size: 0.7rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-faint);
    border-top: 1px solid var(--border);
    padding-top: 8px;
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
</style>
