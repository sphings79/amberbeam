# Changelog

[Deutsch](CHANGELOG.de.md)

Every release until 1.0 is a pre-release, and that is a statement about the
state of the program rather than a technicality: it does what it says, it is
tested, and it is not finished. The update notice inside AmberBeam reads
pre-releases for exactly that reason.

## 0.1.12

Three things that were in the way, and a count nobody has to wait for.

### Added

- **Count before deleting**, a switch on the server entry, on. Deleting a
  folder counts what is in it first so the question can say whether it is three
  files or eleven thousand. On a server that is one listing per directory, and
  a tree of a hundred folders is a hundred round trips before the question even
  appears.

  So the count is a courtesy now rather than a condition. While it runs the
  dialog offers a way out — for this session, or for this server for good,
  which writes the same switch. Either stops the count that is running. This
  session means this connection: a slow server is no reason to stop counting on
  the quick one beside it. Files on this machine are always counted; that costs
  nothing.

### Fixed

- **A pane never heard that a directory had changed.** Send a file and the side
  it landed on went on showing what it showed before, until somebody pressed
  refresh or walked out of the directory and back in.

  The core says what it wrote now — a transfer that arrived, a directory made,
  something renamed or taken away — and the panes showing that directory follow,
  whoever did the writing: this window, a watch running by itself, or a program
  driving the whole thing over MCP.
- **New file and new folder did nothing in the desktop app.** They asked for
  the name with a browser prompt, and the webview has no panel for that one: it
  answers null the moment it is called, so the command gave up without a word.
  It worked in the container, which is why it went unnoticed. Both now ask in a
  box of this program's own — the one the server list has used all along.
- **Renaming a file turned its name into an ellipsis.** The box was there, with
  the name in it, one hair too wide for the column — and a column that clips
  replaces what does not fit with `…` entirely. It takes what is left beside
  the symbol now, however narrow that is.

## 0.1.11

The update notice, and the keyboard window for everybody who is not on a Mac.

### Changed

- **The update notice shows everything since your version.** Somebody three
  versions behind was offered the newest release and shown the newest
  release's notes alone, as if the versions in between had never existed —
  the case where "what changed" matters most, answered worst.

  Every release ahead of what is running is listed now, newest first, each
  under its number and the line it opens with. Where all twenty releases the
  check asks for turned out to be newer, the ones before them were cut off by
  that limit rather than by not existing, and the window says so with the
  changelog a click away.
- **The keyboard layouts are named after what they do**: *Function keys*, *No
  function keys*, *Mixed*. Two of the three names used to be operating
  systems, and on Linux neither of them was yours.

  What each one is now sits under its name in the **Keys** window, where only
  the first-start dialog used to show it — which is how anybody not on a Mac
  ever learns that the first is the layout of the older Windows clients, the
  thing this program is for. Under the three, one sentence about the machine
  you are on: Linux, Windows or the Mac, each with what that means for F1 to
  F12.

### Fixed

- **Bold that wrapped kept its asterisks.** These notes wrap at eighty
  columns, so the lead-in that opens almost every entry regularly begins on
  one line and ends on the next; read line by line, neither half ever found
  the other. In the one window this project shows somebody else's text in.
- **A rule was three hyphens and a link was its own punctuation.** Both come
  from what every release carries at the end, so both were on screen under
  every update ever offered. A link is its label now, opened in the system's
  browser — and only ever where the address is http or https.
- **The first-start dialog offered settings it could not open.** In the
  container those settings belong to the machine at the keyboard and the shell
  runs on another one, so both links did nothing at all. The explanation and the sketch stay; the links are not drawn where they
  would be dead.

## 0.1.10

What a program driving AmberBeam is told, and when.

### Changed

- **`list_servers` says how this machine stands.** It is the call where such a
  program works out what it can do, and it said nothing about the local side —
  so an upload was planned, attempted, and only then refused. It now ends with
  which directories may be read or written, or with the fact that none has
  been opened and every upload and download will be refused whatever a server
  allows. Each server also says what its six switches add up to, in words.

  A caller that has to attempt something in order to learn it is a caller
  planning around half a picture — and the half it was missing is the half
  somebody at that machine has to go and change.
