<script lang="ts">
  import type { CoreError } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import { trap } from "./trap";

  interface Props {
    question: Extract<CoreError, { kind: "certificate-untrusted" }>;
    onaccept: (fingerprint: string) => void;
    oncancel: () => void;
  }

  let { question, onaccept, oncancel }: Props = $props();

  /**
   * Two of the reasons must not be settled by comparing a fingerprint: a
   * withdrawn certificate and one whose signature does not hold. The button
   * stays, because refusing to offer it at all only teaches people to look for
   * another way round — but it says what it is.
   */
  let grave = $derived(
    question.reason === "certificate.revoked" || question.reason === "certificate.broken",
  );
</script>

<div class="backdrop" role="presentation">
  <div class="dialog" class:grave use:trap role="alertdialog" aria-modal="true">
    <h2>{t("certificate.title")}</h2>

    <p>{t("certificate.body", { host: question.host })}</p>

    <p class="reason" class:grave>{t(question.reason, { host: question.host })}</p>

    <dl>
      <dt>{t("certificate.fingerprint")}</dt>
      <dd class="mono">{question.fingerprint}</dd>
      <dt>{t("certificate.detail")}</dt>
      <dd class="quiet">{question.detail}</dd>
    </dl>

    <p class="hint">{t("certificate.scope", { host: question.host })}</p>

    <div class="actions">
      <button type="button" onclick={oncancel}>{t("action.cancel")}</button>
      <button
        type="button"
        class:danger={grave}
        class:primary={!grave}
        onclick={() => onaccept(question.fingerprint)}
      >
        {t("certificate.accept")}
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
    width: min(560px, 92vw);
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-top: 3px solid var(--warn);
    border-radius: 1rem;
    box-shadow: var(--shadow-lg);
    padding: 20px 22px;
  }

  .dialog.grave {
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

  .reason {
    color: var(--warn);
    background: var(--warn-soft);
    padding: 10px 12px;
    border-radius: 0.6rem;
  }

  .reason.grave {
    color: var(--danger);
    background: var(--danger-soft);
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

  dd.quiet {
    color: var(--text-faint);
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
