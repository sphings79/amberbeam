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
  queueMove: (id: string, by?: number, to?: number) => call<void>("queue-move", { id, by, to }),
  queueDecide: (id: string, policy: ConflictPolicy, forAll: boolean) =>
    call<void>("queue-decide", { id, policy, forAll }),

  async openUrl(url: string): Promise<void> {
    // In a browser the window can simply do it, and should: asking the server
    // to open a link would open it on the server.
    window.open(url, "_blank", "noreferrer");
  },

  settings: () => call<Settings>("settings"),
  setSettings: (value: Settings) => call<void>("set-settings", { value }),

  uiState: () => call<unknown>("ui-state"),
  setUiState: (value: unknown) => call<void>("set-ui-state", { value }),
};
