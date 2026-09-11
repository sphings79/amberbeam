# Changelog

[Deutsch](CHANGELOG.de.md)

Every release until 1.0 is a pre-release, and that is a statement about the
state of the program rather than a technicality: it does what it says, it is
tested, and it is not finished. The update notice inside AmberBeam reads
pre-releases for exactly that reason.

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
