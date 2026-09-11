//! Noticing that a newer release exists.
//!
//! Deliberately small and deliberately switchable off. The concept paper is
//! clear that AmberBeam sends nothing about its user anywhere; an update check
//! is the one request it makes on its own, so what it does has to be plain:
//!
//! * It asks GitHub for the newest release of this repository. Nothing else.
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
pub const LATEST_RELEASE: &str = "https://api.github.com/repos/sphings79/amberbeam/releases/latest";

/// Picks the release out of what GitHub answered.
pub fn read_answer(json: &str) -> Option<Release> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    // A draft or a pre-release is not something to point people at.
    if value.get("draft") == Some(&serde_json::Value::Bool(true))
        || value.get("prerelease") == Some(&serde_json::Value::Bool(true))
    {
        return None;
    }
    let tag = value.get("tag_name")?.as_str()?.to_string();
    let url = value
        .get("html_url")
        .and_then(|url| url.as_str())
        .unwrap_or("https://github.com/sphings79/amberbeam/releases")
        .to_string();
    let version = tag
        .trim_start_matches('v')
        .trim_start_matches('V')
        .to_string();
    Some(Release { tag, version, url })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn drafts_and_pre_releases_are_not_read() {
        let draft = r#"{"tag_name": "v0.3.0", "draft": true, "prerelease": false}"#;
        assert!(read_answer(draft).is_none());
        let early = r#"{"tag_name": "v0.3.0", "draft": false, "prerelease": true}"#;
        assert!(read_answer(early).is_none());
    }

    #[test]
    fn an_answer_that_is_not_json_is_not_a_release() {
        assert!(read_answer("<html>rate limited</html>").is_none());
        assert!(read_answer("{}").is_none());
    }
}
