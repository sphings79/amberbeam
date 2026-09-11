# Bringing your servers with you

[Deutsch](importing.de.md)

Nobody types thirty servers in again. AmberBeam reads the lists of six other
programs — open the server list (**⌘S**) and press the import button.

It looks in the usual places by itself and shows what it found. A file that
lives somewhere else can be chosen, or simply dragged onto the window.

## What it reads

| Program | File | Passwords |
|---|---|---|
| OpenSSH | `~/.ssh/config` | none to read — there are none in it |
| FileZilla | `sitemanager.xml` | yes, unless a master password is set |
| WinSCP | `WinSCP.ini` | yes, unless a master password is set |
| Total Commander | `wcx_ftp.ini` | yes |
| An older Windows client | `Sites.dat` | as well as they can be read |
| The same, exported | `.ftp` file | yes — they are in the clear in that file |

Folders come along. A tree in FileZilla or WinSCP arrives as a tree here.

## About those passwords

In most of these files the passwords are **obscured, not encrypted**. The
methods have been published for years; anyone with the file can read them.
That is exactly why it is worth taking them across: once AmberBeam has them
they go into this computer's credential store and out of every file.

You are asked before that happens, and told what it means. Nothing is written
until you have seen the list and ticked what you want.

Where a password cannot be read — a master password in the source, or a file
from a version whose method differs — the entry arrives **without** one rather
than with a wrong one, and the import says so beforehand. A password that
silently fails to log in is worse than one that was never claimed.

If you imported from a `.ftp` export, delete that file afterwards. It holds
every one of your passwords in plain text.

## Taking them out again

The same window exports: plain JSON without passwords, or sealed under a
passphrase with them. There is no third option. A file that carries passwords
and is merely obscured is precisely what the importers above spend their time
undoing.
