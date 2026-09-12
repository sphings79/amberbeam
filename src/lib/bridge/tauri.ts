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
} from "./types";

/** Matches EVENT_CHANNEL in the desktop shell. */
const EVENT_CHANNEL = "amberbeam://event";

/**
 * Everything the core does, through the one command that carries the rest.
 *
 * The shell used to declare a Tauri command per operation, and the container
 * service would have had to declare the same list again. They are one list now,
 * in `amberbeam-commands`, and both shells hand it a name and the arguments as
 * sent. `invoke` stays for the handful that are about windows, or about a path
 * a person chose in a dialog — those exist here and nowhere else.
 */
function send<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  return invoke<T>("run_command", { command, args });
}

export const api: AmberBeamApi = {
  shell: "desktop",

  coreInfo: () => send<CoreInfo>("core_info"),

  async subscribe(handler: (event: CoreEvent) => void): Promise<Unsubscribe> {
    return await listen<CoreEvent>(EVENT_CHANNEL, (message) => handler(message.payload));
  },

  localSession: () => send<Connected>("local_session"),
  connect: (request: ConnectRequest) => send<Connected>("connect", { request }),
  disconnect: (endpoint: string) => send<void>("disconnect", { endpoint }),

  listDir: (endpoint: string, path: string) => send<Listing>("list_dir", { endpoint, path }),
  parentOf: (endpoint: string, path: string) =>
    send<string | null>("parent_of", { endpoint, path }),
  joinPath: (endpoint: string, directory: string, name: string) =>
    send<string>("join_path", { endpoint, directory, name }),

  createDir: (endpoint: string, directory: string, name: string) =>
    send<void>("create_dir", { endpoint, directory, name }),
  createFile: (endpoint: string, directory: string, name: string) =>
    send<void>("create_file", { endpoint, directory, name }),
  renameEntry: (endpoint: string, directory: string, from: string, to: string) =>
    send<void>("rename_entry", { endpoint, directory, from, to }),
  measure: (endpoint: string, path: string) => send<Measurement>("measure", { endpoint, path }),
  removeEntry: (endpoint: string, path: string, siteId?: string | null) =>
    send<Removed>("remove_entry", { endpoint, path, siteId: siteId ?? null }),
  setPermissions: (endpoint: string, path: string, mode: number, recursive: boolean) =>
    send<void>("set_permissions", { endpoint, path, mode, recursive }),

  quickConnectHistory: () => send<QuickConnectEntry[]>("quick_connect_history"),
  forgetQuickConnect: (id: string) => send<void>("forget_quick_connect", { id }),
  saveAsSite: (id: string) => send<string>("save_as_site", { id }),
  rememberPath: (id: string, path: string, siteId?: string | null) =>
    send<void>("remember_path", { id, path, siteId: siteId ?? null }),

  async onFileDrop(handler): Promise<Unsubscribe> {
    return await getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type !== "drop") return;
      handler(event.payload.paths, event.payload.position);
    });
  },

  enqueue: (request: EnqueueRequest) => send<number>("enqueue", { request }),
  queueSnapshot: () => send<Queue>("queue_snapshot"),
  queueTotals: () => send<Totals>("queue_totals"),
  queuePause: (paused: boolean) => send<void>("queue_pause", { paused }),
  queueHold: (id: string) => send<void>("queue_hold", { id }),
  queueResume: (id: string) => send<void>("queue_resume", { id }),
  queueRemove: (id: string) => send<void>("queue_remove", { id }),
  queueClearFinished: () => send<void>("queue_clear_finished"),
  queueClearAll: () => send<void>("queue_clear_all"),
  queueMove: (id: string, by?: number, to?: number) => send<void>("queue_move", { id, by, to }),
  queueDecide: (id: string, policy: ConflictPolicy, forAll: boolean) =>
    send<void>("queue_decide", { id, policy, forAll }),


  compare: (it: {
    hereEndpoint: string;
    herePath: string;
    thereEndpoint: string;
    therePath: string;
    recursive: boolean;
    how: How;
    excludes: string[];
  }) => send<Comparison>("compare", it),
  startEdit: (endpoint: string, path: string) => send<Edit>("start_edit", { endpoint, path }),
  howToEdit: (name: string) => send<EditRule | null>("how_to_edit", { name }),
  openEdits: () => send<Edit[]>("open_edits"),
  editText: (id: string) => send<string>("edit_text", { id }),
  saveEdit: (id: string, text: string) => send<Edit>("save_edit", { id, text }),
  pushEdit: (id: string, anyway: boolean) => send<Edit>("push_edit", { id, anyway }),
  endEdit: (id: string, deleteCopy: boolean) => send<Edit | null>("end_edit", { id, deleteCopy }),
  endEdits: (deleteCopies: boolean) => send<Edit[]>("end_edits", { deleteCopies }),

  openUrl: (url: string) => invoke<void>("open_url", { url }),
  updateSource: () => send<string>("update_source"),
  newerRelease: (answer: string) => send<Release | null>("newer_release", { answer }),

  settings: () => send<Settings>("settings"),
  setSettings: (value: Settings) => send<void>("set_settings", { value }),

  sites: () => send<Site[]>("sites"),
  siteFolders: () => send<string[]>("site_folders"),
  saveSite: (folder: string, site: Site) => send<string>("save_site", { folder, site }),
  deleteSite: (id: string) => send<void>("delete_site", { id }),
  createSiteFolder: (folder: string) => send<void>("create_site_folder", { folder }),
  renameSiteFolder: (from: string, to: string) => send<void>("rename_site_folder", { from, to }),
  deleteSiteFolder: (folder: string) => send<void>("delete_site_folder", { folder }),
  /**
   * Asking the updater whether there is something it could install.
   *
   * A second question on top of the one this program asks GitHub itself, and
   * they are not the same question. Ours reads the release and its notes;
   * this one asks whether there is a signed artefact for *this* machine, which
   * is what decides whether a button may promise to install anything.
   */
  async canInstallUpdate(): Promise<boolean> {
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      return (await check()) !== null;
    } catch {
      // No manifest, no network, no artefact for this platform. All of them
      // mean the same thing to the window: not from here.
      return false;
    }
  },

  async installUpdate(
    onProgress: (downloaded: number, total: number | null) => void,
  ): Promise<void> {
    const { check } = await import("@tauri-apps/plugin-updater");
    const update = await check();
    if (!update) throw new Error("no update to install");

    let downloaded = 0;
    let total: number | null = null;
    await update.downloadAndInstall((event) => {
      if (event.event === "Started") {
        total = event.data.contentLength ?? null;
        onProgress(0, total);
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
        onProgress(downloaded, total);
      } else {
        onProgress(total ?? downloaded, total);
      }
    });
  },

  restart: () => invoke<void>("restart"),

  // The local side is this computer. There is nowhere to send a file to and
  // nowhere to fetch it from that is not already open in the other pane.
  downloadUrl: () => null,
  uploadInto: () =>
    Promise.reject(new Error("this window and the files are on the same computer")),

  setSiteSecret: (id: string, kind: SecretKind, value: string) =>
    send<void>("set_site_secret", { id, kind, value }),
  forgetSiteSecret: (id: string, kind: SecretKind) =>
    send<void>("forget_site_secret", { id, kind }),
  setSessionSecret: (id: string, kind: SecretKind, value: string) =>
    send<void>("set_session_secret", { id, kind, value }),
  forgetSessionSecret: (id: string, kind: SecretKind) =>
    send<void>("forget_session_secret", { id, kind }),

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
    send<SearchResult>("search", { endpoint, root, needle, limit }),
  rawCommand: (endpoint: string, command: string) =>
    send<RawReply>("raw_command", { endpoint, command }),

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
  async openWith(path: string, program: string | null): Promise<boolean> {
    await invoke<void>("open_with", { path, program });
    return true;
  },
  async onClosing(handler: () => Promise<boolean>): Promise<Unsubscribe> {
    // Prevented only when the answer is to stay. Tauri waits for this handler
    // before it looks, so the question can be asked inside it -- and the
    // window closes by *not* being held back, rather than by being closed
    // again from in here.
    //
    // That second close was the whole of the bug this replaces. A window with
    // a listener on this event never closes natively (Tauri's own runtime
    // calls prevent_close for it), so the only thing that ever ends it is the
    // destroy the API does when nothing prevented the event. Closing it again
    // from inside the handler went round the same loop, and the destroy at the
    // end of it needed a permission the capability did not grant -- so the
    // window stayed, and every later press of the close button did nothing at
    // all.
    return await getCurrentWindow().onCloseRequested(async (event) => {
      if (!(await handler())) event.preventDefault();
    });
  },
  async showEditsFolder(): Promise<boolean> {
    await invoke<void>("show_edits_folder");
    return true;
  },
  async closeThisWindow(): Promise<void> {
    await getCurrentWindow().close();
  },
  async openEditor(id: string, name: string): Promise<boolean> {
    await invoke<void>("open_editor", { id, name });
    return true;
  },
  openSite: (id: string, side: OpenSide) => invoke<void>("open_site", { id, side }),
  async onOpenSite(handler): Promise<Unsubscribe> {
    return await listen<{ id: string; side: OpenSide }>("amberbeam://open-site", (event) =>
      handler(event.payload.id, event.payload.side),
    );
  },

  uiState: () => send<unknown>("ui_state"),
  setUiState: (value: unknown) => send<void>("set_ui_state", { value }),
};
