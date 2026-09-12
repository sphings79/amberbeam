# Editing a file where it lies

[Deutsch](editing.de.md)

Right-click a file on a server and choose **Edit remotely**. A copy comes down
to this machine, something opens it, and every save goes straight back up.

That is the whole idea. Everything below exists because of the ways it can go
wrong quietly.

## What opens it

**Settings → Editing** holds one table: kinds of file, and what opens each.

| | |
|---|---|
| **AmberBeam** | The editor built in here. Line numbers, a search, and a save that means the server changed. |
| **The system's choice** | Whatever this computer opens that kind of file with. |
| **A program…** | One you name. On macOS an application bundle works as well as an executable. |

The first row that matches a file wins, so a row added on top beats the long
one underneath without having to be cut out of it. A row can name a kind
(`php`) or a whole file name for the ones that have none — `dockerfile` and
`makefile` are in there, and `.htaccess` counts as `htaccess`.

**What is not in the table is not offered for editing.** That is the same
decision said once instead of twice: a separate list of "what counts as text"
would eventually disagree with this one about the same file.

Underneath it all there is one more test that cannot be switched off: a file
with a zero byte in its first few kilobytes is not text, whatever it is
called, and is refused. A PNG that somebody named `logo.php` is still a PNG.

## Saving

The built-in editor saves with **⌘S**. An external program saves however it
saves — AmberBeam looks at the copy once a second while it is open, and sends
up what it finds. There is no way to be told: a program opening a file does
not announce it, and neither does it closing one.

Whether a save went up is in the server log, along with everything else that
happened to that server.

## When somebody else has been in the file

What the file looked like when the copy was taken is kept. Before writing
back, AmberBeam looks again — and if it is no longer the same file, **nothing
is written** and you are asked.

Size always; the modification time only when both looks offered one. Plenty of
FTP servers answer `MDTM` with nothing useful, and a question nobody can
answer is worse than no question.

Either way what you typed is safe in the copy first, so the work is never the
price of answering a question about it.

## Encodings and line endings

Read off the bytes, and written back exactly as found:

- **Not valid UTF-8** is read as Latin-1 and written back as Latin-1. Reading
  such a file as UTF-8 and saving it rewrites every umlaut in it, and nobody
  notices until much later.
- **A byte order mark** that was there stays there.
- **CR LF line endings** stay CR LF. An editor that quietly converts them
  turns every line of the next diff into a change.

Type a character the file's own encoding has no room for — an emoji into a
Latin-1 file — and the save is refused by name. A question mark where somebody
typed a word is the kind of help nobody wants.

## The copies

They live in a directory of their own under this system's temporary one, one
per file, each keeping the name it had.

Closing the built-in editor ends that edit and the copy goes: closing the
window you were typing in is the one way this program can know you have
finished. For a file open in another program there is no such moment, so the
watching runs until AmberBeam is closed — and then it asks whether the copies
should go.

Copies left behind by a run that ended badly are thrown away the next time the
program starts. The register of what is open lives in memory, so anything
still in that directory belongs to nobody.

**They are not encrypted.** A copy is a file from your server sitting on your
disk for as long as it is open, which is the same as any other download — but
worth knowing rather than assuming otherwise.

## Limits

- Files over 20 MB are not offered. The text travels through one command as a
  single string, and a 400 MB log wants a different tool.
- Asking to edit the same file twice gives back the same copy. Two copies of
  one file, each unaware of the other, is a race with your afternoon as the
  prize.
- **In the container build, only the built-in editor.** The service runs on
  the far machine, and a program started over there would not appear in front
  of you. The file opens in the editor here instead, and the log says why. See
  [Running it as a container](container.md).

See also: [Where the secrets are](security.md).
