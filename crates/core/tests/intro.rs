//! Smoke/regression tests on Monodraw's bundled `Monodraw-Intro.monopic` (a v2 document,
//! so it exercises the whole migration chain). Skipped without a Monodraw install: see `common`.

use solodraw_core::export::TextOptions;
use solodraw_core::{canvas_to_text, Document};

mod common;

fn intro() -> Option<Document> {
    common::monodraw_resource("Monodraw-Intro.monopic").map(|p| Document::open(p).expect("intro loads"))
}

fn opts() -> TextOptions {
    TextOptions { trim_trailing: true, ..Default::default() }
}

#[test]
fn intro_renders() {
    let Some(doc) = intro() else { return };
    assert_eq!(doc.objects.len(), 1443);
    let out = canvas_to_text(&doc, None, &opts()).unwrap();
    for needle in ["Drag Me Around!", "Thank you for using Monodraw :)", "┏━━━", "╔═══", "─┼─", "Customer"] {
        assert!(out.contains(needle), "missing {needle:?}");
    }
    let ascii = canvas_to_text(&doc, None, &TextOptions { unicode: Some(false), ..opts() }).unwrap();
    assert!(!ascii.contains('─') && ascii.contains("+---"));
}

#[test]
fn intro_round_trips() {
    let Some(doc) = intro() else { return };
    let before = canvas_to_text(&doc, None, &opts()).unwrap();
    for minify in [false, true] {
        let back = Document::from_bytes(&doc.to_monopic(minify)).unwrap();
        assert_eq!(canvas_to_text(&back, None, &opts()).unwrap(), before, "minify={minify}");
    }
    let back = Document::from_bytes(&doc.to_monojson()).unwrap();
    assert_eq!(canvas_to_text(&back, None, &opts()).unwrap(), before);
}
