/**
 * Bridge implementation for the container build (milestone M7): the same core,
 * reached over HTTP and WebSocket instead of over Tauri's channel.
 *
 * The service does not exist yet. What exists is this file, built on every
 * push, so the frontend cannot quietly grow a dependency on Tauri — and so the
 * day the service is written, the window needs no changes at all.
 *
 * The shapes below are the contract that service meets: one POST per command
 * under `/api/`, and one WebSocket at `/api/events` carrying the very same
 * event objects the desktop shell emits.
 *
 * The names are the ones in `amberbeam-commands`, spelled exactly as they are
 * there. A second spelling would mean a translation on the way in, and a
 * translation is somewhere to be wrong.
 */

import type {
  AmberBeamApi,
  Connected,
  ConnectRequest,
  CoreEvent,
  CoreInfo,
  Comparison,
  Edit,
  EditRule,
  How,
  Listing,
  ConflictPolicy,
  EnqueueRequest,
  Measurement,
  Queue,
  QuickConnectEntry,
  BundlePreview,
  ImportCandidate,
  ImportPreview,
  ImportSource,
  OpenSide,
  RawReply,
  SearchResult,
  Release,
  Removed,
  SecretKind,
  Site,
  Totals,
  Settings,
  Unsubscribe,
  Watch,
} from "./types";

/**
 * What a service says when it does not know who is asking.
 *
 * One object, compared by identity, so recognising it cannot go wrong on a
 * misspelt string.
 */
export const NOT_SIGNED_IN = { kind: "not-signed-in" } as const;

/** Whether a rejected call was refused for want of a session. */
export function isNotSignedIn(problem: unknown): boolean {
  return problem === NOT_SIGNED_IN;
}

/**
 * One service, as an object the window can call.
 *
 * A function rather than a fixed object, because the desktop program can point
 * at a service on another machine while its own core is still there. Two
 * targets at once is not a thing anybody needs, but module-level state that
 * two shells read differently is a thing that bites, and a closure costs
 * nothing.
 */
