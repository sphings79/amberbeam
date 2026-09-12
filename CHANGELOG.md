# Changelog

[Deutsch](CHANGELOG.de.md)

Every release until 1.0 is a pre-release, and that is a statement about the
state of the program rather than a technicality: it does what it says, it is
tested, and it is not finished. The update notice inside AmberBeam reads
pre-releases for exactly that reason.

## 0.1.3

Getting to a saved server, which until now meant knowing where to look.

### Changed

- **Connect offers the servers you have.** Pressing it used to open a form for
  typing a host into, which is the one thing somebody with a saved server does
  not want to do — their own servers were not reachable from the button whose
  entire purpose is connecting. It now opens a menu: saved servers first,
  grouped by folder, with quick connect and the server list below them. The key
  and the button open the same menu in the same place.
- **The server list has a button where a main button belongs** — in each pane's
  header, and labelled in a new bar along the top. It used to sit in the footer
  between appearance and help, among the things nobody opens twice a week.
  Appearance, settings and keys move up beside it, on the right.
- **The password field in a server's settings is always there.** It only
  appeared once "remember" was ticked, which reads as a bug: a box that arrives
  unannounced explains nothing about why it was not there before. The tick now
  decides how long a password is kept, not whether one can be given at all.

  Without it, the password is held for this run of the program and no further —
  in memory, never written anywhere — and the field says which of the two
  promises is being made. Keeping one for good clears the copy that was only
  for this run.
- **Taking a one-off connection into the server list** was a star with a
  tooltip. Nobody hovers a symbol to find out whether it is the one they want,
  so it says what it does.

### Added

- **The update check reports where it stands.** It used to speak up only when
  there was news, so silence meant three different things: not asked yet,
  nothing newer, or GitHub could not be reached. Claiming to be up to date
  because the request failed is a guess wearing the clothes of an answer, so
  "could not check" is a state of its own and pressing the button tries again.
  Pressing it asks even when the automatic check at start is switched off —
  that setting is about asking unprompted, which a press is not.

### Fixed

- The hint under the quick connect password named the macOS keychain, which is
  not what the credential store is called on Windows or Linux, and called the
  server list the site manager, which is not what this program calls it
  anywhere else.

## 0.1.2

Windows and Linux were still unusable in 0.1.1. The explanation given there was
wrong, and this says what it actually was.

### Fixed

- **The server window still opened blank on Windows.** It was built from a
  synchronous command, which Tauri's own documentation warns against: on
  Windows that deadlocks, because WebView2 wants the main thread and the
  command is already holding it. So the window appeared and nothing in it ever
  started — frozen in 0.1.0, blank in 0.1.1.

  The question mark in the path, blamed for this in 0.1.1, was a real mistake
  and worth fixing, but it was not this one.
- **Every keyboard shortcut was unreachable outside macOS.** They were bound to
  Meta, which is Command on a Mac, the Windows key on Windows and Super on
  Linux — keys the system takes for itself before any program sees them. The
  status bar advertised one of them as "⌘S" on a machine that has no such key.

  The table no longer says which physical key the command modifier is: Command
  on a Mac, Ctrl everywhere else, decided when the program starts. Shortcuts
  are written in the words of the keyboard in front of you rather than in Mac
  symbols, and the two bindings that could not exist away from a Mac —
  full screen, and deleting with Backspace — have answers that work there.

  Keys you chose yourself are carried across unchanged.
- **The dialog on the first start explained Mission Control to people on
  Windows.** It exists for one problem — on a Mac, F1 to F12 are not function
  keys — and it was shown everywhere regardless, down to buttons offering to
  open the macOS system settings. It now appears only where that problem
  exists. Elsewhere a new installation simply starts on the layout of the older
  Windows clients, which is what this program is for.

### Added

- A window that cannot start says so. Any error before or during startup is
  written into the page, and if nothing has been drawn after eight seconds it
  says that too. Its silence is what finally located the fault above: a page
  that never started cannot report anything, including this.

### Changed

- The keyboard page says outright that none of the macOS section applies to
  Windows or Linux, and the shortcuts written out in the other pages give both
  keyboards.
