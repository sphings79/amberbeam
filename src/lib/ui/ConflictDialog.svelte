<script lang="ts">
  import type { ConflictPolicy, QueuedJob } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import { formatSize } from "./format";

  interface Props {
    job: QueuedJob;
    /** How many others are also waiting for an answer. */
    waiting: number;
    ondecide: (policy: ConflictPolicy, forAll: boolean) => void;
    onskipall: () => void;
  }

  let { job, waiting, ondecide, onskipall }: Props = $props();

  let forAll = $state(false);

  const CHOICES: { policy: ConflictPolicy; key: string; accent?: boolean }[] = [
    { policy: "overwrite", key: "conflict.overwrite", accent: true },
    { policy: "overwrite-if-newer", key: "conflict.if-newer" },
    { policy: "resume", key: "conflict.resume" },
    { policy: "rename", key: "conflict.rename" },
    { policy: "skip", key: "conflict.skip" },
  ];
</script>

<div class="backdrop" role="presentation">
  <div class="dialog" role="alertdialog" aria-modal="true">
    <h2>{t("conflict.title")}</h2>
    <p class="subject mono" title={job.targetPath}>{job.name}</p>
    <p class="explain">{t("conflict.body")}</p>

    <dl>
      <dt>{t("conflict.incoming")}</dt>
      <dd class="mono">{formatSize(job.totalBytes)}</dd>
      <dt>{t("conflict.target")}</dt>
      <dd class="mono">{job.targetPath}</dd>
    </dl>

    <div class="choices">
      {#each CHOICES as choice (choice.policy)}
        <button
          type="button"
          class:primary={choice.accent}
          onclick={() => ondecide(choice.policy, forAll)}
        >
          {t(choice.key)}
        </button>
      {/each}
    </div>

    <p class="hint">{t("conflict.resume.hint")}</p>

    {#if waiting > 1}
      <label class="for-all">
        <input type="checkbox" bind:checked={forAll} />
        <span>{t("conflict.for-all", { count: waiting - 1 })}</span>
      </label>
    {/if}

    <div class="actions">
      <button type="button" onclick={onskipall}>{t("conflict.skip-all")}</button>
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
    z-index: 46;
  }

  .dialog {
    width: min(520px, 92vw);
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-top: 3px solid var(--warn);
    border-radius: 1rem;
    box-shadow: var(--shadow-lg);
    padding: 18px 20px;
  }

  h2 {
    margin: 0 0 4px;
    font-size: 1rem;
  }

  .subject {
    margin: 0 0 10px;
    font-size: 0.84rem;
    color: var(--accent);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .explain {
    margin: 0 0 12px;
    font-size: 0.85rem;
    color: var(--text-muted);
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 3px 16px;
    margin: 0 0 14px;
    padding: 10px 12px;
    background: var(--surface-2);
    border-radius: 0.6rem;
    font-size: 0.8rem;
  }

  dt {
    color: var(--text-faint);
  }

  dd {
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 10px;
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

  button.primary {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .hint {
    margin: 0 0 10px;
    font-size: 0.74rem;
    color: var(--text-faint);
  }

  .for-all {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.82rem;
    color: var(--text-muted);
    margin-bottom: 12px;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
  }
</style>
