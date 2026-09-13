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
  | { kind: "encryption-required"; host: string; detail: string }
  | { kind: "wastebasket-failed"; detail: string }
  | { kind: "type-not-edited"; path: string; extension: string }
  | { kind: "not-text-to-edit"; path: string }
  | { kind: "too-big-to-edit"; path: string; megabytes: number }
  | { kind: "edit-changed-on-server"; path: string }
  | { kind: "text-does-not-fit"; character: string }
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
  | { event: "comparing"; directories: number; rows: number }
  | { event: "watched"; id: string; sent: number; refused: number }
  | {
      event: "edited";
      id: string;
      name: string;
      what: "pushed";
    }
  | { event: "edited"; id: string; name: string; what: "changed"; path: string }
  | { event: "edited"; id: string; name: string; what: "failed"; error: CoreError }
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
  /**
   * What to do about a file that is already there, decided for good.
   *
   * Null is asking, which is the default and stays it. This exists because
   * answering the same question forty times while a folder walks is not
   * consent.
   */
  conflictPolicy: ConflictPolicy | null;
  /**
   * Take a finished line out of the queue by itself, after a moment.
   *
   * Off by default. A queue that empties itself is tidy right up until
   * somebody wants to know whether the thing they started actually happened.
   */
  clearFinished: boolean;
  /**
   * What may be edited where it lies, and what opens it.
   *
   * Emptied by hand it stays empty, and then nothing is editable — a setting
   * somebody made, not a state to be repaired behind their back.
   */
  editing: EditRule[];
  /**
   * Show what a comparison found before anything is queued.
   *
   * On by default: four hundred files arriving in the queue unasked is not a
   * decision anybody made.
   */
  reviewComparison: boolean;
  /**
   * Carry a deletion across: a file gone here goes there too.
   *
   * Off, and it stays off unless somebody says otherwise — a checkout or a
   * build that cleans up after itself would take files off a server.
   */
  deleteAlong: boolean;
  /** Whether a program driving this may connect to a server not in the list. */
  mcpQuickConnect: boolean;
  /** Whether it may write new entries into the list. */
  mcpCreateSites: boolean;
}

/** What a recursive delete is about to remove. */
/** What happened to something that was deleted. */
export interface Removed {
  /** Where it went, or null when it is gone for good. */
  movedTo: string | null;
}

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
  /**
   * Line it up without setting it going.
   *
   * For somebody gathering a few things first and then starting them, rather
   * than watching each one leave as it is dropped.
   */
  held?: boolean;
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
  /**
   * Open where this server was last left instead of at `remotePath`.
   *
   * Off by default: somebody who typed a starting directory meant it.
   */
  rememberPath: boolean;
  /**
   * Where it was last left, when the entry asked for that to be remembered.
   *
   * Filled by the core, never by a window, and null when the entry did not ask
   * or has not been anywhere yet. Whichever of the two paths arrives, opening
   * is the same journey — the window does not know which rule produced it.
   */
  lastPath: string | null;
  localPath: string | null;
  concurrency: number;
  retries: number | null;
  temporaryName: boolean | null;
  encryption: Encryption | null;
  passive: boolean | null;
  latin1: boolean | null;
  keepAlive: number | null;
  rememberPassword: boolean;
  /**
   * A directory on the server that deleted files are moved into instead.
   *
   * Null or empty deletes for good, which stays the default: a program that
   * quietly kept everything somebody deleted would be filling a disk they
   * thought they were clearing.
   */
  wastebasket: string | null;
  /**
   * Names a comparison and a watch never look at, as patterns.
   *
   * Per server, because what counts as noise in a web project is not what
   * counts as noise in a backup.
   */
  excludes: string[];
  /**
   * Whether a program driving AmberBeam over MCP may use this server.
   *
   * Off. A server nobody opened is one such a program cannot see, cannot list
   * and cannot name — absent rather than refused.
   */
  mcp: boolean;
  /** Whether it may change anything there: send a file, make a directory. */
  mcpWrite: boolean;
  /** Whether it may delete there. Its own switch, and the last to turn on. */
  mcpDelete: boolean;
  /** One of the interface's accent names, or null for no marking. */
  colour: string | null;
}

/** What two sides are judged by when they are compared. */
export type How = "size" | "size-and-time" | "checksum";