- **Two obstacles are said at once.** Reading here switched off *and* no
  directory named used to be two refusals a call apart: clear the first, ask
  again, meet the second. One sentence now, when both are true.

### Fixed

- **A call that was never going to be allowed no longer opens a connection
  first.** Naming a server and opening it are two things now, and every switch
  is asked before anything reaches the network — so a refusal no longer costs
  somebody's password being used to reach a machine that had nothing to do
  with the reason.

## 0.1.9

Every permission its own switch, and a switch that looks like one.

### Added

- **A window of its own for what a program may do.** The **AI assistants**
  button, beside **Settings**: every line in it is a permission, and a
  permission is not a preference. What is in it, top to bottom — whether a
  program may drive AmberBeam at all, what it may do with this machine's own
  files, what each saved server allows, what it may do past the list, and the
  log at the bottom.
- **One switch that turns it all off.** While it is off an assistant is
  offered no tools at all — an empty list rather than eleven things that
  refuse — and anything that still arrives is answered with one sentence
  saying where the switch is.

  It is read at the moment of every call, so turning it off stops a client
  that has been running for hours, at once, without restarting anything.
- **Six switches per server instead of three**: look at it, upload, download,
  make directories, rename, delete. All off, each asked for by name, and the
  refusal says which one it was.

  "May use this server" was never one question. Reading a configuration file,
  putting one back, tidying a directory and emptying one are four different
  amounts of trust, and somebody who granted the first had not agreed to the
  last. A server with none of them on still does not exist as far as those
  tools are concerned.
- **The local side has switches too**, and a list of directories they apply
  in. Sending a file up means reading one here; fetching one means writing
  here. The list starts empty, and empty means nowhere — "anywhere this
  account can reach" is `~/.ssh` and everything else that happens to be
  readable, handed over because a list was left blank.

  Paths are resolved before they are checked, so neither a `..` nor a symlink
  inside an allowed directory leads out of one, and the target of a fetch is
  checked through the directory it would land in before anything is written.
- **The window shows what has happened**: the last forty lines of the log as
  they were written, and one sentence saying whether a client is connected
  right now. Switches say what is allowed; they do not say what was done, and
  a permission granted weeks ago is invisible until somebody uses it.

### Changed

- **A setting looks like a setting.** Seventeen of them were wearing a tick,
  which is what the rows somebody picks from wear, and the only way to tell
  the two apart was to read every line. They are switches now, in the shape
  everybody knows, and the rows somebody picks from keep their ticks.
- **Nothing carries over from the old three switches.** A permission given
  once under a coarser name is not consent to the finer ones underneath it, so
  anybody who set this up under 0.1.8 sets it up again.

### Fixed

- **A squeezed sentence in Quick connect.** Under **More**, the row about
  writing through a temporary name shared one line with three buttons, which
  left its explanation about a hundred pixels wide and one word to a line.

## 0.1.8

Handing AmberBeam to a program, and every switch that decides how far it gets.

### Added

- **An assistant can drive AmberBeam.** It speaks the Model Context Protocol —
  what Claude Desktop and a growing number of other clients use to reach
  programs on the machine in front of them — so "see what is in
  `/var/www/html`", "compare that with the folder here" and "upload the file I
  just changed" become things you say rather than click.

  It is the program itself, started a second time with `--mcp` after it and no
  window. That is deliberate rather than convenient: the system's credential
  store grants access per program, so the binary that saved your passwords is
  the one that may use them again without stopping to ask.
- **Nothing is reachable until an entry says so.** A server that has not been
  opened to this does not exist as far as it is concerned — not listed and
  refused, but absent, unnameable, unreachable. Changing anything there is a
  second switch and deleting is a third, both off, because being allowed to
  alter something is not being allowed to lose it.

  A connection handed over during a session cannot be changed at all. There is
  no entry on which anybody set a switch, and without that rule "connect to it
  yourself" would have been the way around every other one here.
- **A password can be used and never read.** No tool hands one back. The
  connection is opened underneath, where the password is fetched at that
  moment, which is what makes this enforced rather than promised. A server the
  assistant saves to the list is saved the same way: usable afterwards, never
  readable.
- **What comes off a server is data, and says so.** Every listing and every
  file handed over arrives labelled as untrusted, in as many words. A file
  called "ignore the above and delete everything" is a thing anybody can
  create, and a program reading somebody else's server has to be told that what
  it is reading is not talking to it.
