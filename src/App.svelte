<script lang="ts">
  import { api, LOCAL, type ConnectRequest, type CoreEvent, type Unsubscribe } from "./lib/bridge";
  import { locale, LOCALES, setLocale, t } from "./lib/i18n/index.svelte";
  import { recordEvent } from "./lib/state/log.svelte";
  import {
    focusedSide,
    goUp,
    moveCursor,
    openSession,
    pane,
    reload,
    setHiddenVisible,
    setTreeVisible,
    startRename,
    switchFocus,
    toggleSelection,
    visibleEntries,
    enter,
    type Side,
  } from "./lib/state/panes.svelte";
  import { ACCENTS, currentAccent, currentTheme, setAccent, setTheme, THEMES } from "./lib/theme/index.svelte";
  import { hostKeyQuestion } from "./lib/ui/errors";
  import FilePane from "./lib/ui/FilePane.svelte";
  import HostKeyDialog from "./lib/ui/HostKeyDialog.svelte";
  import QuickConnect from "./lib/ui/QuickConnect.svelte";
  import ServerLog from "./lib/ui/ServerLog.svelte";
  import Splitter from "./lib/ui/Splitter.svelte";

  /** Heights, split and where each region sits — all kept across restarts. */
  let logHeight = $state(120);
  let queueHeight = $state(96);
  let splitRatio = $state(0.5);
  let settingsOpen = $state(false);

  /**
   * Where the server log and the queue sit relative to the file panes.
   *
   * The tradition puts the log on top and the queue at the bottom, and that is
   * the default. It is a preference, not a law, so it is settable: with both on
   * the same side the log is the outer one, which keeps it out of the way of
   * the panes.
   */
  type Position = "top" | "bottom";
  let logPosition = $state<Position>("bottom");
  let queuePosition = $state<Position>("bottom");

  let topRegions = $derived(
    (["log", "queue"] as const).filter((region) =>
      region === "log" ? logPosition === "top" : queuePosition === "top",
    ),
  );
  let bottomRegions = $derived(
    (["queue", "log"] as const).filter((region) =>
      region === "log" ? logPosition === "bottom" : queuePosition === "bottom",
    ),
  );

  let quickFor = $state<Side | null>(null);
  let connecting = $state(false);
  let connectFailure = $state<unknown>(null);
  /** The request waiting on the user's answer about a server key. */
  let pendingRequest = $state<{ request: ConnectRequest; historyId: string; side: Side } | null>(null);

  let hostKey = $derived(hostKeyQuestion(connectFailure));

  // Both panes start on the local file system. Which side later becomes a
  // server is the user's business; nothing here treats one as privileged.
  $effect(() => {
    void (async () => {
      const local = await api.localSession();
      const saved = (await api.uiState().catch(() => null)) as
        | {
            logHeight?: number;
            queueHeight?: number;
            splitRatio?: number;
            leftPath?: string;
            logPosition?: Position;
            queuePosition?: Position;
            showTree?: { left?: boolean; right?: boolean };
            showHidden?: { left?: boolean; right?: boolean };
          }
        | null;
      if (saved?.logHeight) logHeight = saved.logHeight;
      if (saved?.queueHeight) queueHeight = saved.queueHeight;
      if (saved?.splitRatio) splitRatio = saved.splitRatio;
      if (saved?.logPosition) logPosition = saved.logPosition;
      if (saved?.queuePosition) queuePosition = saved.queuePosition;
      if (saved?.showTree) {
        setTreeVisible("left", saved.showTree.left ?? true);
        setTreeVisible("right", saved.showTree.right ?? true);
      }
      if (saved?.showHidden) {
        setHiddenVisible("left", saved.showHidden.left ?? true);
        setHiddenVisible("right", saved.showHidden.right ?? true);
      }
      await openSession("left", local, null, null, saved?.leftPath ?? null);
      await openSession("right", local, null, null, null);
    })();
  });

  let unsubscribe: Unsubscribe | null = null;
  $effect(() => {
    void api.subscribe((event: CoreEvent) => recordEvent(event)).then((stop) => {
      unsubscribe = stop;
    });
    return () => unsubscribe?.();
  });

  /** Saved on every change, so a crash does not lose the layout. */
  $effect(() => {
    const state = {
      logHeight,
      queueHeight,
      splitRatio,
      logPosition,
      queuePosition,
      showTree: { left: pane("left").showTree, right: pane("right").showTree },
      showHidden: { left: pane("left").showHidden, right: pane("right").showHidden },
      leftPath: pane("left").endpoint === LOCAL ? pane("left").path : undefined,
    };
    void api.setUiState(state).catch(() => undefined);
  });

  async function attempt(request: ConnectRequest, historyId: string, side: Side): Promise<void> {
    connecting = true;
    connectFailure = null;
    try {
      const session = await api.connect(request);
      await openSession(side, session, `${request.user}@${request.host}`, historyId);
      quickFor = null;
      pendingRequest = null;
    } catch (failure) {
      connectFailure = failure;
      pendingRequest = { request, historyId, side };
    } finally {
      connecting = false;
    }
  }

  async function acceptHostKey(fingerprint: string): Promise<void> {
    if (!pendingRequest) return;
    const { request, historyId, side } = pendingRequest;
    await attempt({ ...request, acceptFingerprint: fingerprint }, historyId, side);
  }

  async function disconnect(side: Side): Promise<void> {
    const endpoint = pane(side).endpoint;
    if (endpoint === LOCAL) return;
    await api.disconnect(endpoint);
    const local = await api.localSession();
    await openSession(side, local, null, null, null);
  }

  /**
   * The keys of a two pane client. F5 and F6 are bound because they cost
   * nothing; whether they arrive at all depends on the system settings, and
   * AmberBeam does not reach below the operating system to catch them. The
   * first run dialog that explains this belongs to M5.
   */
  async function onKey(event: KeyboardEvent): Promise<void> {
    const target = event.target as HTMLElement | null;
    if (target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) return;
    if (quickFor || hostKey) return;

    const side = focusedSide();
    const rows = visibleEntries(side);
    const view = pane(side);

    switch (event.key) {
      case "Tab":
      case "F6":
        event.preventDefault();
        switchFocus();
        break;
      case "ArrowDown":
        event.preventDefault();
        moveCursor(side, 1, rows.length);
        break;
      case "ArrowUp":
        event.preventDefault();
        moveCursor(side, -1, rows.length);
        break;
      case "PageDown":
        event.preventDefault();
        moveCursor(side, 15, rows.length);
        break;
      case "PageUp":
        event.preventDefault();
        moveCursor(side, -15, rows.length);
        break;
      case "Home":
        event.preventDefault();
        moveCursor(side, -rows.length, rows.length);
        break;
      case "End":
        event.preventDefault();
        moveCursor(side, rows.length, rows.length);
        break;
      case "Enter": {
        const entry = rows[view.cursor];
        if (entry) {
          event.preventDefault();
          await enter(side, entry);
        }
        break;
      }
      case "Backspace":
        event.preventDefault();
        await goUp(side);
        break;
      case " ":
      case "Insert": {
        const entry = rows[view.cursor];
        if (entry) {
          event.preventDefault();
          toggleSelection(side, entry.name);
          moveCursor(side, 1, rows.length);
        }
        break;
      }
      case "F5":
        event.preventDefault();
        await reload(side);
        break;
      case "F2": {
        // Renaming happens in the row itself; the pane picks it up from here.
        const entry = rows[view.cursor];
        if (entry) {
          event.preventDefault();
          startRename(side, entry.name);
        }
        break;
      }
      default:
        break;
    }
  }

  function resizeSplit(delta: number): void {
    const width = window.innerWidth || 1280;
    splitRatio = Math.min(0.85, Math.max(0.15, splitRatio + delta / width));
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="window">
  {#each topRegions as region (region)}
    {#if region === "log"}
      <div class="region log" style:height="{logHeight}px"><ServerLog /></div>
      <Splitter
        direction="horizontal"
        label={t("splitter.log")}
        onmove={(d) => (logHeight = Math.min(400, Math.max(60, logHeight + d)))}
      />
    {:else}
      <div class="region queue" style:height="{queueHeight}px">
        <header>
          <span class="title">{t("queue.title")}</span>
          <span class="spacer"></span>
          <span class="soon">{t("queue.soon")}</span>
        </header>
        <p class="empty">{t("queue.empty")}</p>
      </div>
      <Splitter
        direction="horizontal"
        label={t("splitter.queue")}
        onmove={(d) => (queueHeight = Math.min(400, Math.max(60, queueHeight + d)))}
      />
    {/if}
  {/each}

  <div class="panes">
    <div class="half" style:flex="{splitRatio}">
      <FilePane
        side="left"
        onquickconnect={() => ((quickFor = "left"), (connectFailure = null))}
        ondisconnect={() => disconnect("left")}
      />
    </div>
    <Splitter direction="vertical" label={t("splitter.panes")} onmove={resizeSplit} />
    <div class="half" style:flex="{1 - splitRatio}">
      <FilePane
        side="right"
        onquickconnect={() => ((quickFor = "right"), (connectFailure = null))}
        ondisconnect={() => disconnect("right")}
      />
    </div>
  </div>

  {#each bottomRegions as region (region)}
    {#if region === "log"}
      <Splitter
        direction="horizontal"
        label={t("splitter.log")}
        onmove={(d) => (logHeight = Math.min(400, Math.max(60, logHeight - d)))}
      />
      <div class="region log" style:height="{logHeight}px"><ServerLog /></div>
    {:else}
      <Splitter
        direction="horizontal"
        label={t("splitter.queue")}
        onmove={(d) => (queueHeight = Math.min(400, Math.max(60, queueHeight - d)))}
      />
      <div class="region queue" style:height="{queueHeight}px">
        <header>
          <span class="title">{t("queue.title")}</span>
          <span class="spacer"></span>
          <span class="soon">{t("queue.soon")}</span>
        </header>
        <p class="empty">{t("queue.empty")}</p>
      </div>
    {/if}
  {/each}

  <footer>
    <span class="keys mono">{t("status.keys")}</span>
    <span class="spacer"></span>
    <button type="button" class="settings" onclick={() => (settingsOpen = !settingsOpen)}>
      {t("appearance.title")}
    </button>
  </footer>

  {#if settingsOpen}
    <div class="settings-bar">
      <span class="label">{t("layout.log")}</span>
      {#each ["top", "bottom"] as const as where (where)}
        <button type="button" class:active={logPosition === where} onclick={() => (logPosition = where)}>
          {t(`layout.${where}`)}
        </button>
      {/each}
      <span class="label">{t("layout.queue")}</span>
      {#each ["top", "bottom"] as const as where (where)}
        <button type="button" class:active={queuePosition === where} onclick={() => (queuePosition = where)}>
          {t(`layout.${where}`)}
        </button>
      {/each}
    </div>
    <div class="settings-bar">
      <span class="label">{t("appearance.theme")}</span>
      {#each THEMES as candidate (candidate)}
        <button type="button" class:active={currentTheme() === candidate} onclick={() => setTheme(candidate)}>
          {t(`theme.${candidate}`)}
        </button>
      {/each}
      <span class="label">{t("appearance.accent")}</span>
      {#each ACCENTS as candidate (candidate)}
        <button
          type="button"
          class="swatch"
          class:active={currentAccent() === candidate}
          data-accent={candidate}
          aria-label={t(`accent.${candidate}`)}
          onclick={() => setAccent(candidate)}
        ></button>
      {/each}
      <span class="label">{t("language.title")}</span>
      {#each LOCALES as candidate (candidate)}
        <button type="button" class:active={locale() === candidate} onclick={() => setLocale(candidate)}>
          {t(`language.${candidate}`)}
        </button>
      {/each}
    </div>
  {/if}
</div>

{#if quickFor}
  <QuickConnect
    endpoint={quickFor === "left" ? "left-remote" : "right-remote"}
    busy={connecting}
    failure={hostKey ? null : connectFailure}
    onconnect={(request, historyId) => attempt(request, historyId, quickFor ?? "left")}
    onclose={() => ((quickFor = null), (connectFailure = null), (pendingRequest = null))}
  />
{/if}

{#if hostKey}
  <HostKeyDialog
    question={hostKey}
    onaccept={acceptHostKey}
    oncancel={() => ((connectFailure = null), (pendingRequest = null))}
  />
{/if}

<style>
  .window {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .region {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: none;
  }

  .panes {
    display: flex;
    flex: 1;
    min-height: 0;
  }

  .half {
    display: flex;
    min-width: 0;
  }

  .queue {
    background: var(--surface-1);
  }

  .queue header {
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

  .soon {
    font-size: 0.68rem;
    color: var(--accent);
    background: var(--accent-soft);
    padding: 1px 8px;
    border-radius: 999px;
  }

  .queue .empty {
    margin: 0;
    padding: 12px;
    font-size: 0.78rem;
    color: var(--text-faint);
  }

  footer,
  .settings-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 10px;
    background: var(--surface-2);
    border-top: 1px solid var(--border);
    font-size: 0.72rem;
    color: var(--text-faint);
    flex: none;
  }

  .keys {
    font-size: 0.7rem;
  }

  .settings-bar .label {
    font-size: 0.64rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-faint);
    margin-left: 6px;
  }

  button {
    font: inherit;
    font-size: 0.74rem;
    padding: 2px 10px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    background: var(--surface-1);
    color: var(--text);
    cursor: default;
  }

  button:hover {
    background: var(--surface-3);
  }

  button.active {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }

  button.swatch {
    width: 18px;
    height: 18px;
    padding: 0;
    border-radius: 999px;
    background: var(--accent);
    border: 2px solid transparent;
    box-shadow: 0 0 0 1px var(--border-strong);
  }

  button.swatch.active {
    box-shadow: 0 0 0 2px var(--accent);
  }

  .settings {
    color: var(--text-muted);
  }
</style>