/** What a comparison found about one name. */
export type Difference = "only-here" | "only-there" | "different" | "same";

/** One side of one row of a comparison. */
export interface Seen {
  size: number | null;
  modified: number | null;
}

/** One name, as both sides have it. */
export interface CompareRow {
  /** Where it sits below the two directories, with forward slashes. */
  path: string;
  name: string;
  kind: EntryKind;
  state: Difference;
  here: Seen | null;
  there: Seen | null;
}

/** What a comparison found. */
export interface Comparison {
  rows: CompareRow[];
  /** Directories listed, both sides together. */
  directories: number;
  /**
   * True when the walk stopped at its own limit rather than at the end.
   *
   * Has to be shown. Everything missing from a list that is quietly
   * incomplete looks exactly like agreement.
   */
  cutShort: boolean;
}

/**
 * One directory on this machine being watched, and where what changes goes.
 *
 * Only ever the local side. Neither FTP nor SFTP has any way to say "something
 * changed", so watching a server would mean listing it over and over — a
 * standing load on somebody else's machine rather than a background service.
 */
export interface Watch {
  id: string;
  endpoint: string;
  root: string;
  targetEndpoint: string;
  targetRoot: string;
  /** What the far side is called, as the pane shows it. */
  targetTitle: string | null;
  /** Files sent since this started. */
  sent: number;
  started: number;
}

/** What opens a file of a given kind. */
export type OpenWith = "own" | "system" | "program";

/**
 * One line of the table that says what may be edited, and with what.
 *
 * The same list answers both questions on purpose: "which files are text" and
 * "what opens them" are one decision with two halves, and two lists would
 * eventually disagree about the same file.
 */
export interface EditRule {
  /** Extensions without the dot, or whole names for files that have none. */
  extensions: string[];
  openWith: OpenWith;
  /** The program, when the line says `program`. A path on the machine the
      core runs on, which is why it means nothing in a browser. */
  program: string | null;
}

/**
 * One file taken off a server to be worked on.
 *
 * The copy on this machine is the thing an editor opens; `remotePath` is where
 * it came from and where it goes back to. Nothing here says how it is being
 * edited — that is the window's business, and the core does not need to know.
 */
export interface Edit {
  id: string;
  endpoint: string;
  remotePath: string;
  name: string;
  /** Where the copy lies on this machine. */
  localPath: string;
  /**
   * What the file turned out to be written in, read off its bytes.
   *
   * It is written back the same way. A Latin-1 file read as UTF-8 and saved as
   * UTF-8 has every umlaut in it rewritten, and nobody notices until later.
   */
  encoding: "utf8" | "latin1";
  /** Seconds since the epoch, for showing the list in the order it grew. */
  opened: number;
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
  /**
   * Deletes, or moves into the server's wastebasket when it has one.
   *
   * `siteId` is passed always and the core decides. A rule about what
   * "delete" means belongs in one place; spread across the callers, one of
   * them would eventually delete something somebody expected to find again.
   */
  removeEntry(endpoint: string, path: string, siteId?: string | null): Promise<Removed>;
  setPermissions(
    endpoint: string,
    path: string,
    mode: number,
    recursive: boolean,
  ): Promise<void>;

  /**
   * Takes a copy of a server's file to work on.
   *
   * Asking twice for the same file gives back the same copy. The core refuses
   * what is not text and what is too large — a rule about what may be edited
   * belongs where it can be applied once, not in each window that asks.
   */
  /**
   * Walks two directories and says what differs.
   *
   * Sends `comparing` events as it goes, because a recursive walk of two trees
   * is many listings and no other sign of life.
   */
  compare(it: {
    hereEndpoint: string;
    herePath: string;
    thereEndpoint: string;
    therePath: string;
    recursive: boolean;
    how: How;
    excludes: string[];
  }): Promise<Comparison>;

  /**
   * Watches a directory on this machine and sends up what changes in it.
   *
   * Asking twice for the same directory gives back the same watch: two
   * watchers on one tree would send every change twice, and the second upload
   * would arrive while the first was still going.
   */
  startWatch(it: {
    root: string;
    targetEndpoint: string;
    targetRoot: string;
    targetTitle: string | null;
    /** The far side's server entry, which is what says whether a deletion
        goes into a wastebasket. Passed always; the core decides. */
    siteId: string | null;
    excludes: string[];
  }): Promise<Watch>;
  stopWatch(id: string): Promise<Watch | null>;
  watches(): Promise<Watch[]>;

