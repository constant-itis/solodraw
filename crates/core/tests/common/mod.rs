//! Private fixtures: drawings that belong to the author, plus Monodraw's exports of them. They
//! are not in the public repository. Point `SOLODRAW_PRIVATE_FIXTURES` at a copy (or put one at
//! `fixtures/private/`) to run the tests that use them; without it those tests skip.

#![allow(dead_code)]

use std::path::PathBuf;

/// `<private>/<rel>` if the private fixtures are available, else `None` (and a note on stderr).
pub fn private(rel: &str) -> Option<PathBuf> {
    let root = std::env::var_os("SOLODRAW_PRIVATE_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/private"));
    let p = root.join(rel);
    if !p.exists() {
        eprintln!("skipped: private fixture {rel} not present");
    }
    p.exists().then_some(p)
}

/// The text of `<private>/<rel>`.
pub fn private_text(rel: &str) -> Option<String> {
    private(rel).map(|p| std::fs::read_to_string(p).unwrap())
}

/// `<Monodraw.app>/Contents/Resources/<rel>` from a Monodraw install, if there is one. Point
/// `SOLODRAW_MONODRAW_RESOURCES` at that directory (default: the macOS install location).
pub fn monodraw_resource(rel: &str) -> Option<PathBuf> {
    let root = std::env::var_os("SOLODRAW_MONODRAW_RESOURCES")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/Applications/Monodraw.app/Contents/Resources"));
    let p = root.join(rel);
    p.exists().then_some(p)
}
