<script lang="ts">
  import type { CoreError } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import { trap } from "./trap";

  interface Props {
    question: Extract<CoreError, { kind: "host-key-unknown" | "host-key-changed" }>;
    onaccept: (fingerprint: string) => void;
    oncancel: () => void;
  }

  let { question, onaccept, oncancel }: Props = $props();

  let changed = $derived(question.kind === "host-key-changed");
</script>

<div class="backdrop" role="presentation">
  <div class="dialog" class:changed use:trap role="alertdialog" aria-modal="true">
    <h2>{changed ? t("hostkey.changed.title") : t("hostkey.unknown.title")}</h2>

    <p>
      {changed
        ? t("hostkey.changed.body", { host: question.host })
        : t("hostkey.unknown.body", { host: question.host })}
    </p>

    <dl>
      <dt>{t("hostkey.fingerprint")}</dt>
      <dd class="mono">{question.fingerprint}</dd>
      {#if question.kind === "host-key-changed"}
        <dt>{t("hostkey.known-fingerprint")}</dt>
        <dd class="mono">{question.knownFingerprint}</dd>
      {/if}
    </dl>

    {#if changed}
      <p class="warning">{t("hostkey.changed.warning")}</p>
    {:else}
      <p class="hint">{t("hostkey.unknown.hint")}</p>
    {/if}

    <div class="actions">
      <button type="button" onclick={oncancel}>{t("action.cancel")}</button>
      <button
        type="button"
        class:danger={changed}
        class:primary={!changed}
        onclick={() => onaccept(question.fingerprint)}
      >
        {changed ? t("hostkey.changed.accept") : t("hostkey.unknown.accept")}
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
    z-index: 50;
  }

  .dialog {
    width: min(520px, 92vw);
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-top: 3px solid var(--warn);
    border-radius: 1rem;
    box-shadow: var(--shadow-lg);
    padding: 20px 22px;
  }

  .dialog.changed {
    border-top-color: var(--danger);
  }

  h2 {
    margin: 0 0 8px;
    font-size: 1.05rem;
  }

  p {
    margin: 0 0 12px;
    color: var(--text-muted);
    font-size: 0.88rem;
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 4px 14px;
    margin: 0 0 12px;
    padding: 10px 12px;
    background: var(--surface-2);
    border-radius: 0.6rem;
  }

  dt {
    font-size: 0.68rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  dd {
    margin: 0;
    font-size: 0.8rem;
    word-break: break-all;
  }

  .warning {
    color: var(--danger);
    background: var(--danger-soft);
    padding: 10px 12px;
    border-radius: 0.6rem;
  }

  .hint {
    color: var(--text-faint);
    font-size: 0.82rem;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
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

  button.danger {
    border-color: var(--danger);
    background: var(--danger-soft);
    color: var(--danger);
  }
</style>
