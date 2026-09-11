//! The importers against real files rather than against samples this project
//! wrote itself.
//!
//! Every reader has unit tests with the format written out forwards, which
//! proves the reader matches what was published. What that cannot prove is that
//! the published format is right. These tests close that gap where a real file
//! happens to be on the machine, and skip themselves where it is not — the same
//! bargain as the SFTP and FTP tests.

use amberbeam_core::import::{self, Source};

#[test]
fn the_ssh_config_on_this_machine_reads_as_servers() {
    let Some(home) = std::env::var_os("HOME") else {
        eprintln!("skipping: no HOME");
        return;
    };
    let path = std::path::Path::new(&home).join(".ssh").join("config");
    if !path.is_file() {
        eprintln!("skipping: no {} on this machine", path.display());
        return;
    }

    let found = import::read(Source::SshConfig, &path).expect("read the config");
    eprintln!("{} holds {} hosts", path.display(), found.entries.len());

    for entry in &found.entries {
        assert!(!entry.name.is_empty(), "a nameless entry cannot be shown");
        assert!(
            !entry.name.contains('*') && !entry.name.contains('?'),
            "a pattern is a rule, not a server: {}",
            entry.name
        );
        assert!(!entry.host.is_empty(), "{} has no address", entry.name);
        assert!(entry.port > 0);
        // There are no passwords in an SSH configuration, and claiming one
        // would send somebody looking in their keychain for nothing.
        assert!(!entry.has_password);
    }
}

/// Whatever else is lying about on this machine, read by the right reader.
///
/// The point is not the contents but that discovery and the readers agree: a
/// file found as one kind must be readable as that kind.
#[test]
fn whatever_is_found_can_also_be_read() {
    let found = import::discover();
    if found.is_empty() {
        eprintln!("skipping: nothing to import on this machine");
        return;
    }

    for candidate in &found {
        let read = import::read(candidate.source, &candidate.path).unwrap_or_else(|error| {
            panic!("{} could not be read: {error:?}", candidate.path.display())
        });
        // Counts, not names: whatever is on this machine is the tester's own
        // business, and a test that prints it puts it in a build log.
        eprintln!(
            "{:?}: {} entries, {} with a password, {} warnings",
            candidate.source,
            read.entries.len(),
            read.entries
                .iter()
                .filter(|entry| entry.has_password)
                .count(),
            read.warnings.len()
        );
        for entry in &read.entries {
            assert!(!entry.name.is_empty());
            assert!(!entry.host.is_empty(), "{} has no address", entry.name);
        }
        for warning in &read.warnings {
            eprintln!("  warning: {warning}");
        }
    }
}

/// Every reader against a file on disk.
///
/// The unit tests parse samples written as Rust strings, which are UTF-8 by
/// definition. These are bytes: the export below is windows-1252 as that
/// program really writes it, and reading it as UTF-8 would turn Müller into
/// something else without anybody noticing. The servers are invented and the
/// passwords with them.
#[test]
fn the_sample_files_read_the_way_they_should() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("import");

    let read = |source: Source, name: &str| {
        import::read(source, &dir.join(name))
            .unwrap_or_else(|error| panic!("{name} could not be read: {error:?}"))
    };

    // OpenSSH: three hosts out of two blocks, and `Host *` is not one of them.
    let ssh = read(Source::SshConfig, "ssh-config");
    assert_eq!(ssh.entries.len(), 3);
    assert_eq!(ssh.entries[0].host, "web.example.org");
    assert_eq!(ssh.entries[0].port, 2222);
    let db = ssh.entries.iter().find(|e| e.name == "datenbank").unwrap();
    assert_eq!(db.user, "fallback", "what Host * left for it");

    // WinSCP: the folder out of the session name, the password out of the hex.
    let winscp = read(Source::WinScp, "WinSCP.ini");
    let web = winscp.entries.iter().find(|e| e.name == "Web").unwrap();
    assert_eq!(web.folder, "Kunden/Müller");
    assert_eq!(web.password.as_deref(), Some("tannenbaum"));
    assert_eq!(web.remote_path.as_deref(), Some("/var/www"));
    let old = winscp
        .entries
        .iter()
        .find(|e| e.name == "Alt und offen")
        .unwrap();
    assert_eq!(
        old.encryption,
        Some(amberbeam_core::ftp::Encryption::Explicit)
    );

    // Total Commander: address, port and directory out of one field.
    let tc = read(Source::WcxFtp, "wcx_ftp.ini");
    let mueller = tc
        .entries
        .iter()
        .find(|e| e.name == "Kunde Müller")
        .unwrap();
    assert_eq!(mueller.host, "ftp.example.org");
    assert_eq!(mueller.port, 2121);
    assert_eq!(mueller.remote_path.as_deref(), Some("/var/www"));
    assert_eq!(mueller.password.as_deref(), Some("tannenbaum"));
    assert!(tc.entries.iter().all(|e| e.name != "default"));

    // Sites.dat: one readable password, one that is not — said out loud.
    let dat = read(Source::SitesDat, "Sites.dat");
    let first = dat.entries.iter().find(|e| e.name == "Müller").unwrap();
    assert_eq!(first.folder, "Kunden");
    assert_eq!(first.password.as_deref(), Some("tannenbaum"));
    assert!(dat
        .warnings
        .contains(&"import.warning.some-passwords".to_string()));
    assert!(dat
        .entries
        .iter()
        .any(|e| e.name == "Unlesbar" && !e.has_password));

    // FileZilla: nested folders, Base64, and a directory name with a space in
    // its own notation.
    let fz = read(Source::FileZilla, "sitemanager.xml");
    let webserver = fz.entries.iter().find(|e| e.name == "Webserver").unwrap();
    assert_eq!(webserver.folder, "Kunden/Müller");
    assert_eq!(webserver.password.as_deref(), Some("tannenbaum"));
    assert_eq!(webserver.remote_path.as_deref(), Some("/var/www neue"));
    assert!(fz
        .entries
        .iter()
        .any(|e| e.name == "Offen" && e.folder.is_empty()));

    // The export: windows-1252 on disk. This is the one the unit tests cannot
    // reach, because a Rust string is UTF-8 whether one likes it or not.
    let export = read(Source::SitesXml, "sites-export.ftp");
    assert_eq!(export.entries.len(), 1);
    assert_eq!(
        export.entries[0].name, "Kunde Müller",
        "read as UTF-8 the umlaut would be gone"
    );
    assert_eq!(export.entries[0].folder, "Kunden/Müller");
    assert_eq!(export.entries[0].password.as_deref(), Some("tannenbaum"));
    assert!(export
        .warnings
        .contains(&"import.warning.cleartext".to_string()));
}
