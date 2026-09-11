<script lang="ts">
  import { untrack } from "svelte";

  import type { DirEntry } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import { trap } from "./trap";

  interface Props {
    entries: DirEntry[];
    /** The bits the dialog opens with, chosen by whoever opened it. */
    initialMode: number;
    /** Whether anything in the selection is a directory. */
    hasDirectory: boolean;
    onapply: (mode: number, recursive: boolean) => void;
    oncancel: () => void;
  }

  let { entries, initialMode, hasDirectory, onapply, oncancel }: Props = $props();

  const GROUPS = ["owner", "group", "other"] as const;
  const FLAGS = [
    { key: "read", bit: 4 },
    { key: "write", bit: 2 },
    { key: "execute", bit: 1 },
  ] as const;

  // Deliberately a copy: the dialog is opened fresh for one selection and the
  // boxes are then the user's to change. Following the prop afterwards would
  // undo their clicks whenever the pane behind refreshed.
  let mode = $state(untrack(() => initialMode));
  let recursive = $state(false);

  let octal = $derived(mode.toString(8).padStart(3, "0"));

  function digit(group: number): number {
    return (mode >> (6 - group * 3)) & 0o7;
  }

  function toggle(group: number, bit: number): void {
    mode ^= bit << (6 - group * 3);
  }

  function fromOctal(event: Event): void {
    const text = (event.currentTarget as HTMLInputElement).value;
    const parsed = Number.parseInt(text, 8);
    if (!Number.isNaN(parsed) && parsed >= 0 && parsed <= 0o777) {
      mode = parsed;
    }
  }
</script>

<div class="backdrop" role="presentation">
  <div class="dialog" use:trap role="dialog" aria-modal="true" aria-label={t("perm.title")}>
    <h2>{t("perm.title")}</h2>
    <p class="subject">
      {entries.length === 1 ? entries[0]?.name : t("perm.many", { count: entries.length })}
    </p>

    <table>
      <thead>
        <tr>
          <th></th>
          {#each FLAGS as flag (flag.key)}
            <th>{t(`perm.${flag.key}`)}</th>
          {/each}
          <th class="value">{t("perm.value")}</th>
        </tr>
      </thead>
      <tbody>
        {#each GROUPS as group, index (group)}
          <tr>
            <th scope="row">{t(`perm.${group}`)}</th>
            {#each FLAGS as flag (flag.key)}
              <td>
                <input
                  type="checkbox"
                  checked={(digit(index) & flag.bit) !== 0}
                  onchange={() => toggle(index, flag.bit)}
                  aria-label={`${t(`perm.${group}`)} ${t(`perm.${flag.key}`)}`}
                />
              </td>
            {/each}
            <td class="value mono">{digit(index)}</td>
          </tr>
        {/each}
      </tbody>
    </table>

    <label class="octal">
      <span>{t("perm.octal")}</span>
      <input
        value={octal}
        oninput={fromOctal}
        maxlength="3"
        inputmode="numeric"
        autocomplete="off"
        autocapitalize="off"
        autocorrect="off"
        spellcheck="false"
        class="mono"
      />
    </label>

    {#if hasDirectory}
      <label class="recursive">
        <input type="checkbox" bind:checked={recursive} />
        <span>{t("perm.recursive")}</span>
      </label>
      {#if recursive}
        <p class="warning">{t("perm.recursive.warning")}</p>
      {/if}
    {/if}

    <p class="hint">{t("perm.links")}</p>

    <div class="actions">
      <button type="button" onclick={oncancel}>{t("action.cancel")}</button>
      <button type="button" class="primary" onclick={() => onapply(mode, recursive)}>
        {t("perm.apply")}
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
    z-index: 45;
  }

  .dialog {
    width: min(420px, 92vw);
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-radius: 1rem;
    box-shadow: var(--shadow-lg);
    padding: 18px 20px;
  }

  h2 {
    margin: 0 0 2px;
    font-size: 1rem;
  }

  .subject {
    margin: 0 0 14px;
    font-size: 0.82rem;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    margin-bottom: 12px;
  }

  th,
  td {
    padding: 4px 6px;
    text-align: center;
    font-size: 0.78rem;
  }

  thead th {
    font-size: 0.66rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-faint);
    font-weight: 600;
  }

  tbody th {
    text-align: left;
    color: var(--text-muted);
    font-weight: 500;
  }

  .value {
    color: var(--text-faint);
  }

  .octal {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 10px;
  }

  .octal span {
    font-size: 0.68rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  .octal input {
    width: 64px;
    font: inherit;
    font-size: 0.88rem;
    padding: 4px 8px;
    border: 1px solid var(--border-strong);
    border-radius: 0.5rem;
    background: var(--surface-2);
    color: var(--text);
    text-align: center;
  }

  .recursive {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.84rem;
    color: var(--text-muted);
    margin-bottom: 8px;
  }

  .warning {
    margin: 0 0 8px;
    font-size: 0.78rem;
    color: var(--warn);
    background: var(--warn-soft);
    padding: 8px 10px;
    border-radius: 0.5rem;
  }

  .hint {
    margin: 0 0 12px;
    font-size: 0.74rem;
    color: var(--text-faint);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  button {
    font: inherit;
    font-size: 0.86rem;
    padding: 6px 16px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    background: var(--surface-1);
    color: var(--text);
    cursor: default;
  }

  button:hover {
    background: var(--surface-2);
  }

  button.primary {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }
</style>
