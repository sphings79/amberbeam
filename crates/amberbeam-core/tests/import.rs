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
