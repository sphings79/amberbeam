/**
 * Bridge implementation for the container build (milestone M7): the same core,
 * reached over HTTP and WebSocket instead of over Tauri's channel.
 *
 * The service does not exist yet. What exists is this file, built on every
 * push, so the frontend cannot quietly grow a dependency on Tauri — and so the
 * day the service is written, the window needs no changes at all.
 *
 * The shapes below are the contract that service will have to meet: one POST
 * per command under `/api/`, and one WebSocket at `/api/events` carrying the
 * very same event objects the desktop shell emits.
 */

import type {
  AmberBeamApi,
  Connected,
  ConnectRequest,
  CoreEvent,
  CoreInfo,
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
  SecretKind,
  Site,
  Totals,
  Settings,
  Unsubscribe,
} from "./types";

const base = import.meta.env.VITE_AMBERBEAM_SERVER ?? "";

async function call<T>(command: string, body?: unknown): Promise<T> {
  const response = await fetch(`${base}/api/${command}`, {
    method: "POST",
    headers: { "content-type": "application/json", accept: "application/json" },
    credentials: "same-origin",
    body: JSON.stringify(body ?? {}),
  });
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

export const api: AmberBeamApi = {
  shell: "web",

  coreInfo: () => call<CoreInfo>("core-info"),

  async subscribe(handler: (event: CoreEvent) => void): Promise<Unsubscribe> {
    const address = new URL(`${base}/api/events`, window.location.href);
    address.protocol = address.protocol === "https:" ? "wss:" : "ws:";

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

  localSession: () => call<Connected>("local-session"),
  connect: (request: ConnectRequest) => call<Connected>("connect", request),
  disconnect: (endpoint: string) => call<void>("disconnect", { endpoint }),

  listDir: (endpoint: string, path: string) => call<Listing>("list-dir", { endpoint, path }),
  parentOf: (endpoint: string, path: string) =>
    call<string | null>("parent-of", { endpoint, path }),
  joinPath: (endpoint: string, directory: string, name: string) =>
    call<string>("join-path", { endpoint, directory, name }),

  createDir: (endpoint: string, directory: string, name: string) =>
    call<void>("create-dir", { endpoint, directory, name }),
  createFile: (endpoint: string, directory: string, name: string) =>
    call<void>("create-file", { endpoint, directory, name }),
  renameEntry: (endpoint: string, directory: string, from: string, to: string) =>
    call<void>("rename-entry", { endpoint, directory, from, to }),
  measure: (endpoint: string, path: string) => call<Measurement>("measure", { endpoint, path }),
  removeEntry: (endpoint: string, path: string) => call<void>("remove-entry", { endpoint, path }),
  setPermissions: (endpoint: string, path: string, mode: number, recursive: boolean) =>
    call<void>("set-permissions", { endpoint, path, mode, recursive }),

  quickConnectHistory: () => call<QuickConnectEntry[]>("quick-connect-history"),
  forgetQuickConnect: (id: string) => call<void>("forget-quick-connect", { id }),
  saveAsSite: (id: string) => call<string>("save-as-site", { id }),
  rememberPath: (id: string, path: string) => call<void>("remember-path", { id, path }),

  async onFileDrop(): Promise<Unsubscribe> {
    // A browser never learns the path of a dropped file, only its contents, so
    // the container build cannot answer this the way the desktop does. Files
    // dragged into that window will have to be uploaded through the browser —
    // a different workflow, and part of M7 rather than something to fake here.
    return () => undefined;
  },

  enqueue: (request: EnqueueRequest) => call<number>("enqueue", request),
  queueSnapshot: () => call<Queue>("queue-snapshot"),
  queueTotals: () => call<Totals>("queue-totals"),
  queuePause: (paused: boolean) => call<void>("queue-pause", { paused }),
  queueHold: (id: string) => call<void>("queue-hold", { id }),
  queueResume: (id: string) => call<void>("queue-resume", { id }),
  queueRemove: (id: string) => call<void>("queue-remove", { id }),
  queueClearFinished: () => call<void>("queue-clear-finished"),
  queueClearAll: () => call<void>("queue-clear-all"),
  queueMove: (id: string, by?: number, to?: number) => call<void>("queue-move", { id, by, to }),
  queueDecide: (id: string, policy: ConflictPolicy, forAll: boolean) =>
    call<void>("queue-decide", { id, policy, forAll }),

  updateSource: () => call<string>("update-source"),
  newerRelease: (answer: string) => call<Release | null>("newer-release", { answer }),

  async openUrl(url: string): Promise<void> {
    // In a browser the window can simply do it, and should: asking the server
    // to open a link would open it on the server.
    window.open(url, "_blank", "noreferrer");
  },

  settings: () => call<Settings>("settings"),
  setSettings: (value: Settings) => call<void>("set-settings", { value }),

  sites: () => call<Site[]>("sites"),
  siteFolders: () => call<string[]>("site-folders"),
  saveSite: (folder: string, site: Site) => call<string>("save-site", { folder, site }),
  deleteSite: (id: string) => call<void>("delete-site", { id }),
  createSiteFolder: (folder: string) => call<void>("create-site-folder", { folder }),
  renameSiteFolder: (from: string, to: string) => call<void>("rename-site-folder", { from, to }),
  deleteSiteFolder: (folder: string) => call<void>("delete-site-folder", { folder }),
  setSiteSecret: (id: string, kind: SecretKind, value: string) =>
    call<void>("set-site-secret", { id, kind, value }),
  forgetSiteSecret: (id: string, kind: SecretKind) =>
    call<void>("forget-site-secret", { id, kind }),

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
    call<number>("export-sites", { path, withPasswords, passphrase }),
  bundlePreview: (path: string, passphrase: string | null) =>
    call<BundlePreview>("bundle-preview", { path, passphrase }),
  bundleApply: (path: string, passphrase: string | null, chosen: number[], into: string) =>
    call<number>("bundle-apply", { path, passphrase, chosen, into }),

  importCandidates: () => call<ImportCandidate[]>("import-candidates"),
  importPreview: (source: ImportSource, path: string) =>
    call<ImportPreview>("import-preview", { source, path }),
  importApply: (
    source: ImportSource,
    path: string,
    chosen: number[],
    expected: number,
    takePasswords: boolean,
    into: string,
  ) => call<number>("import-apply", { source, path, chosen, expected, takePasswords, into }),

  search: (endpoint: string, root: string, needle: string, limit: number) =>
    call<SearchResult>("search", { endpoint, root, needle, limit }),
  rawCommand: (endpoint: string, command: string) =>
    call<RawReply>("raw-command", { endpoint, command }),

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
  writeTextFile: (path: string, text: string) => call<void>("write-text-file", { path, text }),
  readTextFile: (path: string) => call<string>("read-text-file", { path }),

  async openSiteManager(): Promise<void> {
    // A browser tab cannot open a native window, and the container build shows
    // the site manager as a view of the same page instead.
    window.location.search = "?view=sites";
  },
  openSite: (id: string, side: OpenSide) => call<void>("open-site", { id, side }),
  async onOpenSite(): Promise<Unsubscribe> {
    // One page, one view: nothing here has to ask another window to connect.
    return () => undefined;
  },

  uiState: () => call<unknown>("ui-state"),
  setUiState: (value: unknown) => call<void>("set-ui-state", { value }),
};
