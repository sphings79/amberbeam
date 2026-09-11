//! `~/.ssh/config`.
//!
//! The most useful of the five and the simplest: no passwords to unpick, and
//! anybody who has used `ssh` from a terminal already has one. What it gives is
//! a name, an address, a user, a port and a key file — which is every field an
//! SFTP entry needs.
//!
//! Read the way OpenSSH reads it in one respect that matters: the first value
//! for a keyword wins, and `Host *` at the bottom therefore fills in what the
//! specific blocks left out rather than overwriting them.

use std::path::Path;

use crate::config::AuthKind;
use crate::endpoint::Protocol;
use crate::error::Result;

use super::{Found, Imported, Source};

pub fn read(path: &Path) -> Result<Found> {
    let text = super::read_text(path)?;
    Ok(parse(&text, path))
}

/// One `Host` line and everything indented under it.
struct Block {
    patterns: Vec<String>,
    entries: Vec<(String, String)>,
}

pub fn parse(text: &str, path: &Path) -> Found {
    let mut found = Found::new(Source::SshConfig, path);
    let mut blocks: Vec<Block> = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Keyword and argument are separated by whitespace or by an equals
        // sign, and OpenSSH treats both the same.
        let (keyword, rest) = match line.split_once(|c: char| c.is_whitespace() || c == '=') {
            Some((keyword, rest)) => (keyword, rest.trim_start_matches(['=', ' ', '\t']).trim()),
            None => continue,
        };

        if keyword.eq_ignore_ascii_case("Host") {
            blocks.push(Block {
                patterns: rest.split_whitespace().map(str::to_string).collect(),
                entries: Vec::new(),
            });
        } else if let Some(block) = blocks.last_mut() {
            block
                .entries
                .push((keyword.to_string(), unquote(rest).to_string()));
        }
    }

    for (index, block) in blocks.iter().enumerate() {
        for pattern in &block.patterns {
            // A pattern is a rule for other hosts, not a host of its own.
            if pattern.contains('*') || pattern.contains('?') || pattern.starts_with('!') {
                continue;
            }

            let look = |keyword: &str| -> Option<&str> {
                // The first match wins, here and in every later block — which
                // is how `Host *` at the bottom fills gaps instead of
                // overwriting what was said above.
                blocks[index..].iter().find_map(|block| {
                    if !matches(&block.patterns, pattern) {
                        return None;
                    }
                    block
                        .entries
                        .iter()
                        .find(|(key, _)| key.eq_ignore_ascii_case(keyword))
                        .map(|(_, value)| value.as_str())
                })
            };

            let key_path = look("IdentityFile").map(str::to_string);
            let mut entry = Imported::named(pattern);
            entry.protocol = Protocol::Sftp;
            entry.host = look("HostName").unwrap_or(pattern).to_string();
            entry.port = look("Port")
                .and_then(|port| port.parse().ok())
                .unwrap_or(22);
            entry.user = look("User").unwrap_or_default().to_string();
            entry.auth = if key_path.is_some() {
                AuthKind::KeyFile
            } else {
                // No key named: the agent is what a terminal would have used,
                // and it is the one choice that needs nothing typed in.
                AuthKind::Agent
            };
            entry.key_path = key_path;
            found.entries.push(entry);
        }
    }

    found
}

/// Whether a `Host` pattern covers a name. Only `*` and `?`, which is all these
/// files use in practice.
fn matches(patterns: &[String], name: &str) -> bool {
    patterns.iter().any(|pattern| glob(pattern, name))
}

fn glob(pattern: &str, name: &str) -> bool {
    let (pattern, name): (Vec<char>, Vec<char>) =
        (pattern.chars().collect(), name.chars().collect());
    // The ordinary two-index walk with one remembered star, which is enough for
    // patterns of this shape and needs no allocation per step.
    let (mut p, mut n) = (0, 0);
    let (mut star, mut resume) = (None, 0);
    while n < name.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == name[n]) {
            p += 1;
            n += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            resume = n;
            p += 1;
        } else if let Some(back) = star {
            p = back + 1;
            resume += 1;
            n = resume;
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const SAMPLE: &str = r#"
# A comment
Host web kunde-web
    HostName example.org
    User dennis
    Port 2222
    IdentityFile ~/.ssh/id_ed25519

Host datenbank
    HostName db.example.org

Host *
    User fallback
    ServerAliveInterval 60
"#;

    fn parsed() -> Found {
        parse(SAMPLE, &PathBuf::from("config"))
    }

    #[test]
    fn every_name_of_a_host_becomes_an_entry() {
        let found = parsed();
        let names: Vec<&str> = found.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["web", "kunde-web", "datenbank"]);
    }

    #[test]
    fn a_pattern_is_a_rule_and_not_a_server() {
        // `Host *` must never turn up in somebody's server list.
        assert!(parsed().entries.iter().all(|e| e.name != "*"));
    }

    #[test]
    fn the_first_value_wins_so_a_fallback_fills_gaps() {
        let found = parsed();
        let web = &found.entries[0];
        assert_eq!(web.host, "example.org");
        assert_eq!(web.user, "dennis", "the specific block, not the fallback");
        assert_eq!(web.port, 2222);
        assert_eq!(web.auth, AuthKind::KeyFile);

        let db = found
            .entries
            .iter()
            .find(|e| e.name == "datenbank")
            .unwrap();
        assert_eq!(db.host, "db.example.org");
        assert_eq!(db.user, "fallback", "what Host * left for it");
        assert_eq!(db.port, 22);
        // Nothing said which key, so the agent — what a terminal would do.
        assert_eq!(db.auth, AuthKind::Agent);
    }

    #[test]
    fn nothing_here_carries_a_password_because_there_is_none_to_carry() {
        assert!(parsed().entries.iter().all(|e| !e.has_password));
    }

    #[test]
    fn keywords_may_be_separated_by_an_equals_sign() {
        let found = parse(
            "Host=eins\nHostName=eins.example\n",
            &PathBuf::from("config"),
        );
        assert_eq!(found.entries.len(), 1);
        assert_eq!(found.entries[0].host, "eins.example");
    }

    #[test]
    fn patterns_match_the_way_ssh_matches_them() {
        assert!(glob("*", "anything"));
        assert!(glob("*.example.org", "web.example.org"));
        assert!(!glob("*.example.org", "example.org"));
        assert!(glob("web?", "web1"));
        assert!(!glob("web?", "web12"));
        assert!(glob("kunde-*-web", "kunde-müller-web"));
    }
}
