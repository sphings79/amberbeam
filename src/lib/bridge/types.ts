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
  /**
   * Set when this connection comes from a site entry. The shell uses it to
   * fetch the stored password, which is why none needs to be passed here.
   */
  siteId?: string;
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
  /** FTP only: how the connection was encrypted. */
  encryption: Encryption | null;
  passive: boolean | null;
  latin1: boolean | null;
  keepAlive: number | null;
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
  | { kind: "timed-out"; host: string; port: number; seconds: number }
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
  /**
   * What the release says about itself, as written.
   *
   * Text from the network, and treated as nothing else: the window builds its
   * own structure from it and never hands it to the browser as markup.
   */
  notes: string;
}

/**
 * A site entry, exactly as it sits on disk — with two things added that are
 * not part of the file.
 *
 * `folder` is where it is filed, which on disk is simply the directory the file
 * is in. `hasPassword` says whether the credential store holds one, and
 * `hasSessionPassword` whether one was typed for this run only — two different
 * promises, kept apart so the window can say which it is making. The password
 * itself never comes to the window, because the window has no use for the
 * value, only for the connection it opens.
 */
export interface Site {
  id: string;
  name: string;
  folder: string;
  hasPassword: boolean;
  hasSessionPassword: boolean;
  protocol: Protocol;
  host: string;
  port: number;
  user: string;
  auth: AuthKind;
  keyPath: string | null;
  remotePath: string | null;
  localPath: string | null;
  concurrency: number;
  retries: number | null;
  temporaryName: boolean | null;
  encryption: Encryption | null;
  passive: boolean | null;
  latin1: boolean | null;
  keepAlive: number | null;
  rememberPassword: boolean;
  /** One of the interface's accent names, or null for no marking. */
  colour: string | null;
}

/** Where an importable list came from. */
export type ImportSource =
  | "ssh-config"
  | "winscp"
  | "wcx-ftp"
  | "filezilla"
  | "sites-dat"
  | "sites-xml";

/** A file that looks like one of those lists. */
export interface ImportCandidate {
  source: ImportSource;
  path: string;
}

/**
 * One server out of somebody else's file.
 *
 * `hasPassword` says whether one was found and could be read; the value stays
 * in the core, which moves it into the credential store without it ever
 * reaching a window.
 */
export interface ImportedEntry {
  name: string;
  folder: string;
  protocol: Protocol;
  host: string;
  port: number;
  user: string;
  auth: AuthKind;
  keyPath: string | null;
  remotePath: string | null;
  encryption: Encryption | null;
  hasPassword: boolean;
}

/** What one file turned out to hold. */
export interface ImportPreview {
  source: ImportSource;
  path: string;
  entries: ImportedEntry[];
  /** Translation keys for anything the reader could not do. */
  warnings: string[];
}

/** One entry of an AmberBeam export, as the preview shows it. */
export interface BundleRow {
  folder: string;
  name: string;
  host: string;
  user: string;
  port: number;
  hasPassword: boolean;
}

/**
 * What an export holds.
 *
 * `sealed` with no entries means the passphrase is still wanted: whether a file
 * is sealed can be seen from its first bytes, so nobody is prompted for a
 * passphrase that does not exist.
 */
export interface BundlePreview {
  sealed: boolean;
  entries: BundleRow[];
}

/** One thing a search turned up. */
export interface SearchMatch {
  path: string;
  name: string;
  kind: EntryKind;
  size: number | null;
  modified: number | null;
}

/** What a search found, and whether it got to the end. */
export interface SearchResult {
  matches: SearchMatch[];
  /** How many directories were read. Over FTP that is what it cost. */
  directories: number;
  /** True when a bound was reached rather than the tree ending. */
  truncated: boolean;
}

/** What the server answered a hand-typed command. */
export interface RawReply {
  code: number;
  text: string;
  /**
   * Whether the command opened a data connection, and the control connection
   * was therefore closed rather than reused.
   */
  connectionDropped: boolean;
}

/** Which secret of an entry is meant. */
export type SecretKind = "password" | "passphrase";