- The guard script checks the keyboard against both keyboards rather than the
  one the machine running it happens to have. That gap is how this shipped.

## 0.1.1

Windows was unusable in 0.1.0, and this is why.

### Fixed

- **The server window opened empty and froze, and took the rest of the program
  with it.** A window is given a *path* to load, and the path was
  `index.html?view=sites`. A question mark is an ordinary character in a path
  on macOS and an illegal one on Windows, so the window had nowhere to load
  from. Which view a window shows now comes from its label instead.

  That one mistake caused all three of the things reported. Every command's
  answer travels back to the window through the same thread the frozen window
  had blocked — so the settings dialog stayed empty, and connecting sat at
  "connecting…" even when the connection itself had long since been made.
- **Connecting could wait for ever**, and that was never a Windows problem. A
  server that accepts the connection and then says nothing — a firewall that
  swallows rather than refuses, a port belonging to some other program — was
  waited for until somebody gave up. Twenty seconds now covers the whole of
  saying hello, and the message says that something *did* answer, which is what
  tells you to look at a firewall rather than at the address.
- **Listing servers and looking for importable files** no longer run on the
  thread that draws. The first asks the credential store once per entry; the
  second walks Downloads, Desktop and Documents two levels deep.

### Changed

- The macOS disk image no longer demands agreement to the whole AGPL before it
  will open. The licence governs distribution, not use.
- The Linux package files its 256-pixel icon under `256x256` rather than
  `256x256@2`, which is a directory name no desktop recognises.

## 0.1.0

The first public build. Everything below works and has been used; what is
missing is listed at the end.

### Transferring

- **SFTP, FTPS and FTP.** Explicit `AUTH TLS` by default, implicit on port 990,
  and plain FTP where a server offers nothing else — marked in red for as long
  as it is open.
- **Several files at once**, set per server. Eight over SFTP, four over FTP, and
  when a server refuses another login AmberBeam lowers the number itself and
  says so rather than letting the rest of the queue end in errors.
- **A queue that survives a restart.** Close the program mid-transfer and what
  was left is still there.
- **Broken transfers continue** from where they stopped — but only when the far
  end is still the same size and age. If it changed, the transfer starts again
  rather than stitching two versions together.
- **Conflicts are asked about**, with "do the same for the rest" on the same
  dialog.
- Drag files between the panes, or in from the Finder.

### Servers

- **A server list in folders**, one readable JSON file per entry, and never a
  password in any of them: those go to the Keychain, the Credential Manager or
  the Secret Service.
- **Import from five other programs**: FileZilla, WinSCP, Total Commander,
  OpenSSH, and the `Sites.dat` or `.ftp` export of an older Windows client.
  Folders come along.
- **Export** as plain JSON without passwords, or sealed under a passphrase with
  them. There is no third shape.
- Key files including PuTTY's `.ppk`, both versions, encrypted or not.

### Checking who is on the other end

- **SSH host keys** against `~/.ssh/known_hosts`. A changed key is refused.
- **TLS certificates** against the system's own trust store. What is wrong is
  named — self-signed, expired, not yet valid, wrong name, revoked, broken
  signature — rather than collapsed into "certificate error". Accepting applies
  to one certificate on one host and port.
- FTPS encrypts the data channel too, or the connection fails and says which
  half the server refused.

### Using it

- Three keyboard layouts, every key changeable, and a dialog on the first start
  that works out which of the Mac's function key obstacles is in your way by
  asking you to press a key.
- Filter a list while typing; search the subfolders when asked, with the cost
  shown afterwards.
- Raw FTP commands under the log.
- Light, dark and the system's choice; five accent colours; English and German.

### Not there yet

- No FXP: transfers between two servers still travel through this machine.
- No directory comparison or mirroring.
- No remote editing.
- No scheduled jobs.
- The container build with its web interface is still to come.
- macOS is the only one used in anger. On Linux the Debian package has been
  installed into a clean Debian 12 and checked: its dependencies are right,
  nothing is missing, and the program, the launcher entry and the icons land
  where they should — but nobody has yet seen the window open on a real Linux
  machine. Windows is built by CI and otherwise untested.
