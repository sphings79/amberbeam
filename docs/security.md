# Where the secrets are

[Deutsch](security.de.md)

Short version: in the system's own credential store, and nowhere else.

## Passwords

A server entry in AmberBeam is a readable JSON file under
`~/Library/Application Support/AmberBeam/sites/` — one per server, in folders
that are real folders. You can read them, edit them, put them in a backup or
in version control.

That is only acceptable because **they hold no password**. Each entry refers
to its secrets by an identifier, and the password itself lives in the
Keychain on macOS, the Credential Manager on Windows, or the Secret Service on
Linux. Remembering a password at all is a choice you make per entry, and
turning it off deletes what was stored.

A password never reaches the program's window. The connection is opened
underneath, where the password is fetched at that moment — so there is no
point at which it sits in a webview waiting to be read out of one.

## Server keys and certificates

**SSH host keys** are checked against `~/.ssh/known_hosts` — the same file the
terminal uses. A server nobody has seen before asks once and shows its
fingerprint. A server whose key *changed* is refused outright, because that is
what an interception looks like.

**TLS certificates** are checked against the certificates this computer
already trusts, including any your employer installed. One that is not
vouched for is refused, and the reason is named rather than lumped into
"certificate error": self-signed, expired, not yet valid, wrong name, revoked,
or a broken signature. Accepting applies to that **one certificate on that one
host and port** — a different certificate on the same server asks again — and
the pane carries a mark for as long as the exception holds.

Exceptions live in `accepted-certificates.json` beside the rest, in the open,
so you can see and remove them without this program's help.

## What is never encrypted, and says so

Plain FTP sends your password and every file in the clear. AmberBeam will do
it, because some servers offer nothing else, and marks the connection in red
for as long as it is open.

FTPS encrypts the control connection **and** the data. There is no half
measure: if a server refuses to encrypt the data channel, the connection fails
and says which of the two it refused. A data channel without `PROT P` is not
partly secure, it is in the clear.

## Exports

Without passwords an export is plain JSON. With them it is sealed under a
passphrase you choose — PBKDF2 and ChaCha20-Poly1305, the ordinary
construction — and the passphrase is stored nowhere. Lose it and the file is
lost with it. There is no third shape.

## What AmberBeam never does

No accounts, no telemetry, no servers of its own. Nothing about what you
connect to leaves your machine.
