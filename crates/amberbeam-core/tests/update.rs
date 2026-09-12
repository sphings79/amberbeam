//! The update notice against what GitHub really answers.
//!
//! The fixture is the actual response for this repository, saved the day 0.1.0
//! was published. It exists because the interesting case is not one this
//! project can invent: a repository whose only release is a pre-release, which
//! is what every release will be until 1.0. A handwritten sample would have
//! whatever shape the person writing it expected.

use amberbeam_core::update::{is_newer, read_answer};

fn answer() -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("releases.json"),
    )
    .expect("the saved answer")
}

#[test]
fn the_first_release_is_found_even_though_it_is_a_pre_release() {
    let found = read_answer(&answer()).expect("a release");
    assert_eq!(found.tag, "v0.1.0");
    assert_eq!(found.version, "0.1.0");
    assert!(found
        .url
        .starts_with("https://github.com/sphings79/amberbeam/releases/"));
}

#[test]
fn somebody_running_that_very_version_is_not_told_to_update() {
    // The other half of the promise. A notice that fires for the version
    // already installed is worse than none: it never goes away.
    let found = read_answer(&answer()).expect("a release");
    assert!(!is_newer("0.1.0", &found.version));
    assert!(!is_newer("0.2.0", &found.version));
    assert!(is_newer("0.0.9", &found.version));
}

#[test]
fn the_version_in_the_release_is_the_version_this_crate_calls_itself() {
    // If these drift, the notice tells everybody about an update they have.
    // The release workflow refuses to build when they disagree; this says the
    // same thing here, where it is cheap to notice.
    let found = read_answer(&answer()).expect("a release");
    assert_eq!(found.version, env!("CARGO_PKG_VERSION"));
}
