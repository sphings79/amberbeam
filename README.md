<div align="center">

<img src="assets/icon.svg" width="96" height="96" alt="AmberBeam logo">

# AmberBeam — dual-pane FTP and SFTP client for macOS

**A file transfer client for the Mac in the tradition of FlashFXP.** Server log
on top, two file panes in the middle, transfer queue at the bottom — and the
keyboard in charge. Built for everyone who came to macOS from Windows and never
found a replacement for their dual-pane FTP client.

[![Licence: AGPL v3](https://img.shields.io/badge/licence-AGPL--3.0-e08b12?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%2013%2B%20%C2%B7%20Apple%20Silicon-2b3040?style=flat-square)](#build-it-yourself)
[![Built with Rust](https://img.shields.io/badge/core-Rust-b7410e?style=flat-square)](https://www.rust-lang.org/)
[![Built with Svelte](https://img.shields.io/badge/interface-Svelte%205-ff3e00?style=flat-square)](https://svelte.dev/)
[![CI](https://img.shields.io/github/actions/workflow/status/sphings79/amberbeam/ci.yml?branch=main&style=flat-square&label=CI)](https://github.com/sphings79/amberbeam/actions/workflows/ci.yml)
[![Stars](https://img.shields.io/github/stars/sphings79/amberbeam?style=flat-square&color=f0b429)](https://github.com/sphings79/amberbeam/stargazers)

[Deutsche Version](README.de.md) · [Status](#status) · [What it will be](#what-it-will-be) · [The three seams](#the-three-seams) · [Build it yourself](#build-it-yourself) · [Roadmap](#roadmap) · [Translating](#translating) · [Licence](#licence)

**By the same hand:** [AmberChest — IMAP mail backup in plain .eml files](https://github.com/sphings79/amberchest)

</div>

---

## Status

> **Milestone M0: the scaffold stands, and nothing transfers yet.**
> The toolchain is proven — the repository builds to a signed `.dmg` on Apple
> Silicon — and the three architectural seams that would be expensive to add
> later are in place. SFTP arrives with M1, transfers with M2. The
> [roadmap](#roadmap) says what is done and what is not, and this README will
> not claim otherwise.

## Why another FTP client for the Mac?

Transmit and ForkLift are good programs. They are also, through and through,
Mac programs. What people moving from Windows miss is not a feature list — it
is a set of **movements**: the function keys, switching panes with a keystroke,
the queue at the bottom, the raw server log at the top. That muscle memory has
nowhere to go on macOS.

AmberBeam is built for exactly that gap. Where Mac convention and FlashFXP habit
collide, **habit wins** — as long as the program does not end up fighting the
operating system. Where it would, you are asked on first run which way you want
it.

## Screenshots

<div align="center">

<img src="assets/screenshots/layout.svg" width="880" alt="AmberBeam window layout: server log on top, local files on the left, the server on the right, transfer queue at the bottom, shown in dark and light theme side by side">

<em><strong>The planned window</strong>, dark and light torn apart down the
middle. Drawn, not photographed: the panes do not list files yet — that is
milestone M1. Everything here is decided, nothing here is invented for the
picture.</em>

</div>

## What it will be

Version 1, as specified:

- **SFTP, FTP and FTPS**, explicit `AUTH TLS` by default, implicit on port 990
  supported, plain FTP possible but visibly marked
- **Two panes, each with a folder tree and a file list.** Either side can be
  local *or* a server — two servers side by side are explicitly allowed
- **Transfer queue** that survives a restart, with per-server concurrency
  (SFTP 8, FTP 4 by default, adjustable up to 64)
- **Resuming a single file that broke in mid-transfer** — not just the queue.
  Size and timestamp of the source are compared first, and if they changed you
  are asked instead of silently handed a file stitched from two versions
- **Site manager with import** from FileZilla, WinSCP, `~/.ssh/config`, Total
  Commander and FlashFXP — because nobody retypes thirty servers
- **PuTTY `.ppk` keys** read and converted, the one thing no Mac client does and
  every Windows switcher needs
- **Function keys as in FlashFXP**, freely remappable, with a first-run dialog
  for the macOS function key problem
- **German and English**, light, dark and system theme, five accent colours
- **No cloud, no accounts, no telemetry.** Passwords live in the macOS keychain,
  never in a configuration file

Later: directory synchronisation, comparing both sides, remote editing,
scheduled jobs, FXP, a container build with a web interface, S3 and WebDAV.

## The three seams

Three decisions had to be right before the first line of code, because adding
them afterwards means touching every call site:

| # | Seam | What it means |
|---|------|---------------|
| 08 | **Source and target are endpoints** | The transfer engine knows two arbitrary endpoints and never assumes one side is local. Server-to-server copying and, later, FXP are additional cases — not a rebuild of the foundation |
| 09 | **One door to the core** | The interface never calls Tauri. Everything goes through `src/lib/bridge`, which has two implementations: Tauri's channel, and HTTP plus WebSocket for the container build |
| 11 | **No text in the code** | Every string goes through `t()` and lives in a flat JSON language file. English is the fallback, so a half finished translation can never show a raw key |

Seams 2 and 3 are checked on every build by `scripts/check-seams.mjs` and
`scripts/check-lang.mjs`, because a rule nobody enforces holds only as long as
everybody remembers it.

## Build it yourself

You need Xcode command line tools, Rust and Node 22 or newer:

```sh
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
brew install node
```

Then:

```sh
git clone https://github.com/sphings79/amberbeam.git
cd amberbeam
npm install
npm run tauri dev     # development window with hot reload
npm run tauri build   # AmberBeam.app and a .dmg in target/release/bundle
```

The first `cargo build` downloads a lot and takes several minutes. That is
normal, not a hung process.

The `.dmg` is **ad-hoc signed, not notarised** — there is no paid Apple
membership behind this yet. On your own Mac that is enough; on someone else's,
Gatekeeper will complain and the app has to be opened from the context menu
once.

### Checks

```sh
npm run verify                    # language files, seams, types
cargo test --package amberbeam-core
cargo clippy --package amberbeam-core --all-targets -- -D warnings
```

The core builds and tests on Linux as well — deliberately, so nothing
platform-specific creeps into it and the container build of M7 stays possible.
CI enforces that on every push.

### Regenerating the pictures

```sh
python3 dev/make-screenshots.py
dev/render-png.py assets/social-preview.svg assets/social-preview.png 1280 640
dev/render-png.py assets/icon.svg assets/icon.png 1024 1024
npm run tauri icon assets/icon.png
```

## Roadmap

| | Milestone | State |
|---|---|---|
| **M0** | Scaffold: a window, a signed `.dmg`, the three seams, licence and README | **done** |
| **M1** | SFTP: connect with password and key, list directories, tree and list, two panes, focus switching | next |
| **M2** | Transfers: queue, concurrency, resuming, conflicts, progress. Usable from here on | |
| **M3** | FTP and FTPS: control and data channel, `MLSD` preferred, `LIST` with dialect detection as fallback | |
| **M4** | Site manager: keychain, import from FileZilla, WinSCP, OpenSSH and FlashFXP, export | |
| **M5** | Keyboard and settings: function keys, first-run dialog, remappable schemes, raw commands, server search | |
| **M6** | Polish: icon, signing and notarisation, help, first release | |
| **M7** | Container build: the same core behind an HTTP and WebSocket service, single user, Docker image for amd64 and arm64 | |

## Translating

Languages are flat JSON files under `src/lib/i18n/`. Copy `en.json`, translate
the values, keep the keys, and send a pull request — or, once M5 lands, drop the
file into `~/Library/Application Support/AmberBeam/lang/` and restart. No build
step, no toolchain.

`npm run check:lang` tells you what is missing or surplus.

## Licence

**AGPL-3.0-or-later.** The Affero clause is here for one reason: the container
build. Anyone who takes it, changes it and offers it as a paid service over a
network has to publish their changes. For the `.dmg` on your desk the difference
is nil — a desktop program offers nobody network interaction.

---

<div align="center">

**If you want to see this finished, a star helps.** It is how the next person
who misses FlashFXP finds it.

[![Buy me a coffee](https://img.shields.io/badge/Buy%20me%20a%20coffee-sphings-f0b429?style=flat-square&logo=buymeacoffee&logoColor=black)](https://buymeacoffee.com/sphings)

</div>
