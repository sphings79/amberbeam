/** Bridge implementation for the desktop build: Tauri's internal channel. */

import { invoke } from "@tauri-apps/api/core";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";

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

/** Matches EVENT_CHANNEL in the desktop shell. */
const EVENT_CHANNEL = "amberbeam://event";

export const api: AmberBeamApi = {
  shell: "desktop",

  coreInfo: () => invoke<CoreInfo>("core_info"),

  async subscribe(handler: (event: CoreEvent) => void): Promise<Unsubscribe> {
    return await listen<CoreEvent>(EVENT_CHANNEL, (message) => handler(message.payload));
  },

  localSession: () => invoke<Connected>("local_session"),
  connect: (request: ConnectRequest) => invoke<Connected>("connect", { request }),
  disconnect: (endpoint: string) => invoke<void>("disconnect", { endpoint }),

  listDir: (endpoint: string, path: string) => invoke<Listing>("list_dir", { endpoint, path }),
  parentOf: (endpoint: string, path: string) =>
    invoke<string | null>("parent_of", { endpoint, path }),
  joinPath: (endpoint: string, directory: string, name: string) =>
    invoke<string>("join_path", { endpoint, directory, name }),

  createDir: (endpoint: string, directory: string, name: string) =>
    invoke<void>("create_dir", { endpoint, directory, name }),
  createFile: (endpoint: string, directory: string, name: string) =>
    invoke<void>("create_file", { endpoint, directory, name }),
  renameEntry: (endpoint: string, directory: string, from: string, to: string) =>
    invoke<void>("rename_entry", { endpoint, directory, from, to }),
  measure: (endpoint: string, path: string) => invoke<Measurement>("measure", { endpoint, path }),
  removeEntry: (endpoint: string, path: string) =>
    invoke<void>("remove_entry", { endpoint, path }),
  setPermissions: (endpoint: string, path: string, mode: number, recursive: boolean) =>
    invoke<void>("set_permissions", { endpoint, path, mode, recursive }),

  quickConnectHistory: () => invoke<QuickConnectEntry[]>("quick_connect_history"),
  forgetQuickConnect: (id: string) => invoke<void>("forget_quick_connect", { id }),
  saveAsSite: (id: string) => invoke<string>("save_as_site", { id }),
  rememberPath: (id: string, path: string) => invoke<void>("remember_path", { id, path }),

  async onFileDrop(handler): Promise<Unsubscribe> {
    return await getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type !== "drop") return;
      handler(event.payload.paths, event.payload.position);
    });
  },

  enqueue: (request: EnqueueRequest) => invoke<number>("enqueue", { request }),
  queueSnapshot: () => invoke<Queue>("queue_snapshot"),
  queueTotals: () => invoke<Totals>("queue_totals"),
  queuePause: (paused: boolean) => invoke<void>("queue_pause", { paused }),
  queueHold: (id: string) => invoke<void>("queue_hold", { id }),
  queueResume: (id: string) => invoke<void>("queue_resume", { id }),
  queueRemove: (id: string) => invoke<void>("queue_remove", { id }),
  queueClearFinished: () => invoke<void>("queue_clear_finished"),
  queueClearAll: () => invoke<void>("queue_clear_all"),
  queueMove: (id: string, by?: number, to?: number) => invoke<void>("queue_move", { id, by, to }),
  queueDecide: (id: string, policy: ConflictPolicy, forAll: boolean) =>
    invoke<void>("queue_decide", { id, policy, forAll }),

  openUrl: (url: string) => invoke<void>("open_url", { url }),
  updateSource: () => invoke<string>("update_source"),
  newerRelease: (answer: string) => invoke<Release | null>("newer_release", { answer }),

  settings: () => invoke<Settings>("settings"),
  setSettings: (value: Settings) => invoke<void>("set_settings", { value }),

  sites: () => invoke<Site[]>("sites"),
  siteFolders: () => invoke<string[]>("site_folders"),
  saveSite: (folder: string, site: Site) => invoke<string>("save_site", { folder, site }),
  deleteSite: (id: string) => invoke<void>("delete_site", { id }),
  createSiteFolder: (folder: string) => invoke<void>("create_site_folder", { folder }),
  renameSiteFolder: (from: string, to: string) => invoke<void>("rename_site_folder", { from, to }),
  deleteSiteFolder: (folder: string) => invoke<void>("delete_site_folder", { folder }),
  setSiteSecret: (id: string, kind: SecretKind, value: string) =>
    invoke<void>("set_site_secret", { id, kind, value }),
  forgetSiteSecret: (id: string, kind: SecretKind) =>
    invoke<void>("forget_site_secret", { id, kind }),
  setSessionSecret: (id: string, kind: SecretKind, value: string) =>
    invoke<void>("set_session_secret", { id, kind, value }),
  forgetSessionSecret: (id: string, kind: SecretKind) =>
    invoke<void>("forget_session_secret", { id, kind }),

  async chooseFile(title: string): Promise<string | null> {
    const chosen = await openDialog({ title, multiple: false, directory: false });
    return typeof chosen === "string" ? chosen : null;
  },
  async chooseSaveFile(title: string, suggested: string): Promise<string | null> {
    return await saveDialog({ title, defaultPath: suggested });
  },

  exportSites: (path: string, withPasswords: boolean, passphrase: string | null) =>
    invoke<number>("export_sites", { path, withPasswords, passphrase }),
  bundlePreview: (path: string, passphrase: string | null) =>
    invoke<BundlePreview>("bundle_preview", { path, passphrase }),
  bundleApply: (path: string, passphrase: string | null, chosen: number[], into: string) =>
    invoke<number>("bundle_apply", { path, passphrase, chosen, into }),

  importCandidates: () => invoke<ImportCandidate[]>("import_candidates"),
  importPreview: (source: ImportSource, path: string) =>
    invoke<ImportPreview>("import_preview", { source, path }),
  importApply: (
    source: ImportSource,
    path: string,
    chosen: number[],
    expected: number,
    takePasswords: boolean,
    into: string,
  ) => invoke<number>("import_apply", { source, path, chosen, expected, takePasswords, into }),

  search: (endpoint: string, root: string, needle: string, limit: number) =>
    invoke<SearchResult>("search", { endpoint, root, needle, limit }),
  rawCommand: (endpoint: string, command: string) =>
    invoke<RawReply>("raw_command", { endpoint, command }),

  openSystemKeyboard: (pane: "function-keys" | "shortcuts") =>
    invoke<void>("open_system_keyboard", { pane }),
  async toggleFullscreen(): Promise<boolean> {
    const window = getCurrentWindow();
    const now = !(await window.isFullscreen());
    await window.setFullscreen(now);
    return now;
  },
  writeTextFile: (path: string, text: string) => invoke<void>("write_text_file", { path, text }),
  readTextFile: (path: string) => invoke<string>("read_text_file", { path }),

  windowLabel(): string {
    // What the window itself declared, before any of this ran. Asking Tauri is
    // the fallback rather than the first move: that call needs the internals to
    // be injected already, and a window that has not got there yet answers by
    // throwing — which, at module scope, is a blank window and no explanation.
    const declared = (globalThis as { __AMBERBEAM_VIEW__?: string }).__AMBERBEAM_VIEW__;
    if (declared) return declared;
    try {
      return getCurrentWindow().label;
    } catch {
      return "main";
    }
  },
  openSiteManager: () => invoke<void>("open_site_manager"),
  openSite: (id: string, side: OpenSide) => invoke<void>("open_site", { id, side }),
  async onOpenSite(handler): Promise<Unsubscribe> {
    return await listen<{ id: string; side: OpenSide }>("amberbeam://open-site", (event) =>
      handler(event.payload.id, event.payload.side),
    );
  },

  uiState: () => invoke<unknown>("ui_state"),
  setUiState: (value: unknown) => invoke<void>("set_ui_state", { value }),
};
