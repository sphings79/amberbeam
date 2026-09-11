/**
 * The whole surface the user interface has towards the core.
 *
 * Seam 2 (concept paper, section 09): every implementation of `AmberBeamApi`
 * has to satisfy this interface — the one going through Tauri's channel on the
 * desktop, and the one going through HTTP and WebSocket in the container build
 * of milestone M7. Nothing outside `src/lib/bridge/` may know which of them is
 * in use; `scripts/check-seams.mjs` enforces that.
 */

export type Protocol = "local" | "sftp" | "ftp" | "ftps";

export interface ProtocolInfo {
  protocol: Protocol;
  defaultPort: number | null;
  defaultConcurrency: number;
  encrypted: boolean;
}

export interface CoreInfo {
  version: string;
  arch: string;
  os: string;
  protocols: ProtocolInfo[];
}

/** The endpoint identifier the local file system always has. */
export const LOCAL = "local";

export type EntryKind = "file" | "directory" | "symlink" | "other";

export interface DirEntry {
  name: string;
  kind: EntryKind;
  size: number | null;
  /** Seconds since the Unix epoch, or null where the endpoint reports none. */
  modified: number | null;
  /** The nine permission bits as a number, or null. */
  permissions: number | null;
  owner: string | null;
  group: string | null;
  linkTarget: string | null;
  kindOfTarget: EntryKind | null;
}

export interface Listing {
  path: string;
  entries: DirEntry[];
}

export interface Connected {
  endpoint: string;
  protocol: Protocol;
  home: string;
}

export type AuthKind = "password" | "key-file" | "agent";

export interface ConnectRequest {
  /** Which pane this connection belongs to. */
  endpoint: string;
  host: string;
  port: number;
  user: string;
  auth: AuthKind;
  /** Never stored anywhere; travels to the core and no further. */
  password?: string;
  keyPath?: string;
  passphrase?: string;
  /** Set on a second attempt, after the user accepted the fingerprint. */
  acceptFingerprint?: string;
  /** Transfers at once for this connection; falls back to the settings. */
  concurrency?: number;
  retries?: number;
  temporaryName?: boolean;
}

export interface QuickConnectEntry {
  id: string;
  protocol: Protocol;
  host: string;
  port: number;
  user: string;
  auth: AuthKind;
  keyPath: string | null;
  lastPath: string | null;
  concurrency: number | null;
  retries: number | null;
  lastUsed: number;
  savedAsSite: boolean;
}

/**
 * What went wrong, as a kind plus the details that belong in the sentence.
 *
 * The core never sends finished text: `kind` picks the translation key and the
 * rest fills the placeholders. That is the same rule as section 11 — no text
 * stands in the code, not even in the Rust half.
 */
export type CoreError =
  | { kind: "unreachable"; host: string; port: number }
  | { kind: "authentication-failed"; user: string }
  | { kind: "key-file"; path: string }
  | { kind: "key-passphrase"; path: string }
  | { kind: "agent" }
  | { kind: "host-key-unknown"; host: string; fingerprint: string }
  | {
      kind: "host-key-changed";
      host: string;
      fingerprint: string;
      knownFingerprint: string;
    }
  | { kind: "path"; path: string; reason: PathProblem }
  | { kind: "source-changed" }
  | { kind: "disconnected" }
  | { kind: "not-connected" }
  | { kind: "other"; detail: string };

export type PathProblem =
  | "not-found"
  | "permission-denied"
  | "not-a-directory"
  | "already-exists"
  | "unknown";

export type LogDirection = "sent" | "received" | "note";

export type CoreEvent =
  | { event: "log"; endpoint: string; direction: LogDirection; text: string }
  | ({ event: "connection"; endpoint: string } & ConnectionState)
  | { event: "listed"; endpoint: string; path: string };

export type ConnectionState =
  | { state: "connecting" }
  | { state: "connected"; banner: string | null }
  | { state: "disconnected" }
  | { state: "failed"; error: CoreError };

/** What applies when a connection says nothing of its own. */
export interface Settings {
  /** Transfers at once. null lets the protocol decide: SFTP 8, FTP 4. */
  concurrency: number | null;
  retries: number;
  keepModified: boolean;
  keepPermissions: boolean;
  /** Write to a temporary name and rename when the file is whole. */
  temporaryName: boolean;
}

/** What a recursive delete is about to remove. */
export interface Measurement {
  files: number;
  directories: number;
  symlinks: number;
  bytes: number;
  /** True when counting stopped early, so the window says "more than". */
  truncated: boolean;
}

/** Stops a subscription. */
export type Unsubscribe = () => void;

/** Which shell the frontend is running in. */
export type Shell = "desktop" | "web";

export interface AmberBeamApi {
  readonly shell: Shell;

  /** Version and capabilities of the core behind this window. */
  coreInfo(): Promise<CoreInfo>;

  /**
   * Everything the core pushes on its own: the server log above all.
   *
   * Request and answer are not enough for a log — it arrives while nothing was
   * asked. On the desktop this rides Tauri's channel, in the container build a
   * WebSocket.
   */
  subscribe(handler: (event: CoreEvent) => void): Promise<Unsubscribe>;

  /** The local file system, reported the way a connection would be. */
  localSession(): Promise<Connected>;

  connect(request: ConnectRequest): Promise<Connected>;
  disconnect(endpoint: string): Promise<void>;

  listDir(endpoint: string, path: string): Promise<Listing>;
  parentOf(endpoint: string, path: string): Promise<string | null>;
  joinPath(endpoint: string, directory: string, name: string): Promise<string>;

  createDir(endpoint: string, directory: string, name: string): Promise<void>;
  createFile(endpoint: string, directory: string, name: string): Promise<void>;
  renameEntry(endpoint: string, directory: string, from: string, to: string): Promise<void>;
  /** Counts a tree before it is deleted, so the warning can say how much. */
  measure(endpoint: string, path: string): Promise<Measurement>;
  removeEntry(endpoint: string, path: string): Promise<void>;
  setPermissions(
    endpoint: string,
    path: string,
    mode: number,
    recursive: boolean,
  ): Promise<void>;

  quickConnectHistory(): Promise<QuickConnectEntry[]>;
  forgetQuickConnect(id: string): Promise<void>;
  /** Writes a site entry and returns where it landed. */
  saveAsSite(id: string): Promise<string>;
  rememberPath(id: string, path: string): Promise<void>;

  settings(): Promise<Settings>;
  setSettings(value: Settings): Promise<void>;

  /** Whatever the window wants back on the next start. */
  uiState(): Promise<unknown>;
  setUiState(value: unknown): Promise<void>;
}
