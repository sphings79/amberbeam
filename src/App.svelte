<script lang="ts">
  import {
    api,
    LOCAL,
    type ConnectRequest,
    type CoreEvent,
    type Site,
    type Unsubscribe,
  } from "./lib/bridge";
  import { locale, LOCALES, setLocale, t } from "./lib/i18n/index.svelte";
  import {
    actionOf,
    hasAnswered,
    restore as restoreKeys,
    saved as savedKeys,
  } from "./lib/keys/index.svelte";
  import { recordEvent } from "./lib/state/log.svelte";
  import { queueState, recordQueueEvent, refreshQueue } from "./lib/state/queue.svelte";
  import { availableUpdate, checkForUpdate, dismissUpdate } from "./lib/state/update.svelte";
  import {
    focusedSide,
    focusPane,
    goUp,
    moveCursor,
    navigate,
    openSession,
    pane,
    reload,
    setHiddenVisible,
    requestCommand,
    setFiltering,
    setTreeVisible,
    startRename,
    switchFocus,
    toggleSelection,
    visibleEntries,
    enter,
    type Side,
  } from "./lib/state/panes.svelte";
  import { ACCENTS, currentAccent, currentTheme, setAccent, setTheme, THEMES } from "./lib/theme/index.svelte";
  import ConflictDialog from "./lib/ui/ConflictDialog.svelte";
  import { trap } from "./lib/ui/trap";
  import KeyboardHelp from "./lib/ui/KeyboardHelp.svelte";
  import KeyboardSettings from "./lib/ui/KeyboardSettings.svelte";
  import KeyboardSetup from "./lib/ui/KeyboardSetup.svelte";
  import SiteManager from "./lib/ui/SiteManager.svelte";
  import Icon from "./lib/ui/Icon.svelte";
  import SettingsDialog from "./lib/ui/SettingsDialog.svelte";
  import { certificateQuestion, hostKeyQuestion } from "./lib/ui/errors";
  import FilePane from "./lib/ui/FilePane.svelte";
  import CertificateDialog from "./lib/ui/CertificateDialog.svelte";
  import HostKeyDialog from "./lib/ui/HostKeyDialog.svelte";
  import QuickConnect from "./lib/ui/QuickConnect.svelte";
  import ServerLog from "./lib/ui/ServerLog.svelte";
  import TransferQueue from "./lib/ui/TransferQueue.svelte";
  import Splitter from "./lib/ui/Splitter.svelte";

  /**
   * Which of the two views this window is.
   *
   * Both are the same bundle: the site manager opens in a window of its own,
   * but building it as a second application would mean two interfaces to keep
   * in step, and they would drift.
   */
  const isSiteManager = api.windowLabel() === "sites";

  /** Heights, split and where each region sits — all kept across restarts. */
  let logHeight = $state(120);
  let queueHeight = $state(96);
  let splitRatio = $state(0.5);
  let settingsOpen = $state(false);
  let transferSettingsOpen = $state(false);

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
      region === "log" ? logPosition === "top" : queuePosition === "top" && !queueHidden,
    ),
  );
  let bottomRegions = $derived(
    (["queue", "log"] as const).filter((region) =>
      region === "log" ? logPosition === "bottom" : queuePosition === "bottom" && !queueHidden,
    ),
  );

  let quickFor = $state<Side | null>(null);
  let connecting = $state(false);
  let connectFailure = $state<unknown>(null);
  /** The request waiting on the user's answer about a server key. */
  let pendingRequest = $state<{
    request: ConnectRequest;
    historyId: string;
    side: Side;
    startPath?: string;
  } | null>(null);

  let hostKey = $derived(hostKeyQuestion(connectFailure));
  /**
   * The keyboard dialog, on the first start and never again unless asked for.
   *
   * Held back until the saved state has been read, or it would flash up for
   * everybody on every start before the answer arrives.
   */
  let stateRead = $state(false);
  let setupOpen = $state(false);
  let helpOpen = $state(false);
  /** The raw command line, which F4 opens and closes. */
  let rawOpen = $state(false);
  let keysOpen = $state(false);
  /** The queue folded away, which is what F8 does. */
  let queueHidden = $state(false);
  /**
   * "Decide later" means later, not never: the dialog stays away for this run
   * and asks again at the next start. Marking it answered would make "later"
   * a word that never arrives.
   */
  let setupPutOff = $state(false);
  let keyboardShown = $derived(
    !isSiteManager && stateRead && (setupOpen || (!hasAnswered() && !setupPutOff)),
  );

  /** A site whose password was never stored, waiting for one. */
  let askingFor = $state<{ request: ConnectRequest; site: Site; side: Side } | null>(null);
  let askedPassword = $state("");
  let certificate = $derived(certificateQuestion(connectFailure));

  /**
   * The first job waiting for an answer about an existing file.
   *
   * One at a time, with "do the same for the others" on the dialog: a queue of
   * two hundred files must not turn into two hundred questions.
   */
  let asking = $derived(queueState().jobs.filter((job) => job.state === "asking"));

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
            keys?: unknown;
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
      restoreKeys(saved?.keys);
      if (saved?.showHidden) {
        setHiddenVisible("left", saved.showHidden.left ?? true);
        setHiddenVisible("right", saved.showHidden.right ?? true);
      }
      // Only now may anything be written back. Opening the gate earlier would
      // let a half-restored state overwrite the whole of it.
      stateRead = true;

      await openSession("left", local, null, null, saved?.leftPath ?? null);
      await openSession("right", local, null, null, null);
    })();
  });

  let unsubscribe: Unsubscribe | null = null;
  $effect(() => {
    void api
      .subscribe((event: CoreEvent) => {
        recordEvent(event);
        recordQueueEvent(event);
      })
      .then((stop) => {
        unsubscribe = stop;
      });
    return () => unsubscribe?.();
  });

  /**
   * Saved on every change, so a crash does not lose the layout.
   *
   * Two guards, and both were learned the hard way. Nothing is written before
   * the saved state has been read, or the defaults this window starts with
   * overwrite what was there. And nothing is written from the site manager at
   * all: it is the same bundle in a second window, with its own copy of every
   * value here — pane sizes it never shows, a keyboard scheme it never loaded —
   * and opening it once was enough to put all of that over the real thing.
   */
  $effect(() => {
    if (isSiteManager || !stateRead) return;
    const state = {
      logHeight,
      queueHeight,
      splitRatio,
      logPosition,
      queuePosition,
      showTree: { left: pane("left").showTree, right: pane("right").showTree },
      showHidden: { left: pane("left").showHidden, right: pane("right").showHidden },
      leftPath: pane("left").endpoint === LOCAL ? pane("left").path : undefined,
      keys: savedKeys(),
    };
    void api.setUiState(state).catch(() => undefined);
  });

  async function attempt(
    request: ConnectRequest,
    historyId: string,
    side: Side,
    startPath?: string,
  ): Promise<void> {
    connecting = true;
    connectFailure = null;
    try {
      const session = await api.connect(request);
      await openSession(
        side,
        session,
        `${request.user}@${request.host}`,
        historyId,
        startPath ?? null,
        // From the session rather than from the request: an exception accepted
        // in an earlier run counts the same, and the mark has to say so.
        session.certificateAccepted,
      );
      quickFor = null;
      pendingRequest = null;
    } catch (failure) {
      connectFailure = failure;
      pendingRequest = { request, historyId, side, startPath };
    } finally {
      connecting = false;
    }
  }

  async function acceptHostKey(fingerprint: string): Promise<void> {
    if (!pendingRequest) return;
    const { request, historyId, side, startPath } = pendingRequest;
    await attempt({ ...request, acceptFingerprint: fingerprint }, historyId, side, startPath);
  }

  /**
   * A site the manager window asked to open.
   *
   * The connection itself happens here rather than there, because every
   * question it can raise has its dialog in this window. A password that was
   * never stored is asked for the same way — one prompt, then the ordinary
   * attempt, so an unknown host key or a certificate still lands where it
   * always does.
   */
  async function openSite(id: string, side: Side): Promise<void> {
    const site = (await api.sites()).find((row) => row.id === id);
    if (!site) return;

    const request: ConnectRequest = {
      endpoint: side,
      siteId: site.id,
      protocol: site.protocol,
      host: site.host,
      port: site.port,
      user: site.user,
      auth: site.auth,
      keyPath: site.keyPath ?? undefined,
      concurrency: site.concurrency,
      retries: site.retries ?? undefined,
      temporaryName: site.temporaryName ?? undefined,
      encryption: site.encryption ?? undefined,
      passive: site.passive ?? undefined,
      latin1: site.latin1 ?? undefined,
      keepAlive: site.keepAlive ?? undefined,
    };

    // Nothing in the store and something needed: ask once, here, rather than
    // let the attempt fail and explain itself afterwards.
    if (site.auth !== "agent" && !site.hasPassword) {
      askingFor = { request, site, side };
      return;
    }
    await attempt(request, `${site.user}@${site.host}`, side, site.remotePath ?? undefined);
  }

  $effect(() => {
    let stop: Unsubscribe | undefined;
    void api.onOpenSite((id, side) => void openSite(id, side)).then((off) => (stop = off));
    return () => stop?.();
  });

  async function acceptCertificate(fingerprint: string): Promise<void> {
    if (!pendingRequest) return;
    const { request, historyId, side, startPath } = pendingRequest;
    await attempt({ ...request, acceptCertificate: fingerprint }, historyId, side, startPath);
  }

  /** Everything a transfer needs to know about where it is going. */
  function other(side: Side): Side {
    return side === "left" ? "right" : "left";
  }

  /** Puts entries from one pane into the queue, bound for the other. */
  async function transfer(from: Side, names: string[]): Promise<void> {
    if (names.length === 0) return;
    const source = pane(from);
    const target = pane(other(from));
    await api.enqueue({
      sourceEndpoint: source.endpoint,
      sourceDirectory: source.path,
      names,
      targetEndpoint: target.endpoint,
      targetDirectory: target.path,
    });
    await refreshQueue();
  }

  /**
   * Files dropped from outside the window.
   *
   * They are always local paths, so the source is the local file system —
   * whichever pane happens to be showing it or not. The pane under the pointer
   * decides where they go.
   */
  // Once, shortly after the window opens, so it does not compete with the
  // first listing for attention or bandwidth.
  $effect(() => {
    const timer = setTimeout(() => void checkForUpdate(), 3000);
    return () => clearTimeout(timer);
  });

  let unsubscribeDrop: Unsubscribe | null = null;
  $effect(() => {
    void api
      .onFileDrop(async (paths, position) => {
        const ratio = window.devicePixelRatio || 1;
        const element = document.elementFromPoint(position.x / ratio, position.y / ratio);
        const pane = element?.closest<HTMLElement>("[data-side]");
        const side = pane?.dataset.side as Side | undefined;
        if (!side) return;
        await dropLocalFiles(side, paths);
      })
      .then((stop) => {
        unsubscribeDrop = stop;
      });
    return () => unsubscribeDrop?.();
  });

  async function dropLocalFiles(side: Side, paths: string[]): Promise<void> {
    if (paths.length === 0) return;
    const target = pane(side);
    // Files arrive as whole paths; the queue wants a directory and names.
    const separator = paths[0]?.includes("\\") ? "\\" : "/";
    const groups = new Map<string, string[]>();
    for (const path of paths) {
      const cut = path.lastIndexOf(separator);
      if (cut <= 0) continue;
      const directory = path.slice(0, cut);
      const name = path.slice(cut + 1);
      groups.set(directory, [...(groups.get(directory) ?? []), name]);
    }
    for (const [directory, names] of groups) {
      await api.enqueue({
        sourceEndpoint: LOCAL,
        sourceDirectory: directory,
        names,
        targetEndpoint: target.endpoint,
        targetDirectory: target.path,
      });
    }
    await refreshQueue();
  }

  /**
   * Shows the folder a job went to.
   *
   * Whichever pane is already on that endpoint gets it; if neither is, the
   * focused one does — going somewhere is more useful than doing nothing.
   */
  async function reveal(endpoint: string, path: string): Promise<void> {
    const separator = path.includes("\\") && !path.startsWith("/") ? "\\" : "/";
    const cut = path.lastIndexOf(separator);
    const directory = cut > 0 ? path.slice(0, cut) : path;
    const side: Side =
      pane("left").endpoint === endpoint
        ? "left"
        : pane("right").endpoint === endpoint
          ? "right"
          : focusedSide();
    if (pane(side).endpoint !== endpoint) return;
    focusPane(side);
    await navigate(side, directory);
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
    // The site manager is the same bundle in another window and brings its own
    // keys; this handler belongs to the panes.
    if (isSiteManager) return;

    const target = event.target as HTMLElement | null;
    if (target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) return;
    if (quickFor || hostKey || certificate) return;
    // A dialog about keys is open. Handing it Tab and F5 while it waits for
    // them is not a detail — it is the one moment those keys mean something
    // else entirely, and while somebody is assigning a key it must not also
    // fire the action they are assigning it away from.
    if (keyboardShown || keysOpen || helpOpen) return;

    const side = focusedSide();
    const rows = visibleEntries(side);
    const view = pane(side);

    // Moving about a list is not a matter of taste and is not in the table:
    // arrows move, Enter opens, Backspace goes up. Those are what a list is,
    // and a scheme that rebound them would be a scheme nobody could use.
    //
    // Bare keys only. The Mac scheme puts delete on ⌘⌫ and starting the queue
    // on ⌘↵, and a block that caught those regardless of the modifier would
    // quietly go up a directory instead.
    const bare = !event.metaKey && !event.ctrlKey && !event.altKey;
    switch (bare ? event.key : "") {
      case "ArrowDown":
        event.preventDefault();
        moveCursor(side, 1, rows.length);
        return;
      case "ArrowUp":
        event.preventDefault();
        moveCursor(side, -1, rows.length);
        return;
      case "PageDown":
        event.preventDefault();
        moveCursor(side, 15, rows.length);
        return;
      case "PageUp":
        event.preventDefault();
        moveCursor(side, -15, rows.length);
        return;
      case "Home":
        event.preventDefault();
        moveCursor(side, -rows.length, rows.length);
        return;
      case "End":
        event.preventDefault();
        moveCursor(side, rows.length, rows.length);
        return;
      case "Enter": {
        const entry = rows[view.cursor];
        if (entry) {
          event.preventDefault();
          await enter(side, entry);
        }
        return;
      }
      case "Backspace":
        event.preventDefault();
        await goUp(side);
        return;
      case " ":
      case "Insert": {
        const entry = rows[view.cursor];
        if (entry) {
          event.preventDefault();
          toggleSelection(side, entry.name);
          moveCursor(side, 1, rows.length);
        }
        return;
      }
      default:
        break;
    }

    // Everything else is a matter of taste, and comes from the table the user
    // can change.
    const action = actionOf(event);
    if (!action) return;
    event.preventDefault();

    switch (action) {
      case "switch-focus":
        switchFocus();
        break;
      case "refresh":
        await reload(side);
        break;
      case "rename": {
        // Renaming happens in the row itself; the pane picks it up from here.
        const entry = rows[view.cursor];
        if (entry) startRename(side, entry.name);
        break;
      }
      case "sites":
        await api.openSiteManager();
        break;
      case "settings":
        transferSettingsOpen = true;
        break;
      case "toggle-hidden":
        setHiddenVisible(side, !view.showHidden);
        break;
      case "toggle-tree":
        setTreeVisible(side, !view.showTree);
        break;
      case "connect-toggle":
        if (view.endpoint === LOCAL) quickFor = side;
        else await disconnect(side);
        break;
      case "help":
        helpOpen = true;
        break;
      case "raw":
        rawOpen = !rawOpen;
        break;
      case "search":
        setFiltering(side, !view.filtering);
        break;
      case "fullscreen":
        await api.toggleFullscreen().catch(() => undefined);
        break;
      case "queue-toggle":
        queueHidden = !queueHidden;
        break;
      case "queue-start":
        await api.queuePause(false);
        await refreshQueue();
        break;
      // Everything a pane already offers goes through the pane, not around it.
      case "delete":
      case "transfer":
      case "new-file":
      case "new-folder":
      case "permissions":
        requestCommand(side, action);
        break;
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

{#if isSiteManager}
  <SiteManager />
{:else}
<div class="window">
  {#each topRegions as region (region)}
    {#if region === "log"}
      <div class="region log" style:height="{logHeight}px">
        <ServerLog
          endpoint={pane(focusedSide()).endpoint}
          protocol={pane(focusedSide()).protocol}
          raw={rawOpen}
          onclose={() => (rawOpen = false)}
        />
      </div>
      <Splitter
        direction="horizontal"
        label={t("splitter.log")}
        onmove={(d) => (logHeight = Math.min(400, Math.max(60, logHeight + d)))}
      />
    {:else}
      <div class="region queue" style:height="{queueHeight}px"><TransferQueue onreveal={reveal} /></div>
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
        ontransfer={(names) => transfer("left", names)}
        onreceive={(from, names) => transfer(from, names)}
      />
    </div>
    <Splitter direction="vertical" label={t("splitter.panes")} onmove={resizeSplit} />
    <div class="half" style:flex="{1 - splitRatio}">
      <FilePane
        side="right"
        onquickconnect={() => ((quickFor = "right"), (connectFailure = null))}
        ondisconnect={() => disconnect("right")}
        ontransfer={(names) => transfer("right", names)}
        onreceive={(from, names) => transfer(from, names)}
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
      <div class="region log" style:height="{logHeight}px">
        <ServerLog
          endpoint={pane(focusedSide()).endpoint}
          protocol={pane(focusedSide()).protocol}
          raw={rawOpen}
          onclose={() => (rawOpen = false)}
        />
      </div>
    {:else}
      <Splitter
        direction="horizontal"
        label={t("splitter.queue")}
        onmove={(d) => (queueHeight = Math.min(400, Math.max(60, queueHeight - d)))}
      />
      <div class="region queue" style:height="{queueHeight}px"><TransferQueue onreveal={reveal} /></div>
    {/if}
  {/each}

  <footer>
    <span class="keys mono">{t("status.keys")}</span>
    <div class="actions">
      {#if availableUpdate()}
        {@const release = availableUpdate()}
        {#if release}
          <button
            type="button"
            class="support update"
            onclick={() => api.openUrl(release.url)}
            title={t("update.hint", { version: release.version })}
          >
            <Icon name="star" size={14} />
            {t("update.available", { version: release.version })}
          </button>
          <button
            type="button"
            class="dismiss"
            onclick={dismissUpdate}
            aria-label={t("action.cancel")}
          >
            ×
          </button>
        {/if}
      {/if}
      <button type="button" class="settings" onclick={() => (settingsOpen = !settingsOpen)}>
        {t("appearance.title")}
      </button>
      <button type="button" class="settings" onclick={() => (transferSettingsOpen = true)}>
        {t("settings.title")}
      </button>
      <button type="button" class="settings" onclick={() => (helpOpen = true)}>
        {t("help.title")}
      </button>
      <button type="button" class="settings" onclick={() => void api.openSiteManager()}>
        {t("sites.title")}
      </button>
      <button
        type="button"
        class="support star"
        onclick={() => api.openUrl("https://github.com/sphings79/amberbeam")}
        title={t("support.star.hint")}
      >
        <Icon name="star" size={14} />
        {t("support.star")}
      </button>
      <button
        type="button"
        class="support coffee"
        onclick={() => api.openUrl("https://buymeacoffee.com/sphings")}
        title={t("support.coffee.hint")}
      >
        <Icon name="coffee" size={14} />
        {t("support.coffee")}
      </button>
    </div>
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
    failure={hostKey || certificate ? null : connectFailure}
    onconnect={(request, historyId) => attempt(request, historyId, quickFor ?? "left")}
    onclose={() => ((quickFor = null), (connectFailure = null), (pendingRequest = null))}
  />
{/if}

{#if transferSettingsOpen}
  <SettingsDialog onclose={() => (transferSettingsOpen = false)} />
{/if}

{#if asking.length > 0 && asking[0]}
  <ConflictDialog
    job={asking[0]}
    waiting={asking.length}
    ondecide={async (policy, forAll) => {
      const id = asking[0]?.id;
      if (id) await api.queueDecide(id, policy, forAll);
      await refreshQueue();
    }}
    onskipall={async () => {
      const id = asking[0]?.id;
      if (id) await api.queueDecide(id, "skip", true);
      await refreshQueue();
    }}
  />
{/if}

{#if helpOpen}
  <KeyboardHelp
    onclose={() => (helpOpen = false)}
    onsetup={() => {
      helpOpen = false;
      keysOpen = true;
    }}
  />
{/if}

{#if keysOpen}
  <KeyboardSettings onclose={() => (keysOpen = false)} />
{/if}

{#if keyboardShown}
  <KeyboardSetup
    onclose={() => {
      setupOpen = false;
      setupPutOff = true;
    }}
  />
{/if}

{#if askingFor}
  <div class="backdrop" role="presentation">
    <div class="ask" use:trap role="dialog" aria-modal="true">
      <h2>{t("sites.password.title", { name: askingFor.site.name })}</h2>
      <p>{t("sites.password.body")}</p>
      <input
        type="password"
        bind:value={askedPassword}
        autocomplete="off"
        autocapitalize="off"
        autocorrect="off"
        spellcheck="false"
      />
      <div class="ask-actions">
        <button
          type="button"
          onclick={() => {
            askingFor = null;
            askedPassword = "";
          }}
        >
          {t("action.cancel")}
        </button>
        <button
          type="button"
          class="primary"
          onclick={() => {
            const waiting = askingFor;
            const secret = askedPassword;
            askingFor = null;
            askedPassword = "";
            if (!waiting) return;
            const field = waiting.site.auth === "key-file" ? "passphrase" : "password";
            void attempt(
              { ...waiting.request, [field]: secret },
              `${waiting.site.user}@${waiting.site.host}`,
              waiting.side,
              waiting.site.remotePath ?? undefined,
            );
          }}
        >
          {t("quick.connect")}
        </button>
      </div>
    </div>
  </div>
{/if}

{#if certificate}
  <CertificateDialog
    question={certificate}
    onaccept={acceptCertificate}
    oncancel={() => {
      connectFailure = null;
      pendingRequest = null;
    }}
  />
{/if}

{#if hostKey}
  <HostKeyDialog
    question={hostKey}
    onaccept={acceptHostKey}
    oncancel={() => ((connectFailure = null), (pendingRequest = null))}
  />
{/if}
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 45%);
    display: grid;
    place-items: center;
    z-index: 50;
  }

  .ask {
    width: min(420px, 92vw);
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-radius: 1rem;
    box-shadow: var(--shadow-lg);
    padding: 20px 22px;
  }

  .ask h2 {
    margin: 0 0 8px;
    font-size: 1rem;
  }

  .ask p {
    margin: 0 0 12px;
    font-size: 0.85rem;
    color: var(--text-muted);
  }

  .ask input {
    font: inherit;
    font-size: 0.86rem;
    padding: 6px 8px;
    border-radius: 0.4rem;
    border: 1px solid var(--border-strong);
    background: var(--surface-0);
    color: var(--text);
    width: 100%;
  }

  .ask-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 14px;
  }

  .ask-actions button {
    font: inherit;
    font-size: 0.86rem;
    padding: 6px 16px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    background: var(--surface-1);
    color: var(--text);
    cursor: default;
  }

  .ask-actions button.primary {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }

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

  /* The four stay together and stay on the right. The key hint gives way
     instead: a reminder that is cut short still reminds, a row of buttons that
     wraps onto a second line looks like a mistake. */
  .actions {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 8px;
    flex: none;
  }

  .keys {
    font-size: 0.7rem;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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

  /* Loud enough to be found, quiet enough not to nag: the accent colour and a
     filled pill, but no animation and no growing on hover. */
  .support {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 0.76rem;
    font-weight: 600;
    padding: 3px 11px;
    border-radius: 999px;
    border: 1px solid var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .support:hover {
    background: var(--accent);
    color: var(--accent-text);
  }

  .support.coffee {
    border-color: var(--warn);
    background: var(--warn-soft);
    color: var(--warn);
  }

  .support.coffee:hover {
    background: var(--warn);
    color: var(--surface-1);
  }

  .support.update {
    border-color: var(--ok);
    background: var(--ok-soft);
    color: var(--ok);
  }

  .support.update:hover {
    background: var(--ok);
    color: var(--surface-1);
  }

  .dismiss {
    border: none;
    background: none;
    color: var(--text-faint);
    padding: 0 2px;
    font-size: 0.9rem;
  }
</style>
