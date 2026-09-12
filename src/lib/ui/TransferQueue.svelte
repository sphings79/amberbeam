<script lang="ts">
  import { api, LOCAL, type QueuedJob } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import {
    dismissLowered,
    jobProgress,
    loweredNotice,
    percentOf,
    queueState,
    queueTotals,
    refreshQueue,
  } from "../state/queue.svelte";
  import { describe } from "./errors";
  import { formatSize } from "./format";
  import Icon from "./Icon.svelte";

  interface Props {
    /** Opens the folder a job is going to, in whichever pane shows it. */
    onreveal: (endpoint: string, path: string) => void;
  }

  let { onreveal }: Props = $props();

  let queue = $derived(queueState());

  /**
   * Whether a finished line takes itself away.
   *
   * Kept in the settings rather than in this window: with the container build
   * two browsers look at one queue, and a switch that only one of them knew
   * about would have lines disappearing for one person and not the other.
   */
  let tidying = $state(false);

  $effect(() => {
    void api.settings().then((settings) => (tidying = settings.clearFinished));
  });

  async function setTidying(on: boolean): Promise<void> {
    tidying = on;
    const settings = await api.settings();
    await api.setSettings({ ...settings, clearFinished: on });
  }
  let totals = $derived(queueTotals());
  let dragging = $state<string | null>(null);

  $effect(() => {
    void refreshQueue();
  });

  let overall = $derived(
    totals.totalBytes > 0 ? Math.round((totals.doneBytes / totals.totalBytes) * 100) : null,
  );

  /** Which way a job is going, for the arrow in its row. */
  function direction(job: QueuedJob): string {
    if (job.sourceEndpoint === LOCAL) return "↑";
    if (job.targetEndpoint === LOCAL) return "↓";
    return "⇄";
  }

  /**
   * A copy between two servers travels through AmberBeam — down from one and up
   * to the other. It is not FXP, where the servers would talk to each other,
   * and the row says so rather than leaving it to be discovered.
   */
  function isBetweenServers(job: QueuedJob): boolean {
    return job.sourceEndpoint !== LOCAL && job.targetEndpoint !== LOCAL;
  }

  function rate(job: QueuedJob): string | null {
    const live = jobProgress(job.id);
    if (!live?.rate) return null;
    return t("queue.rate", { rate: formatSize(live.rate) });
  }

  function onDrop(event: DragEvent, index: number): void {
    event.preventDefault();
    if (dragging) {
      void api.queueMove(dragging, undefined, index).then(refreshQueue);
    }
    dragging = null;
  }
</script>