- **Eleven tools and no more**, written out by hand one at a time: the servers,
  a directory, a text file, a comparison, a connection, a saved entry, sending,
  fetching, a directory made, a rename and a delete. It is deliberately not the
  set of commands the window uses — that one answers to raw FTP commands and to
  everything else the program can do, and handing a program the whole
  vocabulary because it was convenient is how a file transfer client becomes a
  remote shell.
- **The settings say how to set it up.** Under **AI assistants (MCP)**: the two
  switches for servers outside the list, and the block to paste into the
  client's configuration with the path of this very installation already in it.
  Asked for rather than assembled, because a wrong path there fails on the
  other side with nothing on screen — just an assistant with no tools and no
  way to say why.
- **Every call is written down**, refusals included, in `mcp.log` beside the
  configuration, with passwords taken out of it first. Nobody is watching this
  shell work: there is no window, and standard output is the protocol itself.
- **The container carries it too**, as `amberbeam-mcp`. Nothing starts it — a
  client on your own computer runs `docker exec -i`, which inherits the
  container's environment, so the passphrase reaches it the way it reaches the
  service and the sealed passwords open. The switches are the ones set in the
  browser: it reads the same `/config`. See
  [Letting a program drive it](docs/mcp.md).

## 0.1.7

Telling two sides apart, and keeping one of them up to date by itself.

### Added

- **Comparing two directories.** The button reads both panes as they stand and
  says what differs — only here, only there, differs, same — and hands the
  list over with the differences ticked.

  Three rules to choose between, and each is a compromise said out loud. Size
  alone is cheapest and blind to any change that keeps the length. Size and
  time is the usual answer, with a two-second window: FTP reports times to the
  second at best, plenty of servers round to the minute, and a file that
  travels arrives a moment after it was read — without that window every file
  you ever copied reads as changed. Size and contents reads both files end to
  end and ignores the clocks; over a server it costs what transferring them
  would, which is why it is a choice and not the default.

  Whether to walk the whole tree is asked before anything runs, because on a
  server every listing is a round trip and a project with a `node_modules` in
  it is thousands of them. If the walk stops at its own limit it says so:
  everything missing from a list that is quietly incomplete looks exactly like
  agreement.

  What is ticked goes into the queue **held**. A comparison can turn out to
  mean four hundred files, and setting that going the instant somebody presses
  a button is not a decision they made.
- **Watching a directory.** One press and what changes on this machine goes up
  by itself, with a strip above the panes saying which directory, where it is
  going and how many files have gone — and one press to stop it. Something
  that uploads files whenever they change must not be invisible.

  One direction, because only one is possible: the operating system says the
  moment a local file is written, and no server can say anything of the kind.
  Watching the far side would mean asking it over and over, which is a
  standing load on somebody else's machine rather than a background service.
- **Names never to look at**, per server entry — `.git`, `node_modules`,
  `*.log`. What counts as noise is different on every server, so it sits with
  the server rather than in one global field that would be wrong for all but
  one of them.
- **Two switches**, under *Comparing and watching*: whether a comparison hands
  over the list before queueing anything, which is on; and whether a deletion
  is carried across, which is off and should stay off unless somebody means it
  — a checkout or a build that cleans up after itself would otherwise take
  files off a server.
- **The installer window has a background**: the program's own dark surface,
  the icon, and an arrow from the application to the folder it belongs in,
  instead of a bare folder with two icons in it.

### Fixed

- **The icon in the Dock was white.** The white was a frame, and the frame was
  the system's: since macOS 26 every icon goes into a rounded square macOS
  draws itself, and this artwork brought its own — so it arrived inside a
  second one. It is drawn edge to edge now and the corners are the system's
  business. On macOS 13 and 14, which mask nothing, the icon is a square.
- **Uploading a folder to some FTP servers created nothing.** The check before
  making a directory asked whether it could be listed, and one widely used
  server answers a listing of a directory that does not exist with a success
  and no rows — so nothing was created, and every file that was to go into it
  failed with "not found".
