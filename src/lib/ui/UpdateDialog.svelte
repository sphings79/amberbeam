<script lang="ts">
  /**
   * What a new release says about itself, before anybody fetches it.
   *
   * "There is an update" is not enough to decide on. A bug fix somebody has
   * been waiting for and a change to something they rely on both look the same
   * from a version number, so the notes belong in front of the decision rather
   * than behind a link somebody may or may not follow.
   *
   * The text arrives from the network. It is never handed to the browser as
   * markup — no `{@html}` anywhere in this file. The few shapes our own
   * release notes use are recognised and drawn here; anything else stays a
   * line of text, which is the worst that can happen and is harmless.
   */
  import { api, type Release } from "../bridge";
  import { describe } from "./errors";
  import { pieces, type Span } from "./notes";
  import { t } from "../i18n/index.svelte";
  import { trap } from "./trap";

  interface Props {
    release: Release;
    onclose: () => void;
  }

  let { release, onclose }: Props = $props();

  let parts = $derived(pieces(release.notes));

  /**
   * Whether this installation can replace itself.
   *
   * Asked rather than assumed. A `.deb` under `/usr` cannot be rewritten
   * without root, and a manifest that has not caught up yet is the same story
   * from the window's side — in both cases the honest offer is the download
   * page, not a button that fails halfway through.
   */
  let canInstall = $state(false);
  let stage = $state<"asking" | "ready" | "working" | "done" | "failed">("asking");
  let downloaded = $state(0);
  let total = $state<number | null>(null);
  let failure = $state<unknown>(null);

  $effect(() => {
    void api.canInstallUpdate().then((yes) => {
      canInstall = yes;
      stage = "ready";
    });
  });

  let share = $derived(total && total > 0 ? Math.min(100, (downloaded / total) * 100) : null);

  async function install(): Promise<void> {
    stage = "working";
    downloaded = 0;
    try {
      await api.installUpdate((got, size) => {
        downloaded = got;
        total = size;
      });
      stage = "done";
    } catch (problem) {
      failure = problem;
      stage = "failed";
    }
  }
</script>

<!--
  One piece of a line: bold, a link, or the characters as they were written.
  A link opens in the system's browser rather than in here, and only exists at
  all where the address was http or https — see `inline` in notes.ts.
