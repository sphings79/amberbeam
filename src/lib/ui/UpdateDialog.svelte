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
  import { pieces } from "./notes";
  import { t } from "../i18n/index.svelte";
  import { trap } from "./trap";

  interface Props {
    release: Release;
    onclose: () => void;
  }

  let { release, onclose }: Props = $props();

  let parts = $derived(pieces(release.notes));
</script>

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
            <h3>{piece.text}</h3>
          {:else if piece.kind === "bullet"}
            <p class="bullet">
              {#each piece.parts as part, at (at)}
                {#if part.strong}<strong>{part.text}</strong>{:else}{part.text}{/if}
              {/each}
            </p>
          {:else}
            <p class:under={piece.kind === "under"}>
              {#each piece.parts as part, at (at)}
                {#if part.strong}<strong>{part.text}</strong>{:else}{part.text}{/if}
              {/each}
            </p>
          {/if}
        {/each}
      {/if}
    </div>

    <footer>
      <p class="where">{t("update.dialog.where")}</p>
      <div class="actions">
        <button type="button" onclick={onclose}>{t("update.dialog.later")}</button>
        <button type="button" class="go" onclick={() => api.openUrl(release.url)}>
          {t("update.dialog.download")}
        </button>
      </div>
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