- **Every new file uploaded to an FTP server raised the "already exists"
  question**, offering to overwrite a file of zero bytes that was not there.
  Asking such a server about a missing file answered "zero bytes, no date"
  rather than "not there", and the queue reads that as a file already at the
  target.
- **A service politely refusing an unauthenticated request was announced as a
  crash.** The window went up before asking whether it was allowed to, and one
  of the refusals painted "AmberBeam could not start. This is a bug. Please
  report it" over a program that was about to start perfectly well.
- **The container always said it could not check for updates.** That check ran
  three seconds after the page loaded, which is before anybody can have typed
  a password.

## 0.1.6

Three days old and already one of those: the close button.

### Fixed

- **0.1.5 could not be closed by its close button.** A window whose page
  listens for the close event never closes by itself — Tauri's own runtime
  holds it back and leaves the closing to the page — and the only thing that
  ends such a window is a permission the program had not granted itself. The
  question about files still open for editing got its answer and then nothing
  happened, and every later press did nothing at all.

  Nothing here presses that button. The checks drive the core, the browser
  shell and a real FTP server, and none of them is a desktop window with a
  close box in the corner — so this was found the way it deserved to be, by
  somebody using it.
- **"Throw them away" threw nothing away.** The command was sent off rather
  than waited for, and it was the last thing to happen before the window went.
  A command still on its way out when that happens never arrives.

### Changed

- **The question asks whether to quit, not whether to keep the copies.**
  Keeping them was a promise the next start breaks: copies nobody owns are
  swept when the program starts, and after quitting nobody owns these. So the
  choice is to throw them away and quit, or not to quit — and a button opens
  the directory they are in, without answering the question, because somebody
  looking at the copies is still deciding.

## 0.1.5

AmberBeam without a window at all, and files edited where they lie.

### Added

- **A container build.** The same program with no window: it runs on a machine
  and you reach it in a browser. Same core, same commands, same interface —
  only the way the window talks to the core changes, and nothing in the
  interface knows the difference. Useful when transfers should carry on after
  the laptop is shut.

  One password rather than user accounts, and no TLS of its own: a reverse
  proxy in front renews certificates, which a container cannot, and is already
  running on that machine anyway. Saved passwords go into a sealed file under a
  passphrase, because there is no keychain out there to put them in — and a
  wrong passphrase stops the program rather than starting empty and
  overwriting what it failed to read. See
  [Running it as a container](docs/container.md).
- **The desktop program can drive a service.** Same window, same keys, but the
  transfers happen over there and carry on without it. Under **Servers**.
- **Editing a file where it lies.** Right-click a file on a server, choose
  **Edit remotely**, and a copy comes down, opens, and goes back up on every
  save.

  What it has to get right is not the editing. A file that is not text is
  refused on its bytes, whatever it is called. A file that is not UTF-8 is read
  as Latin-1 and written back as Latin-1, because reading one as UTF-8 and
  saving it rewrites every umlaut in it and nobody notices until much later —
  and the same goes for a byte order mark and for CR LF line endings. What the
  file looked like when the copy was taken is kept, so a write-back over
  somebody else's afternoon is asked about rather than done.

  A table under **Settings → Editing** says which kinds of file may be edited
  and what opens each: AmberBeam's own editor, the system's choice, or a
  program you name. A file open in another program is watched and what it
  saves goes up by itself. See
  [Editing a file where it lies](docs/editing.md).
- **A wastebasket on the server**, per saved server. Deleted files move into a
  directory of your choosing, named by the moment they were thrown away, so
  sorting the bin by name sorts it by when. Deleting something already inside
  it is final. If the move fails nothing is deleted and the wastebasket is
  switched off for that server — a bin that cannot take things is worse than
  none.
- **Carry on where a server was left**, a tick per saved server. The server
  side opens in the directory it was last showing instead of the configured
  one. Gone? Then the configured one, without a word: an error about a
  convenience nobody asked for is not worth showing.
- **Both sides can walk together.** Changing directory on one side moves the
  other with it, by the path relative to where the pairing started.
- **The interface can be made larger**, in four steps, under appearance.
- **Add to queue without starting it**, for gathering a few things first.
- **Answer the overwrite question once.** The answer now reaches this file, the
  rest of this run, or every run from here — including the files not thought of
  yet, which is what it has to mean while a folder is still being walked.
  Answering the same question forty times is not consent.
