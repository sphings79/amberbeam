<script lang="ts">
  import { t } from "../i18n/index.svelte";
  import type { Command } from "./commands";
  import Icon from "./Icon.svelte";

  interface Props {
    x: number;
    y: number;
    items: { command: Command; usable: boolean; reason: string | null }[];
    onpick: (command: Command) => void;
    onclose: () => void;
  }

  let { x, y, items, onpick, onclose }: Props = $props();

  let menu = $state<HTMLDivElement | null>(null);

  // Kept inside the window: a menu opened near the bottom edge that runs off
  // the screen is a menu with commands nobody can reach.
  let position = $derived.by(() => {
    const width = 220;
    const height = items.length * 28 + 12;
    return {
      left: Math.min(x, window.innerWidth - width - 8),
      top: Math.min(y, window.innerHeight - height - 8),
    };
  });

  $effect(() => {
    menu?.focus();
  });
</script>

<svelte:window onkeydown={(event) => event.key === "Escape" && onclose()} />

<button type="button" class="scrim" onclick={onclose} oncontextmenu={(e) => (e.preventDefault(), onclose())} aria-label={t("action.cancel")}></button>

<div
  class="menu"
  bind:this={menu}
  role="menu"
  tabindex="-1"
  style:left="{position.left}px"
  style:top="{position.top}px"
>
  {#each items as item (item.command.id)}
    <button
      type="button"
      role="menuitem"
      disabled={!item.usable}
      onclick={() => onpick(item.command)}
    >
      <span class="glyph"><Icon name={item.command.icon} size={14} /></span>
      <span class="label">{t(item.command.key)}</span>
      {#if item.command.comingIn}
        <span class="soon">{item.command.comingIn}</span>
      {/if}
    </button>
  {/each}
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: none;
    border: none;
    padding: 0;
    cursor: default;
  }

  .menu {
    position: fixed;
    z-index: 61;
    min-width: 208px;
    padding: 5px;
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-radius: 0.7rem;
    box-shadow: var(--shadow-lg);
    outline: none;
  }

  button {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    border: none;
    background: none;
    font: inherit;
    font-size: 0.84rem;
    color: var(--text);
    padding: 4px 9px;
    border-radius: 0.45rem;
    cursor: default;
    text-align: left;
  }

  button:hover:not(:disabled) {
    background: var(--accent-soft);
    color: var(--accent);
  }

  button:disabled {
    color: var(--text-faint);
  }

  .glyph {
    width: 16px;
    display: flex;
    justify-content: center;
    color: var(--text-faint);
  }

  button:hover:not(:disabled) .glyph {
    color: var(--accent);
  }

  .label {
    flex: 1;
  }

  .soon {
    font-size: 0.62rem;
    color: var(--accent);
    background: var(--accent-soft);
    padding: 0 6px;
    border-radius: 999px;
  }
</style>