-->
{#snippet span(part: Span)}
  {#if part.href}
    <button type="button" class="link" onclick={() => api.openUrl(part.href ?? "")}>
      {part.text}
    </button>
  {:else if part.strong}
    <strong>{part.text}</strong>
  {:else}{part.text}{/if}
{/snippet}

<div class="backdrop" role="presentation">
  <div class="dialog" use:trap role="dialog" aria-modal="true" aria-label={t("update.dialog.title")}>
    <header>
      <h2>{t("update.dialog.title", { version: release.version })}</h2>
      <button
        type="button"
        class="close"
        onclick={onclose}
        title={t("action.close")}
        aria-label={t("action.close")}
      >×</button>
    </header>

    <div class="notes">
      {#if parts.length === 0}
        <p class="hint">{t("update.dialog.nothing")}</p>
      {:else}
        {#each parts as piece, index (index)}
          {#if piece.kind === "heading"}
            {#if piece.level <= 2}
              <h3 class="version">{piece.text}</h3>
            {:else}
              <h3>{piece.text}</h3>
            {/if}
          {:else if piece.kind === "bullet"}
            <p class="bullet">
              {#each piece.parts as part, at (at)}
                {@render span(part)}
              {/each}
            </p>
          {:else if piece.kind === "rule"}
            <hr />
          {:else}
            <p class:under={piece.kind === "under"}>
              {#each piece.parts as part, at (at)}
                {@render span(part)}
              {/each}
            </p>
          {/if}
        {/each}
      {/if}
      {#if release.older}
        <!-- GitHub is asked for a fixed number of releases. When every one of
             them was newer than this build, the ones before were cut off by
             that limit rather than by not existing, and saying so beats
             implying the list is complete. -->
        <p class="hint">
          {t("update.dialog.older")}
          <button type="button" class="link" onclick={() => api.openUrl(release.changelog)}>
            {t("update.dialog.older.link")}
          </button>
        </p>
      {/if}
    </div>

    <footer>
      {#if stage === "working"}
        <div class="progress" role="progressbar" aria-label={t("update.dialog.working")}>
          <div class="bar" style:width={share === null ? "100%" : `${share}%`} class:unknown={share === null}></div>
        </div>
        <p class="where">
          {share === null
            ? t("update.dialog.working")
            : t("update.dialog.working.share", { done: Math.round(share) })}
        </p>
      {:else if stage === "done"}
        <p class="where done">{t("update.dialog.done")}</p>
        <div class="actions">
          <button type="button" onclick={onclose}>{t("update.dialog.later")}</button>
          <button type="button" class="go" onclick={() => void api.restart()}>
            {t("update.dialog.restart")}
          </button>
        </div>
      {:else}
        <p class="where" class:bad={stage === "failed"}>
          {#if stage === "failed"}
            {describe(failure)}
          {:else if canInstall}
            {t("update.dialog.install.where")}
          {:else}
            {t("update.dialog.where")}
          {/if}
        </p>
        <div class="actions">
          <button type="button" onclick={onclose}>{t("update.dialog.later")}</button>
          {#if canInstall}
            <button type="button" class="go" onclick={() => void install()}>
              {stage === "failed" ? t("update.dialog.retry") : t("update.dialog.install")}
            </button>
          {:else}
            <button type="button" class="go" onclick={() => api.openUrl(release.url)}>
              {t("update.dialog.download")}
            </button>
          {/if}
        </div>
      {/if}
    </footer>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 70;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 45%);
  }

  .dialog {
    display: flex;
    flex-direction: column;
    width: min(560px, 92vw);
    max-height: 80vh;
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-radius: 0.8rem;
    box-shadow: var(--shadow-lg);
    outline: none;
  }

  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 14px 16px;
    border-bottom: 1px solid var(--border);
  }

  h2 {
    flex: 1;
    margin: 0;
    font-size: 0.95rem;
  }

  .close {
    border: none;
    background: none;
    color: var(--text-faint);
    font-size: 1rem;
    cursor: pointer;
  }

  .notes {
    overflow-y: auto;
    padding: 4px 16px 12px;
    font-size: 0.82rem;
    line-height: 1.5;
  }

  .link {
    border: none;
    background: none;
    padding: 0;
    font: inherit;
    color: var(--accent);
    text-decoration: underline;
    cursor: default;
  }

  .notes hr {
    margin: 14px 0 10px;
    border: none;
    border-top: 1px solid var(--border);
  }

  .notes h3.version {
    margin: 20px 0 6px;
    padding-top: 14px;
    border-top: 1px solid var(--border);
    font-size: 0.95rem;
    color: var(--text);
  }

  /* The first one opens the list rather than dividing it. */
  .notes h3.version:first-child {
    margin-top: 0;
    padding-top: 0;
    border-top: none;
  }

  .notes h3 {
    margin: 14px 0 4px;
    font-size: 0.7rem;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  .notes p {
    margin: 5px 0;
    color: var(--text-muted);
  }

  .notes p.bullet {
    padding-left: 14px;
    text-indent: -14px;
  }

  /* Lined up under the point it belongs to, so it reads as part of it. */
  .notes p.under {
    padding-left: 14px;
  }

  .notes p.bullet::before {
    content: "· ";
    color: var(--accent);
  }

  .notes strong {
    color: var(--text);
    font-weight: 600;
  }

  .hint {
    color: var(--text-muted);
  }

  footer {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    border-top: 1px solid var(--border);
  }

  .where {
    flex: 1;
    margin: 0;
    font-size: 0.72rem;
    color: var(--text-faint);
  }

  .where.done {
    color: var(--ok);
  }

  .where.bad {
    color: var(--danger);
  }

  .progress {
    flex: 0 0 120px;
    height: 5px;
    border-radius: 999px;
    background: var(--surface-3);
    overflow: hidden;
  }

  .progress .bar {
    height: 100%;
    background: var(--accent);
    transition: width 120ms linear;
  }

  /* A server that will not say how large the file is still has to look like
     something is happening, so the bar moves instead of standing full. */
  .progress .bar.unknown {
    animation: ab-beam 1.2s linear infinite;
    background: linear-gradient(90deg, var(--surface-3), var(--accent), var(--surface-3));
    background-size: 60% 100%;
  }

  .actions {
    display: flex;
    gap: 8px;
  }

  footer button {
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    padding: 5px 12px;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 0.8rem;
    cursor: pointer;
  }

  footer button.go {
    border-color: var(--accent);
    color: var(--accent);
    font-weight: 600;
  }

  footer button:hover {
    border-color: var(--border-strong);
  }

  footer button.go:hover {
    background: var(--accent-soft);
  }
</style>
