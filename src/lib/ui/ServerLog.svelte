<script lang="ts">
  import { LOCAL } from "../bridge";
  import { clearLog, logLines } from "../state/log.svelte";
  import { t } from "../i18n/index.svelte";
  import { formatTime } from "./format";

  let scroller = $state<HTMLDivElement | null>(null);
  let lines = $derived(logLines());
  /** Off when the user scrolled up to read something. */
  let follow = $state(true);

  $effect(() => {
    void lines.length;
    if (follow && scroller) {
      scroller.scrollTop = scroller.scrollHeight;
    }
  });

  function onScroll(): void {
    if (!scroller) return;
    const distance = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight;
    follow = distance < 24;
  }

  const arrow: Record<string, string> = {
    sent: "→",
    received: "←",
    note: "·",
  };

  /** Short tag for the pane a line came from. */
  function tag(endpoint: string): string {
    if (endpoint === LOCAL) return t("log.tag.local");
    if (endpoint.startsWith("left")) return t("log.tag.left");
    if (endpoint.startsWith("right")) return t("log.tag.right");
    return endpoint;
  }
</script>

<section class="log">
  <header>
    <span class="title">{t("log.title")}</span>
    <span class="spacer"></span>
    {#if !follow}
      <button type="button" onclick={() => (follow = true)}>{t("log.follow")}</button>
    {/if}
    <button type="button" onclick={clearLog}>{t("log.clear")}</button>
  </header>
  <div class="lines mono" bind:this={scroller} onscroll={onScroll}>
    {#each lines as line (line.id)}
      <div class="line {line.direction}">
        <span class="time">{formatTime(line.at)}</span>
        <span class="who">{tag(line.endpoint)}</span>
        <span class="arrow">{arrow[line.direction]}</span>
        <span class="text">{line.text}</span>
      </div>
    {:else}
      <p class="empty">{t("log.empty")}</p>
    {/each}
  </div>
</section>

<style>
  .log {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--surface-1);
  }

  header {
    display: flex;
    align-items: center;
    gap: 8px;
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
    padding: 2px 6px;
    border-radius: 4px;
  }

  header button:hover {
    background: var(--surface-3);
    color: var(--text-muted);
  }

  .lines {
    flex: 1;
    overflow: auto;
    padding: 4px 0;
  }

  .line {
    display: grid;
    grid-template-columns: 72px 28px 16px 1fr;
    gap: 4px;
    padding: 0 10px;
    font-size: 0.74rem;
    line-height: 1.5;
  }

  .time {
    color: var(--text-faint);
  }

  .who {
    color: var(--text-faint);
    text-align: right;
  }

  .arrow {
    color: var(--accent);
    text-align: center;
  }

  .text {
    color: var(--text-muted);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .line.received .text {
    color: var(--text);
  }

  .line.note .text {
    color: var(--text-faint);
  }

  .line.note .arrow {
    color: var(--text-faint);
  }

  .empty {
    margin: 0;
    padding: 8px 10px;
    color: var(--text-faint);
    font-size: 0.74rem;
  }
</style>
