//! Noticing that a newer release exists.
//!
//! Deliberately small and deliberately switchable off. The concept paper is
//! clear that AmberBeam sends nothing about its user anywhere; an update check
//! is the one request it makes on its own, so what it does has to be plain:
//!
//! * It asks GitHub for this repository's releases. Nothing else.
//! * It sends no version, no identifier, no count of anything. The request
//!   reveals an address and a moment in time, which is what any request does.
//! * It can be switched off, and then nothing is asked at all.
//!
//! The comparison lives here, apart from the asking, because version numbers
//! are where this sort of thing usually goes wrong.

use serde::{Deserialize, Serialize};

/// What a check found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    /// The tag as GitHub reports it, `v0.2.0` or `0.2.0`.
    pub tag: String,
    /// Version without the leading `v`, for showing.
    pub version: String,
    /// Where to read about it.
    pub url: String,
    /// What the release says about itself, as written.
    ///
    /// Carried so the window can show what changed before somebody decides to
    /// fetch it. It arrives from the network and is never treated as anything
    /// but text — the window builds its own structure from it rather than
    /// letting a release describe what to draw.
    pub notes: String,
}

/// A version as three numbers, which is all this project uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Version(u32, u32, u32);

/// Reads `1.2.3`, `v1.2.3` or `1.2` — anything else is not a version this
/// program can compare, and guessing would be worse than saying nothing.
fn parse(text: &str) -> Option<Version> {
    let text = text.trim().trim_start_matches('v').trim_start_matches('V');
    // A suffix like `-rc1` means a pre-release, and those are not offered.
    if text.contains('-') || text.contains('+') {
        return None;
    }
    let mut parts = text.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(Version(major, minor, patch))
}

/// Whether `latest` is worth telling the user about.
///
/// Anything unparseable answers "no". A notice that cannot be trusted is worse
/// than no notice: it sends people looking for an update that may not exist.
pub fn is_newer(current: &str, latest: &str) -> bool {
    match (parse(current), parse(latest)) {
        (Some(current), Some(latest)) => latest > current,
        _ => false,
    }
}

/// The address the check asks. Public so the shell doing the asking cannot
/// invent a different one.
///
/// The list rather than `/releases/latest`, and that is not a detail. GitHub's
/// "latest" leaves out pre-releases, and every release of AmberBeam is one
/// until 1.0 — so asking for "latest" would mean the whole notice quietly does
/// nothing for the entire life of the 0.x series. A feature that exists and
/// does nothing is worse than one that was never offered.
pub const LATEST_RELEASE: &str =
    "https://api.github.com/repos/sphings79/amberbeam/releases?per_page=20";

/// Picks the newest release out of what GitHub answered.
///
/// Drafts are skipped: they are not published and their tag may not exist yet.
/// Pre-releases are not, for the reason above.
///
/// The newest is decided by comparing versions rather than by trusting the
/// order of the list. GitHub sorts by when a release was created, and a fix
/// published for an older line afterwards would otherwise look like the newest
/// thing there is.
pub fn read_answer(json: &str) -> Option<Release> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;

    // One release or a list of them: the shell should not have to care which
    // address answered, and a single object is what `/releases/latest` gives.
    let entries: Vec<&serde_json::Value> = match value.as_array() {
        Some(list) => list.iter().collect(),
        None => vec![&value],
    };

    entries
        .into_iter()
        .filter(|entry| entry.get("draft") != Some(&serde_json::Value::Bool(true)))
        .filter_map(release_of)
        .max_by_key(|release| parse(&release.version))
}