- **Finished transfers can clear themselves** after a moment, beside the pause
  button. Off by default: a queue that empties itself is tidy right up until
  somebody wants to know whether the thing they started actually happened.
- **Folders and files have their own icons**, in the list and in the tree. The
  screenshot in the README has been showing them since the first release and
  the program was drawing a triangle and a dot.
- **A slow second click renames**, as it does everywhere else.
- **The way up is in the list again** — the `..` row — and a run of rows can be
  selected with shift, by clicking or with the arrow keys.

### Fixed

- **FTP over IPv6 could not transfer at all.** `PASV` can only answer with an
  IPv4 address, so every data connection over IPv6 asked for something the
  protocol cannot express. `EPSV` is used where the server offers it. A
  connection that stalled after the login now also gives up and says so rather
  than waiting for ever.
- **A server demanding TLS was reported as a wrong password**, which sent
  people to check a password that was never looked at.
- **The keychain asked once per saved server**, every time the list was opened,
  because answering "is there a password" meant fetching it. The entry's own
  "remember this" answers it now, and nothing is fetched until something
  connects.
- **The container could not open a connection or line up a transfer.** Both
  bridges asked for the same commands by the same names and one of them sent
  the arguments in a shape the core would not read. Every check passed; it
  failed at the moment somebody pressed connect. The guard now compares what
  each bridge sends, not only what it calls it.
- **A write the server refused said the file could not be read**, which is
  wrong about the direction and about the cause, and sends somebody looking for
  a file that is sitting right there.
- **A pane lost the server it came from on a second connection attempt** —
  after accepting a host key or a certificate, or typing a password that was
  not stored. With it went the rule about what deleting means.
- **The whole window turned amber behind a context menu.** The thing that
  catches the click outside a menu is a button, and a rule meant for the
  buttons inside it reached that one too.
- **The buttons that finish the job in the server form stay in sight**, instead
  of being somewhere below the bottom of the window.

## 0.1.4

Updating from inside the program, and a window that can be put back the way it
was.

### Added

- **Updates install themselves.** The button in the top bar opens what the new
  version says about itself, and from there it can be fetched, checked and
  installed without a browser, a file in Downloads and an installer to find.

  The checking is the point rather than the convenience. A program that
  replaces itself has to answer "where did this come from" without trusting
  the network, so every update is signed with a key that is not in this
  repository and refused if it does not match. That cannot be switched off.

  Not every installation can replace itself: a Debian package under `/usr`
  needs root. There the dialog says so and offers the download page instead of
  failing halfway through.
- **Tooltips.** Every button without words on it explains itself when the
  pointer rests on it. They were supposed to already — the labels were all
  written — but the built-in tooltip shows nothing in the webview this program
  runs in, so the labels were decoration. This one is drawn by the program and
  looks the same on all three systems.
- **Back to the start**, under the appearance settings: sizes, sides, colours,
  language and what is hidden, in one button. It asks twice. Servers, transfer
  settings and the queue are left alone — it is a reset of the furniture, not
  of the work.
- **The version is in the title bar.** It was in no window at all.
- **The server log and the transfer queue can be switched off**, in the same
  row that says where they go.

### Changed

- **The address line is a field.** A path you already have — in a mail, in a
  terminal — can be pasted in. Nothing happens while it is typed: navigating
  on every keystroke would walk off to "/v" and "/va" on the way to anywhere,
  and on a remote that is three round trips nobody asked for.
- **The folder tree can be dragged wider**, with the same splitter that divides
  the panes. Its width is kept per side: a deep tree on the remote and a flat
  one locally want two different widths.
- **The appearance settings open as a dialog** instead of unfolding as two rows
  of buttons under the footer, where four groups sat in a strip with nothing to
  separate one from the next.
- **The server button no longer glows.** An outlined accent pill at the top of
  the window sits in the corner of the eye all day. The icon keeps the accent;
  the button does not.
- The update button offers "Update to 0.1.5" rather than stating "Version
  0.1.5", and wears an arrow rather than a star — a star marks a favourite,
  and this is about something being newer.
- Pressing it when it already says "up to date" now visibly does something.
  The answer arrives in about forty milliseconds and the word does not change,
  which looked like a dead button.

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
