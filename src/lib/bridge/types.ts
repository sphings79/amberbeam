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
  /**
   * Whether this connection stands on a certificate somebody accepted by hand
   * rather than one an authority vouches for. True also when the exception was
   * accepted in an earlier session — the mark belongs to the state, not to the
   * moment it was made.
   */
  certificateAccepted: boolean;
}

export type AuthKind = "password" | "key-file" | "agent";

/** How an FTP connection is encrypted. Meaningless for the other protocols. */
export type Encryption = "none" | "explicit" | "implicit";

export interface ConnectRequest {
  /** Which pane this connection belongs to. */
  endpoint: string;
  /** Absent means SFTP, which is what every request meant before the choice. */
  protocol?: Protocol;
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
  /** FTP only. Ignored when the protocol is plain FTP. */
  encryption?: Encryption;
  /** FTP only. Passive is what works behind a router. */
  passive?: boolean;
  /** FTP only: the server does not speak UTF-8. */
  latin1?: boolean;
  /** FTP only: seconds between keep-alive commands on an idle connection. */
  keepAlive?: number;
  /**
   * Set on a second attempt, after the user compared the fingerprint and
   * accepted it. Applies to that one certificate, never to the host.
   */
  acceptCertificate?: string;
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
  /** null means: whatever the settings say. */
  concurrency: number | null;
  retries: number | null;
  temporaryName: boolean | null;
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
  | {
      kind: "certificate-untrusted";
      host: string;
      fingerprint: string;
      /** Translation key naming what is wrong with it. */
      reason: string;
      /** What the TLS library said, word for word. */
      detail: string;
    }
  | { kind: "encryption-refused"; detail: string }
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
  | { event: "listed"; endpoint: string; path: string }
  | { event: "progress"; jobs: JobProgress[] }
  | { event: "concurrency-lowered"; endpoint: string; allowed: number }
  | { event: "queue" };

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
  /** Ask GitHub now and then whether a newer release exists. */
  checkForUpdates: boolean;
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

export type JobState = "queued" | "running" | "asking" | "paused" | "done" | "failed";

export type ConflictPolicy =
  | "ask"
  | "overwrite"
  | "skip"
  | "overwrite-if-newer"
  | "rename"
  | "resume";

export interface QueuedJob {
  id: string;
  sourceEndpoint: string;
  sourcePath: string;
  targetEndpoint: string;
  targetPath: string;
  /** What the row shows — the path relative to what was selected. */
  name: string;
  state: JobState;
  conflictPolicy: ConflictPolicy;
  totalBytes: number | null;
  doneBytes: number;
  attempts: number;
  retries: number | null;
  failure: CoreError | null;
  /** What is already at the target, when a conflict was found. */
  existingSize: number | null;
  existingModified: number | null;
  /** The source's own timestamp, for comparing the two. */
  sourceModified: number | null;
  added: number;
}

export interface Queue {
  jobs: QueuedJob[];
  /** While true nothing new starts; what is running is left to finish. */
  paused: boolean;
}

export interface Totals {
  waitingJobs: number;
  askingJobs: number;
  runningJobs: number;
  pausedJobs: number;
  doneJobs: number;
  failedJobs: number;
  doneBytes: number;
  totalBytes: number;
}

/** How far one running transfer has come. */
export interface JobProgress {
  id: string;
  doneBytes: number;
  totalBytes: number | null;
  /** Bytes per second, once there is enough history to say. */
  rate: number | null;
}

export interface EnqueueRequest {
  sourceEndpoint: string;
  sourceDirectory: string;
  names: string[];
  targetEndpoint: string;
  targetDirectory: string;
}

/** A release newer than the one running. */
export interface Release {
  tag: string;
  version: string;
  url: string;
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

  /**
   * Files dropped onto the window from outside it.
   *
   * Not an HTML drop event: the desktop shell intercepts those so it can hand
   * over real paths instead of sandboxed file handles. The position is in
   * device pixels, so the window can work out which pane was hit.
   */
  onFileDrop(
    handler: (paths: string[], position: { x: number; y: number }) => void,
  ): Promise<Unsubscribe>;

  /** Puts what was selected into the queue; answers how many files that is. */
  enqueue(request: EnqueueRequest): Promise<number>;
  queueSnapshot(): Promise<Queue>;
  queueTotals(): Promise<Totals>;
  queuePause(paused: boolean): Promise<void>;
  queueHold(id: string): Promise<void>;
  queueResume(id: string): Promise<void>;
  queueRemove(id: string): Promise<void>;
  queueClearFinished(): Promise<void>;
  /** Empties the queue, stopping what is running. */
  queueClearAll(): Promise<void>;
  queueMove(id: string, by?: number, to?: number): Promise<void>;
  queueDecide(id: string, policy: ConflictPolicy, forAll: boolean): Promise<void>;

  /** Opens a web address in the system's browser. */
  openUrl(url: string): Promise<void>;

  /** Where to ask about releases. Named by the core, not by the window. */
  updateSource(): Promise<string>;
  /** Judges an answer from GitHub against the version running. */
  newerRelease(answer: string): Promise<Release | null>;

  settings(): Promise<Settings>;
  setSettings(value: Settings): Promise<void>;

  /** Whatever the window wants back on the next start. */
  uiState(): Promise<unknown>;
  setUiState(value: unknown): Promise<void>;
}
