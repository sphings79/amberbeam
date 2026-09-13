# Comparing two sides, and watching one

[Deutsch](comparing.de.md)

Two different jobs that answer the same question — *what is not the same over
there* — and they are deliberately not the same feature.

**Comparing** is something you press. It reads both sides, says what differs,
and hands you the list.

**Watching** is something you leave running. It notices what changes on this
machine and sends it up by itself.

## Comparing

The **Compare** button reads the two panes as they stand. Before anything
happens it asks three things.

**Which way round.** Files travel from the left of the dialog to the right,
and the button between them swaps it. Nothing is decided by which pane you
happened to click last.

**Whether to go all the way down.** Off, it compares the two directories that
are open. On, it walks both trees to the bottom — which is one listing per
directory on each side, and on a server every listing is a round trip. A
project with a `node_modules` in it is thousands of them. That is why it is a
question and not an assumption.

**What counts as the same.**

| | |
|---|---|
| **Size** | Cheapest, and blind to any change that keeps the length. |
| **Size and time** | The usual answer, and the one it starts on. |
| **Size and contents** | Reads both files end to end. Certain, and over a server it costs what transferring them would. |

Two timestamps within **two seconds** of each other count as one moment. FTP
reports times to the second at best, plenty of servers round to the minute,
and a file that travels arrives a moment after it was read — without that
window every file you ever copied would read as changed.

A side that reports **no time at all** cannot disagree about one, so that
counts as the same. Otherwise every file on a server whose listing carries no
dates would be queued, every single time. If that is your server, use
checksums; they ignore the clocks entirely, which is the point of them.

### What comes back

Every name from both sides, with what was found about it:

| | |
|---|---|
| **only here** | On the side it would travel from. Ticked. |
| **differs** | On both, and not the same. Ticked. |
| **only there** | On the far side alone. Shown, and left alone — unless deletions are being carried across, see below. |
| **same** | Shown, stepped back, and not tickable. |

Rows that are the same are shown on purpose. A list of nothing but differences
cannot be checked against what is actually there.

Pressing the button puts what is ticked into the queue **held** — a comparison
can turn out to mean four hundred files, and setting that going the instant
somebody presses a button is not a decision they made. Start it when you have
looked at it.

Ticking a **directory** carries everything below it; its own rows are not
queued a second time.

If the walk stops at five thousand directories it says so, in as many words.
Everything missing from a list that is quietly incomplete looks exactly like
agreement, which is the dangerous kind of wrong.

## Watching

The **Watch** button watches the local pane and sends what changes in it to
the other side. A strip appears above the panes while it runs, saying which
directory, where it is going and how many files have gone, with one press to
stop it. Something that uploads files whenever they change must not be
invisible.

**One direction, and only one is possible.** The operating system tells us the
moment a local file is written. No server can say anything of the kind —
neither FTP nor SFTP has any way to announce a change — so watching the far
side would mean asking it over and over, which is a standing load on somebody
else's machine rather than a background service. The button says so rather
than failing when pressed.

**Nothing is asked about overwriting.** Nobody is looking at the window while
this runs, and a question would stop the queue waiting for an answer that is
not coming. The local file is the one the watch exists to carry across.

Events arrive in bursts — an editor writing a file produces a create, a write
and a rename inside a few milliseconds — so they are collected and acted on
once the noise stops.

A watch lives as long as the program. In the container it lives as long as the
service, which means it outlives the browser tab that started it; the strip
asks what is running rather than only knowing what it started itself.

## What is never looked at

**Settings → Editing** is about editing. This is a different list: patterns on
the **server entry**, under *Never look at*. `*` stands for anything, case does
not matter, and a name matches at every level — a rule naming `node_modules`
excludes everything inside it.

It belongs to the entry because what counts as noise is different on every
server. The comparison window fills itself from it and lets you change it for
one run without changing the entry.

## The two switches

**Settings → Comparing and watching**:

**Show what was found before transferring** — on. Off, everything that differs
goes straight into the queue, still held.

**Carry deletions across** — off, and worth leaving off. With it on, a file
that goes here goes there too: the watch removes it, and entries that exist
only on the far side become tickable in the comparison, where the button
counts them apart from the transfers. A checkout, a build that cleans up after
itself or a stray move would otherwise take files off the server, and you
would find out from the server.

A watch reads that switch once, when it starts. One that changed under a
running watch would leave nobody able to say what it had done.

Deletions do not go through the queue — the queue carries a file from one
place to another, and taking one away is not that. They happen first, so a
name freed on the far side can be filled by a transfer in the same batch. On a
server that has a wastebasket set, they go into it rather than away.

That last part asks the server to rename a file, and plenty of FTP servers
refuse to rename at all — the project's own test server among them. When that
happens nothing is deleted, the wastebasket is switched off for that entry, and
a watch says how many changes it could not carry out. The file is still up
there, which is the right way for this to fail.

See also: [Editing a file where it lies](editing.md).
