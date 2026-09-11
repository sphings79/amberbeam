/** Bridge implementation for the desktop build: Tauri's internal channel. */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type {
  AmberBeamApi,
  Connected,
  ConnectRequest,
  CoreEvent,
  CoreInfo,
  Listing,
  Measurement,
  QuickConnectEntry,
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

  uiState: () => invoke<unknown>("ui_state"),
  setUiState: (value: unknown) => invoke<void>("set_ui_state", { value }),
};
