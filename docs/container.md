# Running it as a container

[Deutsch](container.de.md)

The same program without a window: it runs on a machine and you reach it in a
browser. Useful when the transfers should keep going after you close the
laptop, or when the files are somewhere that is awake more reliably than your
desk is.

It is the same core, the same commands and the same interface. Only the way
the window talks to the core changes — over HTTP instead of through the
desktop shell — and nothing in the interface knows the difference.

## Starting it

```yaml
services:
  amberbeam:
    image: ghcr.io/sphings79/amberbeam:latest
    container_name: amberbeam
    restart: unless-stopped
    ports:
      - "2122:2122"
    volumes:
      - ./data:/data
      - ./config:/config
    environment:
      AMBERBEAM_PASSWORD: "pick something"
      AMBERBEAM_SECRET_PASSPHRASE: "pick something else"
```

Then open `http://the-machine:2122` and sign in with that password.

## What it is told

| | |
|---|---|
| `AMBERBEAM_PASSWORD` | The one password. **Without it nothing is let in at all.** |
| `AMBERBEAM_SECRET_PASSPHRASE` | What saved passwords are encrypted with. Without it none are saved, and the program says so. |
| `AMBERBEAM_BEHIND_TLS` | Set when a reverse proxy in front is terminating TLS. |
| `AMBERBEAM_ADDRESS` | Where to listen. `0.0.0.0:2122`. |
| `AMBERBEAM_CONFIG` | Where the server list and settings live. `/config`. |

Both secrets can come from a file instead — `AMBERBEAM_PASSWORD_FILE` and
`AMBERBEAM_SECRET_PASSPHRASE_FILE` — which is what you want if the compose
file lives in a repository. The file wins over the variable: somebody who
mounted a secret meant it.

## The two volumes

`/data` is the local side. It is where the left-hand pane starts, and the
files you transfer to and from live there.

`/config` holds the server list, the settings, and the encrypted password
file. Back this one up; it is the part you would miss.

## Getting files in and out

Drag a file from your own computer onto the left-hand pane and it is uploaded
into whatever directory that pane is showing. Right-click a file there and
**Download** sends it back the other way.

Both only work on `/data` — this machine's own files. A browser cannot put a
file straight onto an FTP server, and it should not pretend to: getting one
there is two steps, and the queue is the second. Dropping onto a server pane
says so rather than failing halfway.

## Editing a file where it lies

Right-click a file on the server pane and **Edit remotely**: a copy comes down,
opens in AmberBeam's own editor over the page, and every save goes straight
back up.

Only that editor. The settings can point a kind of file at another program,
and here that setting cannot be carried out — the service runs on this
machine, and a program started on it would not appear in front of whoever
asked for it. The file opens in the editor here instead, and the log says so
rather than leaving somebody to wonder why their editor never came up.

Which files may be edited at all is the table under **Settings → Editing**,
and it is the same table the desktop program reads when it drives this
service.

## What it does not do

**It does not speak TLS itself.** Put a reverse proxy in front of it, which is
almost certainly already running on that machine. A proxy renews certificates,
which a container cannot, and it is one place to do it rather than one per
program. Set `AMBERBEAM_BEHIND_TLS` so the session cookie is marked
accordingly.

**`/data` is a starting point, not a fence.** Nothing stops somebody who is
signed in from walking up out of it into the rest of the container. There is
not much up there — the image holds the program and little else, and it does
not run as root — but it is worth knowing rather than assuming otherwise.

**One password, not accounts.** This is a program you run for yourself. A user
table would mean a sign-up screen, a password reset and a migration, all for
one person who already owns the machine.

**Everybody shares one session.** Two browsers see the same queue, the same
connections and the same server list, because there is one program running and
they are both looking at it. That is the point, not a limitation: it is how
you start something on one machine and watch it from another.

## The password file

Saved passwords go into `/config/secrets.sealed`, encrypted under the
passphrase with PBKDF2-HMAC-SHA256 at 600,000 rounds and ChaCha20-Poly1305 —
the same sealing an export with passwords uses.

**A wrong passphrase stops the program.** It does not start with an empty
store and carry on, because the first password saved after that would
overwrite everything already in the file. A typo must not destroy what it
failed to read.

Lose the passphrase and the saved passwords are gone. There is no way back
into that file, and that is what it being encrypted means.

## Reaching it from the desktop program

The desktop program can drive a running service instead of its own core: same
window, same keys, but the transfers happen over there. Under **Servers** you
choose between this machine and a service, give it the address and the
password, and the window switches over.

See also: [Where the secrets are](security.md).
