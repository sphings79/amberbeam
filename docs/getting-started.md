# Getting started

[Deutsch](getting-started.de.md)

## Two panes, either of which can be anywhere

Both sides start on this computer. Connect one of them to a server and you
have the usual arrangement; connect both and you have two servers side by
side. Nothing in AmberBeam assumes one side is "the local one" — that is a
decision made before the first line of code, and it is why transfers between
two servers are a question of when rather than whether.

**Tab** or **F6** moves between the sides. The side with the brighter frame is
the one the keys are talking to.

## Connecting

The **+** button on a pane, or **F12**, opens the connect dialog. Pick the
protocol first — the port follows it, unless you typed one yourself:

| | Port | |
|---|---|---|
| **SFTP** | 22 | Over SSH. Use this when the server offers it. |
| **FTPS (explicit)** | 21 | Ordinary FTP that switches to TLS with `AUTH TLS`. |
| **FTPS (implicit)** | 990 | TLS from the first byte. Older, and rarer. |
| **FTP** | 21 | No encryption at all. Marked in red for as long as it is open. |

A server you want again belongs in the server list: **⌘S**, or the *Servers*
button. Entries there keep everything — where both sides should start, how
many transfers at once, a colour to tell them apart — and their passwords go
to this computer's credential store rather than into any file.

## Moving files

Select with the **space bar** or the mouse, then:

- **F5** on the toolbar's transfer button, or drag the selection to the other
  pane
- Drag files in from the Finder
- A folder goes with everything in it, and the structure is kept

Everything lands in the **queue** at the bottom. It survives a restart: close
the program mid-transfer and what was left is still there when you come back,
and a file that was half sent continues from where it stopped rather than
starting again.

## When something is already there

AmberBeam asks rather than deciding. Overwrite, skip, keep both, or compare
the sizes and dates first — and "do the same for the rest" is on the same
dialog, because a queue of two hundred files must not become two hundred
questions.

## When a transfer breaks

It picks up where it left off, but only when that is safe: the file at the
far end has to be the same size and age it was when the transfer stopped. If
it changed, AmberBeam starts again rather than stitching two versions
together. A file assembled from two different versions looks complete and is
not, which is the one failure this program is built to never produce.

Some FTP servers cannot continue a transfer at all. Those say so when they
connect, not when something breaks.
