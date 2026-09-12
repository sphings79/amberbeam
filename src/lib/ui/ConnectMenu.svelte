<script lang="ts">
  /**
   * What opens when somebody presses connect.
   *
   * The button said "connect" and offered only a form to type a host into,
   * which is the one thing somebody with a saved server does not want to do.
   * The servers they already have belong here, first, and the form stays
   * available below them.
   *
   * Not a reuse of `ContextMenu`: that one is built out of the command table,
   * and a server is not a command. Bending it to carry both would make the two
   * of them worse.
   */
  import { api, type Site } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    /** Where the button is, so the menu opens under it. */
    x: number;
    y: number;
    onpick: (id: string) => void;
    onquick: () => void;
    onmanage: () => void;
    onclose: () => void;
  }

  let { x, y, onpick, onquick, onmanage, onclose }: Props = $props();

  let rows = $state<Site[] | null>(null);
  let menu = $state<HTMLDivElement | null>(null);

  /**
   * Read on opening rather than held between times.
   *
   * The list lives in another window, and that window can add to it while this
   * one is open. Asking each time is cheap and cannot be stale.
   */
  $effect(() => {
    void api.sites().then((found) => (rows = found));
  });

  /**
   * Folders first as headings, then the entries under them, in the order the
   * site window shows them — one list somebody can learn once.
   */
  let grouped = $derived.by(() => {
    const found = rows ?? [];
    const loose = found.filter((row) => row.folder === "");
    const names = [...new Set(found.map((row) => row.folder).filter(Boolean))].sort((a, b) =>
      a.localeCompare(b),
    );
    return {
      loose,
      folders: names.map((name) => ({
        name,
        sites: found.filter((row) => row.folder === name),
      })),
      total: found.length,
    };
  });

  /**
   * Kept inside the window, and above the button when there is no room below.
   *
   * A menu of thirty servers opened near the bottom edge would otherwise put
   * its last entries where nobody can reach them.
   */
  let position = $derived.by(() => {
    const width = 260;
    const height = Math.min(420, (grouped.total + 2) * 30 + 60);
    const below = y + height + 8 < window.innerHeight;
    return {
      left: Math.max(8, Math.min(x, window.innerWidth - width - 8)),
      top: below ? y : Math.max(8, y - height),
    };
  });

  /** Arrow keys walk the entries, wherever the pointer happens to be. */
  function walk(event: KeyboardEvent): void {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    event.preventDefault();
    const items = [...(menu?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];
    if (items.length === 0) return;
    const at = items.indexOf(document.activeElement as HTMLButtonElement);
    const next = event.key === "ArrowDown" ? at + 1 : at - 1;
    items[(next + items.length) % items.length]?.focus();
  }

  $effect(() => {
    menu?.focus();
  });
</script>

<svelte:window onkeydown={(event) => event.key === "Escape" && onclose()} />

<button
  type="button"
  class="scrim"
  onclick={onclose}
  oncontextmenu={(event) => (event.preventDefault(), onclose())}
  aria-label={t("action.cancel")}
></button>

<div
  class="menu"
  bind:this={menu}
  role="menu"
  tabindex="-1"
  onkeydown={walk}
  style:left="{position.left}px"
  style:top="{position.top}px"
>
  <div class="servers">
    {#if rows === null}
      <p class="hint">{t("connect.loading")}</p>
    {:else if grouped.total === 0}
      <p class="hint">{t("connect.none")}</p>
    {:else}
      {#each grouped.loose as site (site.id)}
        <button type="button" role="menuitem" class="site" onclick={() => onpick(site.id)}>
          <span class="name">{site.name}</span>
          <span class="where">{site.user}@{site.host}</span>
        </button>
      {/each}
      {#each grouped.folders as folder (folder.name)}
        <p class="folder"><Icon name="folder" size={12} />{folder.name}</p>
        {#each folder.sites as site (site.id)}
          <button type="button" role="menuitem" class="site" onclick={() => onpick(site.id)}>
            <span class="name">{site.name}</span>
            <span class="where">{site.user}@{site.host}</span>
          </button>
        {/each}
      {/each}
    {/if}
  </div>

  <div class="rest">
    <button type="button" role="menuitem" onclick={onquick}>
      <Icon name="connect" size={14} />
      {t("connect.quick")}
    </button>
    <button type="button" role="menuitem" onclick={onmanage}>
      <Icon name="sites" size={14} />
      {t("connect.manage")}
    </button>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 60;
    border: 0;
    padding: 0;
    background: transparent;
    cursor: default;
  }

  .menu {
    position: fixed;
    z-index: 61;
    width: 260px;
    display: flex;
    flex-direction: column;
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-radius: 0.7rem;
    box-shadow: var(--shadow-lg);
    padding: 4px;
    outline: none;
  }

  .servers {
    max-height: 320px;
    overflow-y: auto;
  }

  .folder {
    display: flex;
    align-items: center;
    gap: 5px;
    margin: 6px 0 2px;
    padding: 0 8px;
    font-size: 0.68rem;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  .hint {
    margin: 0;
    padding: 10px 8px;
    font-size: 0.78rem;
    color: var(--text-muted);
  }

  button[role="menuitem"] {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    border: 0;
    border-radius: 5px;
    padding: 6px 8px;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 0.82rem;
    text-align: left;
    cursor: pointer;
  }

  button[role="menuitem"]:hover,
  button[role="menuitem"]:focus-visible {
    background: var(--accent-soft);
    outline: none;
  }

  .site {
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
  }

  .name {
    font-weight: 500;
  }

  .where {
    font-size: 0.72rem;
    color: var(--text-muted);
  }

  .rest {
    border-top: 1px solid var(--border);
    margin-top: 4px;
    padding-top: 4px;
  }
</style>