  startEdit(endpoint: string, path: string): Promise<Edit>;
  /**
   * Which line of the table covers a file, or null for none.
   *
   * Asked rather than worked out here. A window deciding for itself would end
   * up offering a file the core refuses, or greying out one it would take.
   */
  howToEdit(name: string): Promise<EditRule | null>;
  openEdits(): Promise<Edit[]>;
  editText(id: string): Promise<string>;
  /**
   * Writes the copy and sends it back.
   *
   * Fails with `edit-changed-on-server` when the file up there is no longer
   * the one that was taken. The typing is safe in the copy either way, so the
   * window can ask and then call `pushEdit` with `anyway`.
   */
  saveEdit(id: string, text: string): Promise<Edit>;
  pushEdit(id: string, anyway: boolean): Promise<Edit>;
  endEdit(id: string, deleteCopy: boolean): Promise<Edit | null>;
  endEdits(deleteCopies: boolean): Promise<Edit[]>;

  quickConnectHistory(): Promise<QuickConnectEntry[]>;
  forgetQuickConnect(id: string): Promise<void>;
  /** Writes a site entry and returns where it landed. */
  saveAsSite(id: string): Promise<string>;
  /**
   * Records where a pane is now.
   *
   * A quick connect entry always keeps it; a saved server keeps it only when
   * it asked to, which is why `siteId` goes along and the core decides.
   */
  rememberPath(id: string, path: string, siteId?: string | null): Promise<void>;

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

  /**
   * Where a browser can fetch one of this machine's files, or null.
   *
   * Null in the desktop program, where the local side already is your disk
   * and "downloading" it would mean copying a file onto itself. Only this
   * machine's own files: reading from a server means holding a data
   * connection open for as long as a browser takes, and moving a file between
   * a server and here is what the queue is for.
   */
  downloadUrl(endpoint: string, path: string): string | null;

  /**
   * Puts a file from the viewer's own computer into a directory here.
   *
   * The container build. In the desktop program the two are the same computer
   * and this refuses rather than pretending.
   */
  uploadInto(endpoint: string, directory: string, file: File): Promise<void>;

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
  /**
   * Shows a file that is being edited.
   *
   * A window of its own on the desktop, so two files can sit side by side.
   * A browser has no second window of the same session to open, so the web
   * shell answers false and the caller shows the editor where it is.
   */
  openEditor(id: string, name: string): Promise<boolean>;
  /**
   * Closes the window this page is in, where there is one to close.
   *
   * Checked against Tauri's own list rather than assumed: closing a window is
   * not part of its default permissions, so the capability names it.
   */
  closeThisWindow(): Promise<void>;
  /**
   * Asks before the program is closed, where there is a closing to catch.
   *
   * The handler says whether to go ahead. A browser has none of this: closing
   * a tab is not the program ending — the service carries on without it — and
   * a browser will not let a page ask a question of its own on the way out.
   */
  onClosing(handler: () => Promise<boolean>): Promise<Unsubscribe>;
  /**
   * Hands a copy that is being edited to another program.
   *
   * False where there is no other program to hand it to, which is a browser:
   * the service is on a different machine, and what it could start there is
   * not what somebody sitting here meant.
   */
  openWith(path: string, program: string | null): Promise<boolean>;
  /**
   * The path of the program that is running, for the line a client needs.
   *
   * Null where there is nothing to point at: a browser cannot say what runs
   * on the machine serving it, and the answer there is a different one.
   */
  mcpCommand(): Promise<string | null>;
  /**
   * Shows the directory the copies being edited live in.
   *
   * Takes nothing: the core knows where it put them, and "open this folder"
   * with a path in it is a call that can be pointed anywhere. False where
   * there is no file manager of this person's to open it in.
   */
  showEditsFolder(): Promise<boolean>;
  /** Asks the main window to open this entry on that side. */
  openSite(id: string, side: OpenSide): Promise<void>;
  /** Heard by the main window when the site manager asks for a connection. */
  onOpenSite(handler: (id: string, side: OpenSide) => void): Promise<Unsubscribe>;

  uiState(): Promise<unknown>;
  setUiState(value: unknown): Promise<void>;
}