fn release_of(entry: &serde_json::Value) -> Option<Release> {
    let tag = entry.get("tag_name")?.as_str()?.to_string();
    let version = tag
        .trim_start_matches('v')
        .trim_start_matches('V')
        .to_string();
    // A tag this program cannot compare is a tag it must not offer: it would
    // send somebody looking for an update that may be older than what they run.
    parse(&version)?;

    Some(Release {
        tag,
        notes: entry
            .get("body")
            .and_then(|body| body.as_str())
            .unwrap_or_default()
            .to_string(),
        url: entry
            .get("html_url")
            .and_then(|url| url.as_str())
            .unwrap_or("https://github.com/sphings79/amberbeam/releases")
            .to_string(),
        version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_notes_travel_with_the_release() {
        let answer = serde_json::json!([{
            "tag_name": "v0.3.0",
            "html_url": "https://github.com/sphings79/amberbeam/releases/tag/v0.3.0",
            "body": "### Fixed\n\n- Something that was broken.",
            "draft": false,
        }]);
        let found = read_answer(&answer.to_string()).unwrap();
        assert_eq!(found.notes, "### Fixed\n\n- Something that was broken.");
    }

    /// A release with nothing written about it is not an error. It is a
    /// release somebody published in a hurry, and the window has to cope.
    #[test]
    fn a_release_without_notes_is_still_a_release() {
        let answer = serde_json::json!([{
            "tag_name": "v0.3.0",
            "html_url": "https://github.com/sphings79/amberbeam/releases/tag/v0.3.0",
            "draft": false,
        }]);
        let found = read_answer(&answer.to_string()).unwrap();
        assert_eq!(found.notes, "");
    }

    #[test]
    fn a_higher_version_is_newer() {
        assert!(is_newer("0.1.0", "0.2.0"));
        assert!(is_newer("0.1.0", "0.1.1"));
        assert!(is_newer("0.9.9", "1.0.0"));
        assert!(
            is_newer("0.1.0", "v0.2.0"),
            "the leading v is not part of it"
        );
    }

    #[test]
    fn the_same_or_an_older_version_is_not() {
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.2.0", "0.1.9"));
        assert!(!is_newer("1.0.0", "0.9.9"));
    }

    #[test]
    fn ten_comes_after_nine_rather_than_before_it() {
        // The mistake a string comparison makes, and the reason for parsing.
        assert!(is_newer("0.9.0", "0.10.0"));
        assert!(!is_newer("0.10.0", "0.9.0"));
    }

    #[test]
    fn a_pre_release_is_never_offered() {
        assert!(!is_newer("0.1.0", "0.2.0-rc1"));
        assert!(!is_newer("0.1.0", "0.2.0+build7"));
    }

    #[test]
    fn nonsense_answers_no_rather_than_guessing() {
        assert!(!is_newer("0.1.0", "tomorrow"));
        assert!(!is_newer("0.1.0", ""));
        assert!(!is_newer("0.1.0", "0.2.0.1"));
        assert!(!is_newer("unknown", "0.2.0"));
    }

    #[test]
    fn a_release_is_read_out_of_the_answer() {
        let json = r#"{
            "tag_name": "v0.3.0",
            "html_url": "https://github.com/sphings79/amberbeam/releases/tag/v0.3.0",
            "draft": false,
            "prerelease": false
        }"#;
        let release = read_answer(json).expect("a release");
        assert_eq!(release.version, "0.3.0");
        assert!(release.url.ends_with("v0.3.0"));
        assert!(is_newer("0.1.0", &release.version));
    }

    #[test]
    fn a_draft_is_not_read_but_a_pre_release_is() {
        // A draft is not published and its tag may not exist yet.
        let draft = r#"{"tag_name": "v0.3.0", "draft": true, "prerelease": false}"#;
        assert!(read_answer(draft).is_none());

        // A pre-release is. Every release of AmberBeam is one until 1.0, and a
        // check that skipped them would do nothing at all for the whole of the
        // 0.x series — a feature that exists and does nothing is worse than one
        // that was never offered.
        let early = r#"{"tag_name": "v0.3.0", "draft": false, "prerelease": true}"#;
        assert_eq!(read_answer(early).map(|r| r.version), Some("0.3.0".into()));
    }

    #[test]
    fn the_newest_is_the_highest_version_and_not_the_first_in_the_list() {
        // GitHub sorts by when a release was created. A fix published for an
        // older line afterwards sits at the top of that list and is not the
        // newest thing there is.
        let list = r#"[
            {"tag_name": "v0.1.4", "draft": false, "prerelease": true},
            {"tag_name": "v0.3.0", "draft": false, "prerelease": true},
            {"tag_name": "v0.2.9", "draft": false, "prerelease": false}
        ]"#;
        assert_eq!(read_answer(list).map(|r| r.version), Some("0.3.0".into()));
    }

    #[test]
    fn a_draft_in_a_list_is_passed_over_rather_than_ending_the_search() {
        let list = r#"[
            {"tag_name": "v9.9.9", "draft": true, "prerelease": false},
            {"tag_name": "v0.2.0", "draft": false, "prerelease": true}
        ]"#;
        assert_eq!(read_answer(list).map(|r| r.version), Some("0.2.0".into()));
    }

    #[test]
    fn a_tag_that_cannot_be_compared_is_not_offered() {
        // Offering it would send somebody looking for an update that might be
        // older than what they are running.
        let list = r#"[{"tag_name": "nightly", "draft": false, "prerelease": true}]"#;
        assert!(read_answer(list).is_none());
        let mixed = r#"[
            {"tag_name": "nightly", "draft": false, "prerelease": true},
            {"tag_name": "v0.1.0", "draft": false, "prerelease": true}
        ]"#;
        assert_eq!(read_answer(mixed).map(|r| r.version), Some("0.1.0".into()));
    }

    #[test]
    fn an_empty_list_is_no_release_rather_than_a_failure() {
        // A repository with no releases yet is the ordinary state of one on its
        // first day, and it must not look like something went wrong.
        assert!(read_answer("[]").is_none());
    }

    #[test]
    fn an_answer_that_is_not_json_is_not_a_release() {
        assert!(read_answer("<html>rate limited</html>").is_none());
        assert!(read_answer("{}").is_none());
    }
}
