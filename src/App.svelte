<script lang="ts">
  import {
    api,
    connectedTo,
    isNotSignedIn,
    useService,
    useThisMachine,
    LOCAL,
    type ConnectRequest,
    type CoreEvent,
    type Site,
    type Unsubscribe,
  } from "./lib/bridge";
  import { resetLocale, t } from "./lib/i18n/index.svelte";
  import {
    DEFAULT_ACCENT,
    DEFAULT_SIZE,
    DEFAULT_THEME,
    setAccent,
    setSize,
    setTheme,
  } from "./lib/theme/index.svelte";
  import {
    actionOf,
    hasAnswered,
    label,
    restore as restoreKeys,
    saved as savedKeys,
    setupApplies,
  } from "./lib/keys/index.svelte";
  import { clearLog, note, recordEvent } from "./lib/state/log.svelte";
  import { queueState, recordQueueEvent, refreshQueue } from "./lib/state/queue.svelte";
  import {
    availableUpdate,
    checkForUpdate,
    updateCheckedAt,
    updateStatus,
  } from "./lib/state/update.svelte";
  import {
    focusedSide,
    focusPane,
    goUp,
    moveCursor,
    selectTo,
    navigate,
    openSession,
    pane,
    reload,
    setHiddenVisible,
    requestCommand,
    setFiltering,
    setBrowsingTogether,
    browsingTogether,
    whenDirectoryMissing,
    setTreeVisible,
    setTreeWidth,
    TREE_DEFAULT,
    startRename,
    switchFocus,
    toggleSelection,
    visibleEntries,
    enter,
    type Side,
  } from "./lib/state/panes.svelte";
  import ConflictDialog from "./lib/ui/ConflictDialog.svelte";
  import { tips } from "./lib/ui/tips";
  import { trap } from "./lib/ui/trap";
  import KeyboardHelp from "./lib/ui/KeyboardHelp.svelte";
  import KeyboardSettings from "./lib/ui/KeyboardSettings.svelte";
  import KeyboardSetup from "./lib/ui/KeyboardSetup.svelte";
  import AppearanceDialog from "./lib/ui/AppearanceDialog.svelte";
  import ConnectMenu from "./lib/ui/ConnectMenu.svelte";
  import ServiceDialog from "./lib/ui/ServiceDialog.svelte";
  import SignIn from "./lib/ui/SignIn.svelte";
  import UpdateDialog from "./lib/ui/UpdateDialog.svelte";
  import Editor from "./lib/ui/Editor.svelte";
  import SiteManager from "./lib/ui/SiteManager.svelte";
  import Icon from "./lib/ui/Icon.svelte";
  import SettingsDialog from "./lib/ui/SettingsDialog.svelte";
  import { certificateQuestion, describe, hostKeyQuestion } from "./lib/ui/errors";
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

  /**
   * The file this window is for, when it is an editor window.
   *
   * The identifier rides in the window's label rather than in its URL, for the
   * same reason the site manager's view does: a URL here is a path, and a
   * question mark is illegal in one on Windows.
   */
  const editorFor = api.windowLabel().startsWith("editor:")
    ? api.windowLabel().slice("editor:".length)
    : null;

  /**
   * A file being edited over this page, where a window of its own is not
   * possible — which is a browser.
   */
  let editingHere = $state<string | null>(null);

  /**
   * Takes a copy of a server's file and puts it in front of somebody.
   *
   * Whether the copy may be taken at all is the core's answer and its refusal
   * says why. What opens it is the table's answer. Nothing is decided here
   * beyond falling back when the answer cannot be carried out — a browser has
   * no program of this person's to start, and an editor that does not appear
   * is worse than one they did not pick.
   */
  async function edit(path: string, side: Side): Promise<void> {
    const started = await api.startEdit(pane(side).endpoint, path);
    const rule = await api.howToEdit(started.name);

    if (rule && rule.openWith !== "own") {
      const program = rule.openWith === "program" ? rule.program : null;
      if (await api.openWith(started.localPath, program)) return;
      // Said rather than quietly substituted. Somebody who chose a program and
      // got something else is owed the reason, and in a browser the reason is
      // that the program would start on the other machine.
      note(t("editing.here-only", { name: started.name }));
    }

    if (!(await api.openEditor(started.id, started.name))) {
      editingHere = started.id;
    }
  }

  /**
   * A file another program is editing, whose write-back the server refused.
   *
   * Asked here because there is no window of ours to ask in: the file is open
   * somewhere else entirely, and the person is looking at that. Nothing was
   * written, and the copy still holds what they saved.
   */
  let editChanged = $state<{ id: string; name: string; path: string } | null>(null);

  /**
   * The copies still lying about as the program closes, and the answer nobody
   * has given yet.
   *
   * Asked rather than assumed, because the two answers are both reasonable and
   * only one of them can be taken back. A copy thrown away is gone with
   * whatever was typed into it and never saved; a copy kept is somebody's file
   * sitting unencrypted in a temporary directory.
   */
  let leftBehind = $state<{ names: string[]; answer: (deleteCopies: boolean) => void } | null>(
    null,
  );

  $effect(() => {
    // Only this window asks. The site manager and the editor windows close on
    // their own, and three windows asking the same question is two too many.
    if (isSiteManager || editorFor) return;
    let stop: Unsubscribe | undefined;
    void api
      .onClosing(async () => {
        const open = await api.openEdits().catch(() => []);
        if (open.length === 0) return true;
        return await new Promise<boolean>((settle) => {
          leftBehind = {
            names: open.map((edit) => edit.name),
            answer: (deleteCopies) => {
              leftBehind = null;
              void api.endEdits(deleteCopies).catch(() => undefined);
              settle(true);
            },
          };
        });
      })
      .then((off) => (stop = off));
    return () => stop?.();
  });

  /** Heights, split and where each region sits — all kept across restarts. */
  /**
   * What the window looks like before anybody drags anything.
   *
   * Named because two places need them: the state below, and the reset button
   * in the appearance settings. A default written out twice is a default that
   * disagrees with itself the first time one copy is changed.
   */
  const START: {
    logHeight: number;
    queueHeight: number;
    splitRatio: number;
    logPosition: Position;
    queuePosition: Position;
  } = {
    logHeight: 120,
    queueHeight: 96,
    splitRatio: 0.5,
    logPosition: "bottom",
    queuePosition: "bottom",
  };

  let logHeight = $state(START.logHeight);
  let queueHeight = $state(START.queueHeight);
  let splitRatio = $state(START.splitRatio);
  let settingsOpen = $state(false);
  let transferSettingsOpen = $state(false);

  /**
   * Where the server log and the queue sit relative to the file panes.
   *
   * The older Windows clients put the log above the panes. Both start below
   * them here, and that is a decision rather than an oversight: the two panes
   * are what somebody looks at all day, and anything above them pushes the
   * thing they came for further down the window. A log is glanced at when
   * something goes wrong, not read while working.
   *
   * A preference, not a law, so both are settable — and with both on the same
   * side the log is the outer one, which keeps it out of the way of the panes.
   */
  type Position = "top" | "bottom";
  let logPosition = $state<Position>(START.logPosition);
  let queuePosition = $state<Position>(START.queuePosition);
  /**
   * Kept apart from the position rather than folded into it as a third value.
   *
   * Hiding a region and moving it are different decisions, and somebody who
   * puts the log away still meant it to come back where they had it. A single
   * "off" would forget that.
   */
  let logHidden = $state(false);

  let topRegions = $derived(
    (["log", "queue"] as const).filter((region) =>
      region === "log"
        ? logPosition === "top" && !logHidden
        : queuePosition === "top" && !queueHidden,
    ),
  );
  let bottomRegions = $derived(
    (["queue", "log"] as const).filter((region) =>
      region === "log"
        ? logPosition === "bottom" && !logHidden
        : queuePosition === "bottom" && !queueHidden,
    ),
  );

  /**
   * Everything the window remembers about how it looks, back to the start.
   *
   * Sizes, sides, colours, language, and what is hidden. Not what anybody has
   * connected to or saved: this is a reset of the furniture, not of the work,
   * and a button in the appearance settings must not quietly be more than it
   * says.
   */
  function resetLook(): void {
    logHeight = START.logHeight;
    queueHeight = START.queueHeight;
    splitRatio = START.splitRatio;
    logPosition = START.logPosition;
    queuePosition = START.queuePosition;
    logHidden = false;
    queueHidden = false;
    rawOpen = false;
    for (const side of ["left", "right"] as const) {
      setTreeVisible(side, true);
      setTreeWidth(side, TREE_DEFAULT);
    }
    setTheme(DEFAULT_THEME);
    setAccent(DEFAULT_ACCENT);
    setSize(DEFAULT_SIZE);
    resetLocale();
  }

  /** The three answers the settings offer for one region, as one value. */
  type Where = Position | "off";

  function place(position: Position, hidden: boolean): Where {
    return hidden ? "off" : position;
  }

  let quickFor = $state<Side | null>(null);
  /** The release the check found, if it found one. */
  let newer = $derived(availableUpdate());
  /** Whether its notes are being read. */
  let updateOpen = $state(false);

  /**
   * What the update button says when the pointer rests on it.
   *
   * When a check has run, the time it ran is the useful thing: it turns
   * "Aktuell" from a claim into a claim with a date on it.
   */
  function lastCheck(): string {
    const at = updateCheckedAt();
    if (at === null) return t("update.check.hint");
    return t("update.checked", {
      time: new Date(at).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" }),
    });
  }

  /** The connect menu, and which side asked for it. */
  let connectMenu = $state<{ side: Side; x: number; y: number } | null>(null);

  /**
   * Closes the menu and then does the thing it was opened for.
   *
   * In that order, and the side read out first: closing tears down the block
   * the menu's own state lives in, so anything still reading from it
   * afterwards is reading something that has been taken away. That is not a
   * subtle failure — the menu shut and nothing happened at all.
   */
  function closeMenu(then: (side: Side) => void): void {
    const side = connectMenu?.side;
    connectMenu = null;
    if (side) then(side);
  }

  /**
   * Opens the menu where the button for that side is.
   *
   * The key and the button have to arrive at the same place, so the position
   * comes from the button either way rather than being guessed at here. If it
   * is not on screen — a disconnected pane has a different button — the menu
   * falls back to the top left, which is where that pane begins.
   */
  function openConnectMenu(side: Side): void {
    const button = document.querySelector<HTMLElement>(`[data-connect="${side}"]`);
    const box = button?.getBoundingClientRect();
    connectMenu = box
      ? { side, x: box.left, y: box.bottom + 4 }
      : { side, x: side === "left" ? 16 : window.innerWidth / 2, y: 80 };
  }
  let connecting = $state(false);
  let connectFailure = $state<unknown>(null);
  /** The request waiting on the user's answer about a server key. */
  let pendingRequest = $state<{
    request: ConnectRequest;
    historyId: string;
    side: Side;
    startPath?: string;
    siteId?: string;
    resume?: string;
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
    !isSiteManager &&
      stateRead &&
      setupApplies() &&
      (setupOpen || (!hasAnswered() && !setupPutOff)),
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
      try {
        await api.coreInfo();
      } catch (problem) {
        if (isNotSignedIn(problem)) {
          wantsPassword = true;
          return;
        }
      }
      await begin();
    })();
  });

  /** Everything the window does once it is allowed to ask questions. */
  async function begin(): Promise<void> {
    {
      const local = await api.localSession();
      const saved = (await api.uiState().catch(() => null)) as
        | {
            logHeight?: number;
            queueHeight?: number;
            splitRatio?: number;
            leftPath?: string;
            logPosition?: Position;
            queuePosition?: Position;
            logHidden?: boolean;
            queueHidden?: boolean;
            showTree?: { left?: boolean; right?: boolean };
            treeWidth?: { left?: number; right?: number };
            browseTogether?: boolean;
            showHidden?: { left?: boolean; right?: boolean };
            keys?: unknown;
          }
        | null;
      if (saved?.logHeight) logHeight = saved.logHeight;
      if (saved?.queueHeight) queueHeight = saved.queueHeight;
      if (saved?.splitRatio) splitRatio = saved.splitRatio;
      if (saved?.logPosition) logPosition = saved.logPosition;
      if (saved?.queuePosition) queuePosition = saved.queuePosition;
      logHidden = saved?.logHidden === true;
      queueHidden = saved?.queueHidden === true;
      if (saved?.treeWidth) {
        if (saved.treeWidth.left) setTreeWidth("left", saved.treeWidth.left);
        if (saved.treeWidth.right) setTreeWidth("right", saved.treeWidth.right);
      }
      setBrowsingTogether(saved?.browseTogether === true);
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
    }
  }

  /**
   * Which machine's files are on screen, for showing and for nothing else.
   *
   * The bridge holds the real answer; this follows it, because the bridge is
   * not a store and a program that can show two machines must never leave
   * somebody guessing which one they are looking at.
   */
  let service = $state<string | null>(null);
  let serviceOpen = $state(false);
  /**
   * Whether the service wants a password before it will say anything.
   *
   * The container build only. Everything behind it needs a session, so there
   * is nothing worth drawing until there is one.
   */
  let wantsPassword = $state(false);

  /**
   * The question two coupled panes raise, and the answer somebody gives it.
   *
   * Held open as a promise while the dialog is on screen, so the pane that is
   * following waits for the answer — it arrives where the question was asked
   * rather than as a second thing happening later.
   */
  let missing = $state<{ name: string; where: string; answer: (make: boolean) => void } | null>(
    null,
  );

  $effect(() => {
    whenDirectoryMissing(
      (side, path) =>
        new Promise<boolean>((answer) => {
          const at = path.lastIndexOf("/");
          missing = {
            name: at === -1 ? path : path.slice(at + 1),
            where: pane(side).path,
            answer: (make) => {
              missing = null;
              answer(make);
            },
          };
        }),
    );
  });

  /**
   * Starts again on whichever core is now in use.
   *
   * Everything is put down first. A pane still listing the last machine, a
   * queue still holding its jobs, a log still filling up from a socket nobody
   * closed — each of them would be one machine's answer sitting under
   * another's, and that is worse than an empty window.
   */
  async function startOver(): Promise<void> {
    unsubscribe?.();
    unsubscribe = null;
    clearLog();
    const local = await api.localSession();
    await openSession("left", local, null, null, null);
    await openSession("right", local, null, null, null);
    await refreshQueue();
    unsubscribe = await api.subscribe((event: CoreEvent) => {
      recordEvent(event);
      recordQueueEvent(event);
      recordEditEvent(event);
    });
  }

  /**
   * What became of a file another program is editing.
   *
   * Everything but a refusal goes to the log, where the rest of what happened
   * to a server already is. A refusal has to be asked about, and there is no
   * window of ours to ask in — the file is open somewhere else entirely.
   */
  function recordEditEvent(event: CoreEvent): void {
    if (event.event !== "edited") return;
    if (event.what === "changed") {
      editChanged = { id: event.id, name: event.name, path: event.path };
      return;
    }
    note(
      event.what === "pushed"
        ? t("editing.sent", { name: event.name })
        : t("editing.failed", { name: event.name, why: describe(event.error) }),
    );
  }

  let unsubscribe: Unsubscribe | null = null;
  $effect(() => {
    void api
      .subscribe((event: CoreEvent) => {
        recordEvent(event);
        recordQueueEvent(event);
        recordEditEvent(event);
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
      logHidden,
      queueHidden,
      showTree: { left: pane("left").showTree, right: pane("right").showTree },
      treeWidth: { left: pane("left").treeWidth, right: pane("right").treeWidth },
      browseTogether: browsingTogether(),
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
    siteId?: string,
    resume?: string,
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
        siteId ?? null,
        resume ?? null,
      );
      quickFor = null;
      pendingRequest = null;
    } catch (failure) {
      connectFailure = failure;
      // Everything the attempt was given, because the second attempt is the
      // same attempt: a host key accepted must not cost the pane the server it
      // belongs to, which is what decides whether deleting means deleting.
      pendingRequest = { request, historyId, side, startPath, siteId, resume };
    } finally {
      connecting = false;
    }
  }

  async function acceptHostKey(fingerprint: string): Promise<void> {
    if (!pendingRequest) return;
    const { request, historyId, side, startPath, siteId, resume } = pendingRequest;
    await attempt(
      { ...request, acceptFingerprint: fingerprint },
      historyId,
      side,
      startPath,
      siteId,
      resume,
    );
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
    if (site.auth !== "agent" && !site.hasPassword && !site.hasSessionPassword) {
      askingFor = { request, site, side };
      return;
    }
    await attempt(
      request,
      `${site.user}@${site.host}`,
      side,
      site.remotePath ?? undefined,
      site.id,
      // Only ever set when the entry asked to be remembered; the core decides
      // that, so there is nothing to weigh up here.
      site.lastPath ?? undefined,
    );
  }

  $effect(() => {
    let stop: Unsubscribe | undefined;
    void api.onOpenSite((id, side) => void openSite(id, side)).then((off) => (stop = off));
    return () => stop?.();
  });

  async function acceptCertificate(fingerprint: string): Promise<void> {
    if (!pendingRequest) return;
    const { request, historyId, side, startPath, siteId, resume } = pendingRequest;
    await attempt(
      { ...request, acceptCertificate: fingerprint },
      historyId,
      side,
      startPath,
      siteId,
      resume,
    );
  }

  /** Everything a transfer needs to know about where it is going. */
  function other(side: Side): Side {
    return side === "left" ? "right" : "left";
  }

  /** Puts entries from one pane into the queue, bound for the other. */
  /**
   * Sends what was chosen to the other side.
   *
   * `held` lines it up without setting it going, for somebody gathering a few
   * things first. Everything else is the same journey, so it is one function
   * and not two that would drift.
   */
  async function transfer(from: Side, names: string[], held = false): Promise<void> {
    if (names.length === 0) return;
    const source = pane(from);
    const target = pane(other(from));
    await api.enqueue({
      sourceEndpoint: source.endpoint,
      sourceDirectory: source.path,
      names,
      targetEndpoint: target.endpoint,
      targetDirectory: target.path,
      held,
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
    if (quickFor || hostKey || certificate || connectMenu) return;
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
        if (event.shiftKey) {
          selectTo(side, Math.min(view.cursor + 1, rows.length - 1));
        } else {
          moveCursor(side, 1, rows.length);
        }
        return;
      case "ArrowUp":
        event.preventDefault();
        if (event.shiftKey) {
          selectTo(side, Math.max(view.cursor - 1, 0));
        } else {
          moveCursor(side, -1, rows.length);
        }
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
        if (view.endpoint === LOCAL) openConnectMenu(side);
        else await disconnect(side);
        break;
      case "help":
        helpOpen = true;
        break;
      case "raw":
        rawOpen = !rawOpen;
        // Typing commands at a log that is hidden would be typing into
        // nothing, and the answers would arrive somewhere nobody is looking.
        if (rawOpen) logHidden = false;
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

{#if wantsPassword}
  <SignIn
    onin={() => {
      wantsPassword = false;
      void begin();
    }}
  />
{:else if editorFor}
  <Editor id={editorFor} onclose={() => void api.closeThisWindow()} />
{:else if isSiteManager}
  <SiteManager />
{:else}
<div class="window" use:tips>
  <!-- The one button this program exists for, where a program's main button
       belongs. It sat in the footer between appearance and help, among the
       things nobody opens twice a week. -->
  <div class="bar">
    <button type="button" class="servers" onclick={() => void api.openSiteManager()}>
      <Icon name="sites" size={14} />
      {t("sites.title")}
      <span class="shortcut mono">{label("Mod+S")}</span>
    </button>

    <!-- Whose files are on screen. Quiet on this machine, unmistakable when
         it is not: a program that can show two machines must never leave
         somebody guessing which one they are looking at.

         Not in the container build. A page served by a service is already on
         it, and "this machine" would be pointing at the very thing it is
         distinguishing itself from. -->
    {#if api.shell !== "web"}
    <button
      type="button"
      class="where"
      class:elsewhere={service !== null}
      onclick={() => (serviceOpen = true)}
      title={t("service.title")}
    >
      {service ?? t("service.here")}
    </button>
    {/if}

    <span class="gap"></span>

    <!-- Says where the check stands rather than only speaking up when there is
         news. Silence used to mean three different things — not asked yet,
         nothing new, could not reach GitHub — and they are not the same. -->
    <button
      type="button"
      class="update"
      class:news={updateStatus() === "available"}
      onclick={() => (newer ? (updateOpen = true) : void checkForUpdate(true))}
      title={newer ? t("update.hint", { version: newer.version }) : lastCheck()}
    >
      {#if newer}
        <Icon name="update" size={13} />
        {t("update.available", { version: newer.version })}
      {:else if updateStatus() === "checking"}
        {t("update.checking")}
      {:else if updateStatus() === "current"}
        {t("update.current")}
      {:else if updateStatus() === "unreachable"}
        {t("update.unreachable")}
      {:else}
        {t("update.check")}
      {/if}
    </button>

    <!-- The three that open a window and change nothing by themselves, kept
         together and away from the one that does the work. -->
    <button type="button" class="settings" onclick={() => (settingsOpen = !settingsOpen)}>
      {t("appearance.title")}
    </button>
    <button type="button" class="settings" onclick={() => (transferSettingsOpen = true)}>
      {t("settings.title")}
    </button>
    <button type="button" class="settings" onclick={() => (helpOpen = true)}>
      {t("help.title")}
    </button>
  </div>

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
        onconnect={(at) => (connectMenu = { side: "left", ...at })}
        onservers={() => void api.openSiteManager()}
        ondisconnect={() => disconnect("left")}
        ontransfer={(names, held) => transfer("left", names, held)}
        onedit={(path) => edit(path, "left")}
        onreceive={(from, names) => transfer(from, names)}
      />
    </div>
    <Splitter direction="vertical" label={t("splitter.panes")} onmove={resizeSplit} />
    <div class="half" style:flex="{1 - splitRatio}">
      <FilePane
        side="right"
        onconnect={(at) => (connectMenu = { side: "right", ...at })}
        onservers={() => void api.openSiteManager()}
        ondisconnect={() => disconnect("right")}
        ontransfer={(names, held) => transfer("right", names, held)}
        onedit={(path) => edit(path, "right")}
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
    <span class="keys mono">{t("status.keys", { servers: label("Mod+S") })}</span>
    <div class="actions">
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

</div>

{#if missing}
  {@const asked = missing}
  <div class="backdrop" role="presentation">
    <div class="ask" use:trap role="dialog" aria-modal="true">
      <h2>{t("together.title")}</h2>
      <p>{t("together.body", { name: asked.name, where: asked.where })}</p>
      <div class="buttons">
        <button type="button" onclick={() => asked.answer(false)}>{t("together.stay")}</button>
        <button type="button" class="primary" onclick={() => asked.answer(true)}>
          {t("together.create")}
        </button>
      </div>
    </div>
  </div>
{/if}

{#if serviceOpen}
  <ServiceDialog
    onuse={(address, token) => {
      serviceOpen = false;
      useService(address, token);
      service = connectedTo();
      void startOver();
    }}
    onhere={() => {
      serviceOpen = false;
      useThisMachine();
      service = connectedTo();
      void startOver();
    }}
    onclose={() => (serviceOpen = false)}
  />
{/if}

{#if settingsOpen}
  <AppearanceDialog
    logWhere={place(logPosition, logHidden)}
    queueWhere={place(queuePosition, queueHidden)}
    onlog={(where) => {
      logHidden = where === "off";
      if (where !== "off") logPosition = where;
    }}
    onqueue={(where) => {
      queueHidden = where === "off";
      if (where !== "off") queuePosition = where;
    }}
    onreset={resetLook}
    onclose={() => (settingsOpen = false)}
  />
{/if}

{#if updateOpen && newer}
  <UpdateDialog release={newer} onclose={() => (updateOpen = false)} />
{/if}

{#if connectMenu}
  {@const menu = connectMenu}
  <ConnectMenu
    x={menu.x}
    y={menu.y}
    onpick={(id) => closeMenu((side) => void openSite(id, side))}
    onquick={() => closeMenu((side) => ((quickFor = side), (connectFailure = null)))}
    onmanage={() => closeMenu(() => void api.openSiteManager())}
    onclose={() => (connectMenu = null)}
  />
{/if}

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
    ondecide={async (policy, scope) => {
      const id = asking[0]?.id;
      // Written down before it is acted on. A remembered answer that the
      // program forgets because something failed a moment later would be an
      // answer somebody gave and did not get.
      if (scope === "always") {
        const settings = await api.settings();
        await api.setSettings({ ...settings, conflictPolicy: policy });
      }
      if (id) await api.queueDecide(id, policy, scope !== "one");
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
              waiting.site.id,
              waiting.site.lastPath ?? undefined,
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

{#if leftBehind}
  <div class="backdrop" role="presentation">
    <div class="box" use:trap role="dialog" aria-modal="true">
      <h2>{t("editing.left.title")}</h2>
      <p>{t("editing.left", { names: leftBehind.names.join(", ") })}</p>
      <div class="choices">
        <button type="button" class="primary" onclick={() => leftBehind?.answer(true)}>
          {t("editing.left.delete")}
        </button>
        <button type="button" onclick={() => leftBehind?.answer(false)}>
          {t("editing.left.keep")}
        </button>
      </div>
    </div>
  </div>
{/if}

{#if editChanged}
  <!-- No editor window of ours to ask in: the file is open in another program
       and that is where the person is looking. Nothing was written, and what
       they saved is still in the copy either way. -->
  <div class="backdrop" role="presentation">
    <div class="box" use:trap role="dialog" aria-modal="true">
      <h2>{t("editing.changed.title", { name: editChanged.name })}</h2>
      <p>{t("editing.changed", { path: editChanged.path })}</p>
      <div class="choices">
        <button
          type="button"
          class="primary"
          onclick={() => {
            const waiting = editChanged;
            editChanged = null;
            if (waiting) void api.pushEdit(waiting.id, true).catch(() => undefined);
          }}
        >
          {t("editor.changed.overwrite")}
        </button>
        <button type="button" onclick={() => (editChanged = null)}>
          {t("editor.changed.leave")}
        </button>
      </div>
    </div>
  </div>
{/if}

{#if editingHere}
  <!-- Where a window of its own is not possible. It covers the page rather
       than floating over it: an editor with the file list showing round the
       edges invites typing into one while looking at the other. -->
  <div class="editing">
    <Editor id={editingHere} onclose={() => (editingHere = null)} />
  </div>
{/if}
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 45%);
    display: grid;
    place-items: center;
    /* Above everything, which no other dialog here needs to be. This is the
       only one that arrives without anybody having asked for it — a file was
       saved in another program — so it cannot rely on being the most recent
       thing opened. It was found sitting behind the keyboard dialog. */
    z-index: 72;
  }

  .box {
    width: min(420px, 92vw);
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-radius: 1rem;
    box-shadow: var(--shadow-lg);
    padding: 18px;
  }

  .box h2 {
    margin: 0 0 6px;
    font-size: 0.94rem;
  }

  .box p {
    margin: 0 0 14px;
    font-size: 0.8rem;
    color: var(--text-muted);
  }

  .choices {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }

  .choices button {
    border: 1px solid var(--border-strong);
    border-radius: 0.5rem;
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-size: 0.8rem;
    padding: 5px 12px;
    cursor: pointer;
  }

  .choices .primary {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }

  .editing {
    position: fixed;
    inset: 0;
    z-index: 60;
  }

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

  .bar {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 0 0 auto;
    padding: 5px 8px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-1);
  }

  /* Findable without being shouted at. The icon carries the accent, the
     button itself does not: an outlined pill at the top of the window sits in
     the corner of the eye all day, and that is what made it nag. */
  .bar .servers {
    display: flex;
    align-items: center;
    gap: 6px;
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    padding: 4px 10px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.8rem;
    font-weight: 500;
    cursor: pointer;
  }

  .bar .servers :global(svg) {
    color: var(--accent);
  }

  .bar .servers:hover {
    border-color: var(--border-strong);
    background: var(--surface-3);
  }

  .bar .gap {
    flex: 1;
  }

  .bar .where {
    border: 1px solid transparent;
    border-radius: 0.5rem;
    padding: 4px 10px;
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-size: 0.76rem;
    cursor: pointer;
  }

  .bar .where:hover {
    border-color: var(--border);
    color: var(--text);
  }

  /* Another machine is not a detail. */
  .bar .where.elsewhere {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }

  .bar .update {
    display: flex;
    align-items: center;
    gap: 5px;
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    padding: 4px 10px;
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-size: 0.76rem;
    cursor: pointer;
  }

  .bar .update:hover {
    border-color: var(--border-strong);
    color: var(--text);
  }

  /* The one state worth colour. Everything else the button reports is the
     absence of news, and the absence of news should not glow. */
  .bar .update.news {
    border-color: var(--accent);
    color: var(--accent);
    font-weight: 600;
  }

  .bar .shortcut {
    font-size: 0.7rem;
    color: var(--text-muted);
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

  footer {
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

</style>