export function makeApi(target: Target): AmberBeamApi {
  const base = target.base;

  async function call<T>(command: string, body?: unknown): Promise<T> {
    const response = await fetch(`${base}/api/${command}`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        accept: "application/json",
        // A browser carries the cookie the service set; a desktop window
        // talking to another machine carries this instead. Both are the same
        // session, arrived at by the same login.
        ...(target.token ? { authorization: `Bearer ${target.token}` } : {}),
      },
      credentials: "same-origin",
      body: JSON.stringify(body ?? {}),
    });
    if (response.status === 401) {
      // Told apart from every other failure on purpose. "Not signed in" is
      // not something to show as an error; it is the window's cue to ask for
      // the password, and a message buried among the others cannot be that.
      throw NOT_SIGNED_IN;
    }
    if (!response.ok) {
      // The service answers a failure with the same tagged object the desktop
      // shell rejects with, so both halves of the window handle one shape.
      let detail: unknown = { kind: "other", detail: `${command} answered ${response.status}` };
      try {
        detail = await response.json();
      } catch {
        // A response that is not even JSON stays with the fallback above.
      }
      throw detail;
    }
    return (await response.json()) as T;
  }

  const built: AmberBeamApi = {
    shell: "web",

    coreInfo: () => call<CoreInfo>("core_info"),

    async subscribe(handler: (event: CoreEvent) => void): Promise<Unsubscribe> {
      const address = new URL(`${base}/api/events`, window.location.href);
      address.protocol = address.protocol === "https:" ? "wss:" : "ws:";
      // A WebSocket cannot be opened with a header, so a client that has no
      // cookie to lean on puts the token here. It is the ordinary way round
      // this and worth knowing about: an address can end up in a proxy log in
      // a way a header does not, which is one more reason this belongs behind
      // TLS. A browser on the service's own page sends its cookie and adds
      // nothing.
      if (target.token) address.searchParams.set("token", target.token);

      let socket = new WebSocket(address);
      let closed = false;
      let retry: ReturnType<typeof setTimeout> | undefined;

      const attach = () => {
        socket.addEventListener("message", (message) => {
          handler(JSON.parse(message.data as string) as CoreEvent);
        });
        socket.addEventListener("close", () => {
          // A dropped connection must not silence the server log for good; a
          // desktop channel never drops, so only this half needs to reconnect.
          if (closed) return;
          retry = setTimeout(() => {
            socket = new WebSocket(address);
            attach();
          }, 1000);
        });
      };
      attach();

      return () => {
        closed = true;
        if (retry !== undefined) clearTimeout(retry);
        socket.close();
      };
    },

    localSession: () => call<Connected>("local_session"),
    connect: (request: ConnectRequest) => call<Connected>("connect", { request }),
    disconnect: (endpoint: string) => call<void>("disconnect", { endpoint }),

    listDir: (endpoint: string, path: string) => call<Listing>("list_dir", { endpoint, path }),
    parentOf: (endpoint: string, path: string) =>
      call<string | null>("parent_of", { endpoint, path }),
    joinPath: (endpoint: string, directory: string, name: string) =>
      call<string>("join_path", { endpoint, directory, name }),

    createDir: (endpoint: string, directory: string, name: string) =>
      call<void>("create_dir", { endpoint, directory, name }),
    createFile: (endpoint: string, directory: string, name: string) =>
      call<void>("create_file", { endpoint, directory, name }),
    renameEntry: (endpoint: string, directory: string, from: string, to: string) =>
      call<void>("rename_entry", { endpoint, directory, from, to }),
    measure: (endpoint: string, path: string) => call<Measurement>("measure", { endpoint, path }),
    removeEntry: (endpoint: string, path: string, siteId?: string | null) =>
      call<Removed>("remove_entry", { endpoint, path, siteId: siteId ?? null }),
    setPermissions: (endpoint: string, path: string, mode: number, recursive: boolean) =>
      call<void>("set_permissions", { endpoint, path, mode, recursive }),

    compare: (it: {
      hereEndpoint: string;
      herePath: string;
      thereEndpoint: string;
      therePath: string;
      recursive: boolean;
      how: How;
      excludes: string[];
    }) => call<Comparison>("compare", it),
    startWatch: (it: {
      root: string;
      targetEndpoint: string;
      targetRoot: string;
      targetTitle: string | null;
      siteId: string | null;
      excludes: string[];
    }) => call<Watch>("start_watch", it),
    stopWatch: (id: string) => call<Watch | null>("stop_watch", { id }),
    watches: () => call<Watch[]>("watches"),
    startEdit: (endpoint: string, path: string) =>
      call<Edit>("start_edit", { endpoint, path }),
    howToEdit: (name: string) => call<EditRule | null>("how_to_edit", { name }),
    openEdits: () => call<Edit[]>("open_edits"),
    editText: (id: string) => call<string>("edit_text", { id }),
    saveEdit: (id: string, text: string) => call<Edit>("save_edit", { id, text }),
    pushEdit: (id: string, anyway: boolean) => call<Edit>("push_edit", { id, anyway }),
    endEdit: (id: string, deleteCopy: boolean) =>
      call<Edit | null>("end_edit", { id, deleteCopy }),
    endEdits: (deleteCopies: boolean) => call<Edit[]>("end_edits", { deleteCopies }),

    quickConnectHistory: () => call<QuickConnectEntry[]>("quick_connect_history"),
    forgetQuickConnect: (id: string) => call<void>("forget_quick_connect", { id }),
    saveAsSite: (id: string) => call<string>("save_as_site", { id }),
    rememberPath: (id: string, path: string, siteId?: string | null) =>
      call<void>("remember_path", { id, path, siteId: siteId ?? null }),

    async onFileDrop(): Promise<Unsubscribe> {
      // A browser never learns the path of a dropped file, only its contents, so
      // the container build cannot answer this the way the desktop does. Files
      // dragged into that window will have to be uploaded through the browser —
      // a different workflow, and part of M7 rather than something to fake here.
      return () => undefined;
    },

    enqueue: (request: EnqueueRequest) => call<number>("enqueue", { request }),
    queueSnapshot: () => call<Queue>("queue_snapshot"),
    queueTotals: () => call<Totals>("queue_totals"),
    queuePause: (paused: boolean) => call<void>("queue_pause", { paused }),
    queueHold: (id: string) => call<void>("queue_hold", { id }),
    queueResume: (id: string) => call<void>("queue_resume", { id }),
    queueRemove: (id: string) => call<void>("queue_remove", { id }),
    queueClearFinished: () => call<void>("queue_clear_finished"),
    queueClearAll: () => call<void>("queue_clear_all"),
    queueMove: (id: string, by?: number, to?: number) => call<void>("queue_move", { id, by, to }),
    queueDecide: (id: string, policy: ConflictPolicy, forAll: boolean) =>
      call<void>("queue_decide", { id, policy, forAll }),

    updateSource: () => call<string>("update_source"),
    newerRelease: (answer: string) => call<Release | null>("newer_release", { answer }),

    async openUrl(url: string): Promise<void> {
      // In a browser the window can simply do it, and should: asking the server
      // to open a link would open it on the server.
      window.open(url, "_blank", "noreferrer");
    },

    settings: () => call<Settings>("settings"),
    setSettings: (value: Settings) => call<void>("set_settings", { value }),

    sites: () => call<Site[]>("sites"),
    siteFolders: () => call<string[]>("site_folders"),
    saveSite: (folder: string, site: Site) => call<string>("save_site", { folder, site }),
    deleteSite: (id: string) => call<void>("delete_site", { id }),
    createSiteFolder: (folder: string) => call<void>("create_site_folder", { folder }),
    renameSiteFolder: (from: string, to: string) => call<void>("rename_site_folder", { from, to }),
    deleteSiteFolder: (folder: string) => call<void>("delete_site_folder", { folder }),
    // A page in a browser cannot replace the program serving it, and should not
    // pretend otherwise. The window offers the download page instead.
    downloadUrl(endpoint: string, path: string): string {
      const address = new URL(`${base}/api/download`, window.location.href);
      address.searchParams.set("endpoint", endpoint);
      address.searchParams.set("path", path);
      if (target.token) address.searchParams.set("token", target.token);
      return address.toString();
    },

    async uploadInto(endpoint: string, directory: string, file: File): Promise<void> {
      const address = new URL(`${base}/api/upload`, window.location.href);
      address.searchParams.set("endpoint", endpoint);
      address.searchParams.set("directory", directory);
      address.searchParams.set("name", file.name);
      // The file itself as the body, not wrapped in a form. A hundred
      // gigabytes should travel as a hundred gigabytes and not as a part of
      // something larger that has to be taken apart at the other end.
      const answer = await fetch(address, {
        method: "POST",
        credentials: "same-origin",
        headers: target.token ? { authorization: `Bearer ${target.token}` } : {},
        body: file,
      });
      if (!answer.ok) throw await answer.json();
    },

    canInstallUpdate: () => Promise.resolve(false),
    installUpdate: () => Promise.reject(new Error("the web shell cannot install updates")),
    restart: () => Promise.reject(new Error("the web shell cannot restart the program")),

    setSiteSecret: (id: string, kind: SecretKind, value: string) =>
      call<void>("set_site_secret", { id, kind, value }),
    forgetSiteSecret: (id: string, kind: SecretKind) =>
      call<void>("forget_site_secret", { id, kind }),
    setSessionSecret: (id: string, kind: SecretKind, value: string) =>
      call<void>("set_session_secret", { id, kind, value }),
    forgetSessionSecret: (id: string, kind: SecretKind) =>
      call<void>("forget_session_secret", { id, kind }),

    async chooseFile(): Promise<string | null> {
      // A browser cannot hand over a path, and the container build has no file
      // system of the viewer's to point at. Its way in is the drop target, and
      // uploading arrives with the service itself in M7.
      return null;
    },
    async chooseSaveFile(): Promise<string | null> {
      // Likewise: a browser downloads rather than writes, which is a different
      // shape of the same job and belongs with the service.
      return null;
    },

    exportSites: (path: string, withPasswords: boolean, passphrase: string | null) =>
      call<number>("export_sites", { path, withPasswords, passphrase }),
    bundlePreview: (path: string, passphrase: string | null) =>
      call<BundlePreview>("bundle_preview", { path, passphrase }),
    bundleApply: (path: string, passphrase: string | null, chosen: number[], into: string) =>
      call<number>("bundle_apply", { path, passphrase, chosen, into }),

    importCandidates: () => call<ImportCandidate[]>("import_candidates"),
    importPreview: (source: ImportSource, path: string) =>
      call<ImportPreview>("import_preview", { source, path }),
    importApply: (
      source: ImportSource,
      path: string,
      chosen: number[],
      expected: number,
      takePasswords: boolean,
      into: string,
    ) => call<number>("import_apply", { source, path, chosen, expected, takePasswords, into }),

    search: (endpoint: string, root: string, needle: string, limit: number) =>
      call<SearchResult>("search", { endpoint, root, needle, limit }),
    rawCommand: (endpoint: string, command: string) =>
      call<RawReply>("raw_command", { endpoint, command }),

    async openSystemKeyboard(): Promise<void> {
      // A browser has no system settings to open, and the container build runs
      // on somebody else's machine anyway.
    },
    async toggleFullscreen(): Promise<boolean> {
      // The browser's own, which needs a gesture and grants nothing on its own.
      const element = document.documentElement;
      if (document.fullscreenElement) {
        await document.exitFullscreen();
        return false;
      }
      await element.requestFullscreen();
      return true;
    },
    writeTextFile: (path: string, text: string) => call<void>("write_text_file", { path, text }),
    readTextFile: (path: string) => call<string>("read_text_file", { path }),

    windowLabel: () =>
      // One page in a browser, so the view is where it has always been for this
      // shell: in the address. A browser has no windows to label.
      new URLSearchParams(location.search).get("view") === "sites" ? "sites" : "main",

    async openSiteManager(): Promise<void> {
      // A browser tab cannot open a native window, and the container build shows
      // the site manager as a view of the same page instead.
      window.location.search = "?view=sites";
    },
    async openWith(): Promise<boolean> {
      // The service runs on another machine. Starting a program over there
      // would not put it in front of the person who asked.
      return false;
    },
    async mcpCommand(): Promise<string | null> {
      // There is a program to point a client at, but it is not this one and it
      // is not on this machine. The container's own instructions say how to
      // reach it; a path made up here would be a path to nothing.
      return null;
    },
    async onClosing(): Promise<Unsubscribe> {
      // Closing a tab is not the program ending: the service carries on, and
      // the copies it holds are swept when it next starts. A browser would not
      // let this page ask a question of its own on the way out anyway.
      return () => undefined;
    },
    async showEditsFolder(): Promise<boolean> {
      // The copies are on the machine running the service, and a file manager
      // opened there would be nowhere anybody can see.
      return false;
    },
    async closeThisWindow(): Promise<void> {
      // A browser tab that closed itself would take the whole session with it.
    },
    async openEditor(): Promise<boolean> {
      // A browser tab cannot open a second window belonging to this session,
      // and a page that navigated away to show one would have to come back --
      // which means signing in and starting over. The editor is shown over
      // this page instead.
      return false;
    },
    openSite: (id: string, side: OpenSide) => call<void>("open_site", { id, side }),
    async onOpenSite(): Promise<Unsubscribe> {
      // One page, one view: nothing here has to ask another window to connect.
      return () => undefined;
    },

    uiState: () => call<unknown>("ui_state"),
    setUiState: (value: unknown) => call<void>("set_ui_state", { value }),
  };

  return built;
}

/**
 * Where a service is, and what proves the right to talk to it.
 *
 * An empty address means the service that served this page, which is what the
 * container build always means.
 */
export interface Target {
  base: string;
  token?: string;
}

/** The web shell's own: whatever served this page. */
export const api: AmberBeamApi = makeApi({
  base: import.meta.env.VITE_AMBERBEAM_SERVER ?? "",
});