<section class="queue">
  <header>
    <span class="title">{t("queue.title")}</span>

    {#if overall !== null && totals.totalBytes > 0}
      <div class="overall" title={t("queue.overall")}>
        <div class="bar"><div class="fill" style:width="{overall}%"></div></div>
        <span class="figure mono">{overall} %</span>
      </div>
    {/if}

    <span class="counts mono">
      {t("queue.counts", {
        running: totals.runningJobs,
        waiting: totals.waitingJobs,
        done: totals.doneJobs,
      })}
    </span>

    <span class="spacer"></span>

    <button
      type="button"
      class:on={!queue.paused}
      onclick={() => api.queuePause(!queue.paused).then(refreshQueue)}
      title={queue.paused ? t("queue.start") : t("queue.pause")}
    >
      {queue.paused ? "▶" : "❚❚"}
    </button>
    <!-- Beside the pause, because it is the other thing somebody wants to say
         about the queue as a whole rather than about one line in it. -->
    <button
      type="button"
      class:on={tidying}
      onclick={() => void setTidying(!tidying)}
      title={tidying ? t("queue.tidy.on") : t("queue.tidy.off")}
    >
      ⌫
    </button>
    <button
      type="button"
      onclick={() => api.queueClearFinished().then(refreshQueue)}
      title={t("queue.clear.hint")}
    >
      {t("queue.clear")}
    </button>
    <button
      type="button"
      class="danger"
      onclick={() => api.queueClearAll().then(refreshQueue)}
      title={t("queue.clear-all.hint")}
      disabled={queue.jobs.length === 0}
    >
      {t("queue.clear-all")}
    </button>
  </header>

  {#if loweredNotice()}
    {@const notice = loweredNotice()}
    {#if notice}
      <p class="lowered">
        {t("queue.lowered", { allowed: notice.allowed })}
        <button
          type="button"
          onclick={dismissLowered}
          title={t("action.close")}
          aria-label={t("action.close")}
        >×</button>
      </p>
    {/if}
  {/if}

  <div class="rows">
    {#each queue.jobs as job, index (job.id)}
      {@const percent = percentOf(job.id, job.doneBytes, job.totalBytes)}
      <div
        class="row {job.state}"
        draggable="true"
        role="listitem"
        ondragstart={() => (dragging = job.id)}
        ondragover={(event) => event.preventDefault()}
        ondrop={(event) => onDrop(event, index)}
        ondblclick={() => onreveal(job.targetEndpoint, job.targetPath)}
        title={t("queue.reveal")}
      >
        <span class="arrow mono">{direction(job)}</span>
        <span class="name" title={job.targetPath}>{job.name}</span>

        {#if isBetweenServers(job)}
          <span class="through" title={t("queue.through.hint")}>{t("queue.through")}</span>
        {/if}

        <div class="bar">
          {#if percent !== null}
            <div class="fill" style:width="{percent}%"></div>
          {/if}
        </div>

        <span class="state mono">
          {#if job.state === "failed" || (job.state === "paused" && job.failure)}
            <span class="failure" title={describe(job.failure)}>{t("queue.state.failed")}</span>
          {:else if job.state === "asking"}
            <span class="asking">{t("queue.state.asking")}</span>
          {:else if job.state === "running"}
            {rate(job) ?? (percent !== null ? `${percent} %` : t("queue.state.running"))}
          {:else}
            {t(`queue.state.${job.state}`)}
          {/if}
        </span>

        <span class="controls">
          <button
            type="button"
            onclick={() => api.queueMove(job.id, -1).then(refreshQueue)}
            title={t("queue.up")}
            disabled={index === 0}
          >
            ▲
          </button>
          <button
            type="button"
            onclick={() => api.queueMove(job.id, 1).then(refreshQueue)}
            title={t("queue.down")}
            disabled={index === queue.jobs.length - 1}
          >
            ▼
          </button>
          {#if job.state === "running" || job.state === "queued"}
            <button
              type="button"
              onclick={() => api.queueHold(job.id).then(refreshQueue)}
              title={t("queue.hold")}
            >
              ❚❚
            </button>
          {:else if job.state === "paused" || job.state === "failed"}
            <button
              type="button"
              onclick={() => api.queueResume(job.id).then(refreshQueue)}
              title={t("queue.continue")}
            >
              ▶
            </button>
          {/if}
          <button
            type="button"
            onclick={() => api.queueRemove(job.id).then(refreshQueue)}
            title={t("queue.remove")}
          >
            <Icon name="delete" size={13} />
          </button>
        </span>
      </div>
    {:else}
      <p class="empty">{t("queue.empty")}</p>
    {/each}
  </div>
</section>

<style>
  .queue {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--surface-1);
  }

  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 3px 10px;
    background: var(--surface-2);
    border-bottom: 1px solid var(--border);
  }

  .title {
    font-size: 0.66rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-faint);
    font-weight: 600;
  }

  .overall {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 160px;
  }

  .overall .bar {
    flex: 1;
  }

  .figure,
  .counts {
    font-size: 0.68rem;
    color: var(--text-faint);
    white-space: nowrap;
  }

  .spacer {
    flex: 1;
  }

  header button {
    font: inherit;
    font-size: 0.7rem;
    border: none;
    background: none;
    color: var(--text-faint);
    cursor: default;
    padding: 2px 7px;
    border-radius: 4px;
  }

  header button:hover {
    background: var(--surface-3);
    color: var(--text-muted);
  }

  header button.on {
    color: var(--accent);
  }

  header button.danger:hover:not(:disabled) {
    background: var(--danger-soft);
    color: var(--danger);
  }

  header button:disabled {
    opacity: 0.35;
  }

  .rows {
    flex: 1;
    overflow: auto;
  }

  .row {
    display: grid;
    grid-template-columns: 18px minmax(100px, 1fr) auto minmax(80px, 200px) 96px auto;
    align-items: center;
    gap: 8px;
    padding: 2px 10px;
    font-size: 0.8rem;
    color: var(--text-muted);
    border-bottom: 1px solid var(--border);
  }

  .row:hover {
    background: var(--surface-2);
  }

  .row.done {
    color: var(--text-faint);
  }

  .arrow {
    color: var(--accent);
    text-align: center;
  }

  .row.done .arrow {
    color: var(--ok);
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .through {
    font-size: 0.62rem;
    color: var(--text-faint);
    background: var(--surface-3);
    padding: 0 6px;
    border-radius: 999px;
    white-space: nowrap;
  }

  .bar {
    height: 7px;
    border-radius: 4px;
    background: var(--surface-3);
    overflow: hidden;
  }

  .fill {
    height: 100%;
    background: var(--accent);
    border-radius: 4px;
    transition: width 0.25s linear;
  }

  .row.done .fill {
    background: var(--ok);
  }

  .state {
    font-size: 0.7rem;
    text-align: right;
    white-space: nowrap;
  }

  .failure {
    color: var(--danger);
  }

  .asking {
    color: var(--warn);
  }

  .controls {
    display: flex;
    gap: 1px;
  }

  .controls button {
    font: inherit;
    font-size: 0.66rem;
    border: none;
    background: none;
    color: var(--text-faint);
    cursor: default;
    padding: 2px 4px;
    border-radius: 3px;
    display: flex;
    align-items: center;
  }

  .controls button:hover:not(:disabled) {
    background: var(--surface-3);
    color: var(--accent);
  }

  .controls button:disabled {
    opacity: 0.25;
  }

  .empty {
    margin: 0;
    padding: 12px;
    font-size: 0.78rem;
    color: var(--text-faint);
  }

  .lowered {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    padding: 5px 10px;
    font-size: 0.76rem;
    color: var(--warn);
    background: var(--warn-soft);
    border-bottom: 1px solid var(--border);
  }

  .lowered button {
    margin-left: auto;
    border: none;
    background: none;
    color: inherit;
    font-size: 0.9rem;
    cursor: default;
    padding: 0 4px;
  }
</style>
