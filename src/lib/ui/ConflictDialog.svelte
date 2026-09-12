<script lang="ts">
  /** How far an answer reaches: this file, the rest of this run, or for good. */
  export type Scope = "one" | "rest" | "always";

  import type { ConflictPolicy, QueuedJob } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import { trap } from "./trap";
  import { formatDate, formatSize } from "./format";

  interface Props {
    job: QueuedJob;
    ondecide: (policy: ConflictPolicy, scope: Scope) => void;
    onskipall: () => void;
  }

  let { job, ondecide, onskipall }: Props = $props();

  /**
   * How far an answer reaches.
   *
   * A tick saying "and the others" was there before, and it only appeared when
   * more than one job was already waiting — which during a folder upload is
   * never, because they are walked one at a time. So the question came back
   * for every file and the answer that would have stopped it was invisible.
   *
   * Three reaches, said plainly. "The rest" now includes the ones not thought
   * of yet, which is what it has to mean while a folder is still being walked.
   */
  let scope = $state<Scope>("one");

  /**
   * The accented choice, and the one Enter means.
   *
   * It looked chosen already and was not: the colour said "press Enter" and
   * Enter did nothing. A button that wears the accent has to be the one the
   * keyboard is on, or the colour is a lie about what the program will do.
   */
  let armed = $state<HTMLButtonElement | null>(null);


  $effect(() => {
    armed?.focus();
  });

  const SCOPES: { value: Scope; key: string }[] = [
    { value: "one", key: "conflict.scope.one" },
    { value: "rest", key: "conflict.scope.rest" },
    { value: "always", key: "conflict.scope.always" },
  ];

  const CHOICES: { policy: ConflictPolicy; key: string; accent?: boolean }[] = [
    { policy: "overwrite", key: "conflict.overwrite", accent: true },
    { policy: "overwrite-if-newer", key: "conflict.if-newer" },
    { policy: "resume", key: "conflict.resume" },
    { policy: "rename", key: "conflict.rename" },
    { policy: "skip", key: "conflict.skip" },
  ];

  /** Read from the same list the button is drawn from, so they cannot differ. */
  const accented = CHOICES.find((choice) => choice.accent)?.policy;
</script>

<div class="backdrop" role="presentation">
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="dialog"
    use:trap
    role="alertdialog"
    aria-modal="true"
    tabindex="-1"
    onkeydown={(event) => {
      // The accented button is what Enter means, wherever the cursor happens
      // to be. Focusing it was not enough: choosing how far the answer reaches
      // moves the focus onto a radio, and Enter then landed on nothing — a
      // button that looks armed and does nothing is worse than one that looks
      // plain.
      //
      // Unless the focus is already on a button, which has its own meaning
      // and gets to keep it.
      if (event.key !== "Enter") return;
      if ((event.target as HTMLElement | null)?.tagName === "BUTTON") return;
      event.preventDefault();
      if (accented) ondecide(accented, scope);
    }}
  >
    <h2>{t("conflict.title")}</h2>
    <p class="subject mono" title={job.targetPath}>{job.name}</p>
    <p class="explain">{t("conflict.body")}</p>

    <!-- What is there against what is coming. Answering "overwrite or keep"
         without seeing which is newer is guessing. -->
    <table>
      <thead>
        <tr>
          <th></th>
          <th>{t("conflict.existing")}</th>
          <th>{t("conflict.new")}</th>
        </tr>
      </thead>
      <tbody>
        <tr>
          <th scope="row">{t("column.size")}</th>
          <td class="mono">{formatSize(job.existingSize)}</td>
          <td class="mono" class:bigger={
            job.totalBytes !== null &&
            job.existingSize !== null &&
            job.totalBytes > job.existingSize
          }>
            {formatSize(job.totalBytes)}
          </td>
        </tr>
        <tr>
          <th scope="row">{t("column.modified")}</th>
          <td class="mono">
            {job.existingModified === null ? t("conflict.unknown") : formatDate(job.existingModified)}
          </td>
          <td class="mono" class:bigger={
            job.sourceModified !== null &&
            job.existingModified !== null &&
            job.sourceModified > job.existingModified
          }>
            {job.sourceModified === null ? t("conflict.unknown") : formatDate(job.sourceModified)}
          </td>
        </tr>
      </tbody>
    </table>

    <p class="target mono" title={job.targetPath}>{job.targetPath}</p>

    <div class="choices">
      {#each CHOICES as choice (choice.policy)}
        <button
          type="button"
          class:primary={choice.accent}
          {@attach (node) => {
            if (choice.accent) armed = node;
          }}
          onclick={() => ondecide(choice.policy, scope)}
        >
          {t(choice.key)}
        </button>
      {/each}
    </div>

    <p class="hint">{t("conflict.resume.hint")}</p>

    <fieldset class="reach">
      <legend>{t("conflict.scope")}</legend>
      {#each SCOPES as choice (choice.value)}
        <label>
          <input type="radio" value={choice.value} bind:group={scope} />
          <span>{t(choice.key)}</span>
        </label>
      {/each}
    </fieldset>
    {#if scope === "always"}
      <p class="hint warn">{t("conflict.scope.always.hint")}</p>
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

  table {
    width: 100%;
    border-collapse: collapse;
    margin-bottom: 8px;
    background: var(--surface-2);
    border-radius: 0.6rem;
    overflow: hidden;
    font-size: 0.8rem;
  }

  th,
  td {
    padding: 5px 10px;
    text-align: right;
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
    color: var(--text-faint);
    font-weight: 500;
  }

  /* The newer or larger of the two is marked, because that is the thing the
     answer usually turns on. */
  .bigger {
    color: var(--accent);
  }

  .target {
    margin: 0 0 12px;
    font-size: 0.72rem;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: ltr;
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


  .reach {
    display: flex;
    align-items: center;
    gap: 14px;
    margin: 12px 0 0;
    padding: 8px 12px;
    border: 1px solid var(--border);
    border-radius: 0.6rem;
  }

  .reach legend {
    padding: 0 6px;
    font-size: 0.7rem;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  .reach label {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 0.8rem;
  }

  .hint.warn {
    color: var(--warn);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
  }
</style>
