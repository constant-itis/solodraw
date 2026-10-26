//! The author's real documents (a private fixture: see `common`) (Monodraw 2021–2023, model v3 → full 3→4→5 migration).

use std::path::PathBuf;

use solodraw_core::export::TextOptions;
use solodraw_core::{canvas_to_text, Document};

mod common;

fn fixture() -> Option<String> {
    common::private_text("ASCII_CG_Layout.txt")
}

fn dir() -> Option<PathBuf> {
    common::private("monopic")
}

fn render(doc: &Document, trim_padding: bool) -> String {
    let o = TextOptions { trim_trailing: true, trim_padding, ..Default::default() };
    canvas_to_text(doc, None, &o).unwrap_or_default()
}

fn open(name: &str) -> Document {
    Document::open(dir().unwrap().join(name)).unwrap()
}

#[test]
fn all_real_files_render_and_round_trip() {
    let Some(dir) = dir() else { return };
    let mut n = 0;
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_none_or(|x| x != "monopic") {
            continue;
        }
        let doc = Document::open(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let before = render(&doc, false);
        // card_cookBook.monopic is an empty canvas (document + canvas only).
        assert!(!before.trim().is_empty() || doc.objects.len() <= 2, "{} renders empty", p.display());
        for minify in [false, true] {
            let back = Document::from_bytes(&doc.to_monopic(minify)).unwrap();
            assert_eq!(render(&back, false), before, "{} minify={minify}", p.display());
        }
        n += 1;
    }
    assert_eq!(n, 34);
}

/// Locate `block` (lines) inside the fixture text.
fn found_in_fixture(fixture: &str, block: &str) -> bool {
    let fx: Vec<Vec<char>> = fixture.lines().map(|l| l.chars().collect()).collect();
    let b: Vec<Vec<char>> = block.lines().map(|l| l.chars().collect()).collect();
    (0..fx.len().saturating_sub(b.len() - 1)).any(|y| {
        (0..fx[y].len()).any(|x| b.iter().enumerate().all(|(i, row)| fx[y + i].get(x..x + row.len()) == Some(&row[..])))
    })
}

#[test]
fn grafting_knife_card_matches_monodraw_export() {
    let (Some(fixture), Some(_)) = (fixture(), dir()) else { return };
    let out = render(&open("grafting_knife.monopic"), true);
    assert_eq!(out.lines().count(), 16);
    assert!(found_in_fixture(&fixture, &out), "{out}");
}

/// `ascii_all_1` (Jan 2023) is an earlier version of the document the fixture was exported
/// from (Jan 2025). Apart from cards edited since, it must line up exactly — same origin,
/// same cells.
#[test]
fn ascii_all_1_lines_up_with_later_export() {
    let (Some(fixture), Some(_)) = (fixture(), dir()) else { return };
    let out = render(&open("ascii_all_1.monopic"), false);
    let fx: Vec<&str> = fixture.lines().collect();
    let same = out.lines().zip(fx.iter()).filter(|(a, b)| a == *b).count();
    // 42 lines differ only by later content edits (renamed cards, a removed card row, a header nudge).
    assert!(same >= 131, "only {same} identical lines");
    assert_eq!(out.lines().next(), fx.first().copied());
}