/** Which pane a site is to be opened in. */
export type OpenSide = "left" | "right";

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
  // --- The site manager ---

  /** Every entry, with the folder it sits in. Never a password. */
  sites(): Promise<Site[]>;
  siteFolders(): Promise<string[]>;
  /**
   * Writes an entry, moving or renaming its file when either changed, and
   * answers with its identifier — which the core assigns when the entry is new,
   * because that identifier is what the credential store files the password
   * under and one place has to be in charge of it.
   */
  saveSite(folder: string, site: Site): Promise<string>;
  /** Removes an entry, and with it whatever the credential store held for it. */
  deleteSite(id: string): Promise<void>;
  createSiteFolder(folder: string): Promise<void>;
  renameSiteFolder(from: string, to: string): Promise<void>;
  /** Only an empty one — a folder full of servers deserves its own question. */
  deleteSiteFolder(folder: string): Promise<void>;
  /** One way only: a secret goes in, and never comes back out here. */
  /**
   * Whether this installation can replace itself.
   *
   * Not every one can. A `.deb` sits under `/usr` and cannot be rewritten
   * without root, so there the honest answer is no and the window offers the
   * download page instead of a button that would fail halfway.
   */
  canInstallUpdate(): Promise<boolean>;

  /**
   * Fetches the new version and installs it.
   *
   * Nothing is written until the download has been checked against the public
   * key built into this program. The progress callback is for showing where it
   * has got to; a download that stops silently is the one thing worse than a
   * slow one.
   */
  installUpdate(onProgress: (downloaded: number, total: number | null) => void): Promise<void>;

  /** Starts the program again, once an update has been written. */
  restart(): Promise<void>;

  setSiteSecret(id: string, kind: SecretKind, value: string): Promise<void>;
  forgetSiteSecret(id: string, kind: SecretKind): Promise<void>;

  /**
   * Holds a secret for this run of the program and no longer.
   *
   * For somebody who typed a password without asking for it to be kept. It
   * lives in the core rather than in the window that took it: the site list is
   * a window of its own, and what it holds cannot be reached from the window
   * that connects.
   */
  setSessionSecret(id: string, kind: SecretKind, value: string): Promise<void>;
  forgetSessionSecret(id: string, kind: SecretKind): Promise<void>;

  // --- Importing somebody else's list ---

  /**
   * Asks the system for a file, and answers with its path or null when the
   * person changed their mind.
   *
   * The one place a native dialog is worth the dependency: somebody whose
   * server list lives in a folder nobody would look in has no other way to
   * point at it than to type the path.
   */
  chooseFile(title: string): Promise<string | null>;
  /** Asks the system where to write a file. */
  chooseSaveFile(title: string, suggested: string): Promise<string | null>;

  /** Files that look like a server list, in the places they usually are. */
  importCandidates(): Promise<ImportCandidate[]>;
  /** What one of them holds. Never the passwords. */
  importPreview(source: ImportSource, path: string): Promise<ImportPreview>;
  /**
   * Takes the ticked entries over, and answers how many.
   *
   * `expected` is how many entries the preview showed: the file is read again
   * rather than the preview trusted, which is what keeps the passwords out of
   * the window, and the count guards against it having changed in between.
   */
  importApply(
    source: ImportSource,
    path: string,
    chosen: number[],
    expected: number,
    takePasswords: boolean,
    into: string,
  ): Promise<number>;

  // --- Taking the list with you ---

  /**
   * Writes the whole list to one file, and answers how many entries.
   *
   * Two shapes and no third: without passwords it is plain JSON anybody can
   * read, with them it is sealed under a passphrase. Asking for passwords
   * without a passphrase is refused rather than quietly written in the open.
   */
  exportSites(path: string, withPasswords: boolean, passphrase: string | null): Promise<number>;
  /** Looks into an export. Never a password. */
  bundlePreview(path: string, passphrase: string | null): Promise<BundlePreview>;
  bundleApply(
    path: string,
    passphrase: string | null,
    chosen: number[],
    into: string,
  ): Promise<number>;

  /**
   * Looks for a name through a whole tree, without regard to case.
   *
   * Bounded on both sides — enough matches, or enough directories — and says
   * which bound it hit. Over FTP every directory is a separate data
   * connection, so this is not free and does not pretend to be.
   */
  search(endpoint: string, root: string, needle: string, limit: number): Promise<SearchResult>;

  /**
   * Sends one command exactly as typed.
   *
   * Only FTP has such a thing; anything else refuses and says why.
   */
  rawCommand(endpoint: string, command: string): Promise<RawReply>;

  // --- The keyboard ---

  /**
   * Opens the system settings pane a key scheme needs.
   *
   * It only opens the pane. Turning the setting on is the person's own doing:
   * no program should be able to change how somebody's keyboard behaves.
   */
  openSystemKeyboard(pane: "function-keys" | "shortcuts"): Promise<void>;
  /** Turns full screen on or off, and answers what it now is. */
  toggleFullscreen(): Promise<boolean>;
  /** Writes a file the user picked. Used for key schemes. */
  writeTextFile(path: string, text: string): Promise<void>;
  readTextFile(path: string): Promise<string>;

  /**
   * Which window this is — "main", "sites", or whatever the shell calls it.
   *
   * The view used to be chosen by a query string in the window's URL, which
   * cost an entire Windows release: that URL is a path, and a question mark is
   * illegal in one there. A label is not a path.
   */
  windowLabel(): string;

  /** Opens the site manager in a window of its own. */
  openSiteManager(): Promise<void>;
  /** Asks the main window to open this entry on that side. */
  openSite(id: string, side: OpenSide): Promise<void>;
  /** Heard by the main window when the site manager asks for a connection. */
  onOpenSite(handler: (id: string, side: OpenSide) => void): Promise<Unsubscribe>;

  uiState(): Promise<unknown>;
  setUiState(value: unknown): Promise<void>;
}
