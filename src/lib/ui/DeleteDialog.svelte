<script lang="ts">
  import type { DirEntry, Measurement } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import { formatSize } from "./format";

  interface Props {
    entries: DirEntry[];
    /** What the count found, or null while it is still counting. */
    measured: Measurement | null;
    onconfirm: () => void;
    oncancel: () => void;
  }

  let { entries, measured, onconfirm, oncancel }: Props = $props();

  let total = $derived(
    measured ? measured.files + measured.directories + measured.symlinks : 0,
  );
  /** More than the selected rows means the delete reaches into folders. */
  let reachesInside = $derived(total > entries.length);
</script>

<div class="backdrop" role="presentation">
  <div class="dialog" role="alertdialog" aria-modal="true">
    <h2>{t("delete.title")}</h2>

    {#if entries.length === 1}
      <p class="subject mono">{entries[0]?.name}</p>
    {:else}
      <p class="subject">{t("delete.many", { count: entries.length })}</p>
    {/if}

    {#if !measured}
      <p class="counting">{t("delete.counting")}</p>
    {:else}
      <dl>
        <dt>{t("delete.files")}</dt>
        <dd class="mono">{measured.files}</dd>
        <dt>{t("delete.folders")}</dt>
        <dd class="mono">{measured.directories}</dd>
        {#if measured.symlinks > 0}
          <dt>{t("delete.links")}</dt>
          <dd class="mono">{measured.symlinks}</dd>
        {/if}
        <dt>{t("delete.size")}</dt>
        <dd class="mono">{formatSize(measured.bytes)}</dd>
      </dl>

      {#if measured.truncated}
        <p class="warning">{t("delete.truncated", { count: total })}</p>
      {:else if reachesInside}
        <p class="warning">{t("delete.recursive", { count: total })}</p>
      {/if}

      {#if measured.symlinks > 0}
        <p class="hint">{t("delete.links.hint")}</p>
      {/if}
    {/if}

    <div class="actions">
      <button type="button" onclick={oncancel}>{t("action.cancel")}</button>
      <button type="button" class="danger" disabled={!measured} onclick={onconfirm}>
        {t("delete.confirm")}
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
    width: min(440px, 92vw);
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-top: 3px solid var(--danger);
    border-radius: 1rem;
    box-shadow: var(--shadow-lg);
    padding: 18px 20px;
  }

  h2 {
    margin: 0 0 4px;
    font-size: 1rem;
  }

  .subject {
    margin: 0 0 14px;
    font-size: 0.84rem;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .counting {
    margin: 0 0 14px;
    font-size: 0.82rem;
    color: var(--text-faint);
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 3px 16px;
    margin: 0 0 12px;
    padding: 10px 12px;
    background: var(--surface-2);
    border-radius: 0.6rem;
    font-size: 0.82rem;
  }

  dt {
    color: var(--text-faint);
  }

  dd {
    margin: 0;
  }

  .warning {
    margin: 0 0 10px;
    font-size: 0.8rem;
    color: var(--danger);
    background: var(--danger-soft);
    padding: 8px 10px;
    border-radius: 0.5rem;
  }

  .hint {
    margin: 0 0 10px;
    font-size: 0.76rem;
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

  button.danger {
    border-color: var(--danger);
    background: var(--danger-soft);
    color: var(--danger);
  }

  button:disabled {
    opacity: 0.5;
  }
</style>
