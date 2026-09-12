<script lang="ts">
  /**
   * The strip that says a directory is being watched.
   *
   * It is the whole of what makes watching safe to leave running: something
   * that quietly uploads files whenever they change has to be visible while it
   * does it, and stoppable in one press. A background job nobody can see is a
   * background job nobody remembers starting.
   */
  import { type Watch } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    watches: Watch[];
    onstop: (id: string) => void;
  }

  let { watches, onstop }: Props = $props();
</script>

{#each watches as watch (watch.id)}
  <div class="strip">
    <Icon name="together" size={13} />
    <span class="what">
      {t("watch.running", {
        root: watch.root,
        target: watch.targetTitle ?? t("pane.local"),
        into: watch.targetRoot,
      })}
    </span>
    <span class="sent">{t("watch.sent", { sent: watch.sent })}</span>
    <button type="button" onclick={() => onstop(watch.id)}>{t("watch.stop")}</button>
  </div>
{/each}

<style>
  .strip {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 12px;
    background: var(--accent-soft);
    border-bottom: 1px solid var(--border);
    color: var(--accent);
    font-size: 0.76rem;
  }

  .what {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sent {
    flex: none;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  button {
    flex: none;
    border: 1px solid var(--accent);
    border-radius: 0.45rem;
    background: transparent;
    color: var(--accent);
    font: inherit;
    font-size: 0.74rem;
    padding: 2px 10px;
    cursor: pointer;
  }
</style>
