<script lang="ts">
  import { api, type Settings } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import { trap } from "./trap";

  interface Props {
    onclose: () => void;
  }

  let { onclose }: Props = $props();

  let settings = $state<Settings | null>(null);

  $effect(() => {
    void api.settings().then((loaded) => (settings = loaded));
  });

  /** Saved as it is changed: a dialog with an OK button that can be lost is
      worse than one that simply remembers. */
  async function change(patch: Partial<Settings>): Promise<void> {
    if (!settings) return;
    const next = { ...settings, ...patch };
    settings = next;
    await api.setSettings(next);
  }
</script>

<div class="backdrop" role="presentation">
  <div class="dialog" use:trap role="dialog" aria-modal="true" aria-label={t("settings.title")}>
    <header>
      <h2>{t("settings.title")}</h2>
      <button type="button" class="close" onclick={onclose} aria-label={t("action.cancel")}>×</button>
    </header>

    {#if settings}
      <div class="body">
        <label class="row">
          <span class="label">{t("settings.concurrency")}</span>
          <input
            type="number"
            min="1"
            max="64"
            value={settings.concurrency ?? ""}
            placeholder={t("settings.concurrency.auto")}
            onchange={(event) => {
              const text = event.currentTarget.value.trim();
              const parsed = Number.parseInt(text, 10);
              void change({
                concurrency: text === "" || Number.isNaN(parsed) ? null : parsed,
              });
            }}
          />
        </label>
        <p class="hint">{t("settings.concurrency.hint")}</p>

        <label class="row">
          <span class="label">{t("settings.retries")}</span>
          <input
            type="number"
            min="1"
            max="20"
            value={settings.retries}
            onchange={(event) => {
              const parsed = Number.parseInt(event.currentTarget.value, 10);
              if (!Number.isNaN(parsed)) void change({ retries: parsed });
            }}
          />
        </label>
        <p class="hint">{t("settings.retries.hint")}</p>

        <label class="check">
          <input
            type="checkbox"
            checked={settings.temporaryName}
            onchange={(event) => change({ temporaryName: event.currentTarget.checked })}
          />
          <span>{t("settings.temporary-name")}</span>
        </label>
        <p class="hint">{t("settings.temporary-name.hint")}</p>

        <label class="check">
          <input
            type="checkbox"
            checked={settings.keepModified}
            onchange={(event) => change({ keepModified: event.currentTarget.checked })}
          />
          <span>{t("settings.keep-modified")}</span>
        </label>

        <label class="check">
          <input
            type="checkbox"
            checked={settings.keepPermissions}
            onchange={(event) => change({ keepPermissions: event.currentTarget.checked })}
          />
          <span>{t("settings.keep-permissions")}</span>
        </label>
        <p class="hint">{t("settings.keep-permissions.hint")}</p>

        <label class="check">
          <input
            type="checkbox"
            checked={settings.checkForUpdates}
            onchange={(event) => change({ checkForUpdates: event.currentTarget.checked })}
          />
          <span>{t("settings.check-updates")}</span>
        </label>
        <p class="hint">{t("settings.check-updates.hint")}</p>

        <p class="note">{t("settings.per-connection")}</p>
      </div>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 45%);
    display: grid;
    place-items: center;
    z-index: 47;
  }

  .dialog {
    width: min(460px, 92vw);
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
    cursor: default;
    padding: 0 6px;
  }

  .body {
    padding: 14px 18px 18px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-top: 8px;
  }

  .label {
    font-size: 0.84rem;
    color: var(--text);
  }

  input[type="number"] {
    width: 96px;
    font: inherit;
    font-size: 0.86rem;
    padding: 4px 8px;
    border: 1px solid var(--border-strong);
    border-radius: 0.5rem;
    background: var(--surface-2);
    color: var(--text);
    text-align: right;
  }

  input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-ring);
  }

  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.84rem;
    color: var(--text);
    margin-top: 10px;
  }

  .hint {
    margin: 2px 0 0;
    font-size: 0.74rem;
    color: var(--text-faint);
  }

  .note {
    margin: 16px 0 0;
    padding-top: 12px;
    border-top: 1px solid var(--border);
    font-size: 0.74rem;
    color: var(--text-faint);
  }
</style>
