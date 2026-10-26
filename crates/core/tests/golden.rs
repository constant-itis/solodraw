//! Golden exports from real Monodraw 1.6.1, compared byte for byte (black-box captures):
//! - private `cli-golden`: the bundled CLI, every original as ASCII (no flag) and Unicode (`-u`)
//!   (the originals are the author's documents: see `common`);
//! - fixtures/gui: GUI File > Export > Text (default options) of test drawings made in the GUI.

use std::path::{Path, PathBuf};

mod common;

use solodraw_core::export::TextOptions;
use solodraw_core::{canvas_to_text, Document};

/// (mode, original) pairs that still differ from Monodraw.
/// Remove an entry when a fix makes it match (the test fails until you do).
const KNOWN_DIFFS: &[(&str, &str)] = &[];


fn originals(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            originals(&p, out);
        } else if p.extension().is_some_and(|x| x == "monopic") {
            out.push(p);
        }
    }
}

/// What Monodraw's CLI prints: the trimmed drawing plus a newline, or `(null)` when empty.
fn cli_output(doc: &Document, unicode: bool) -> String {
    let o = TextOptions { trim_padding: true, unicode: Some(unicode), ..Default::default() };
    match canvas_to_text(doc, None, &o) {
        Some(s) => s + "\n",
        None => "(null)".to_string(),
    }
}

#[test]
fn matches_monodraw_cli_exports() {
    let Some(root) = common::private("cli-golden") else { return };
    let mut files = Vec::new();
    originals(&root.join("originals"), &mut files);
    let (mut checked, mut wrong, mut now_match) = (0, Vec::new(), Vec::new());
    for f in &files {
        let rel = f.strip_prefix(root.join("originals")).unwrap().to_str().unwrap().to_string();
        let doc = Document::open(f).unwrap();
        for (mode, unicode) in [("ascii", false), ("unicode", true)] {
            let gold = root.join("08-roundtrip").join(format!("{rel}.{mode}.txt"));
            let Ok(gold) = std::fs::read_to_string(&gold) else { continue };
            checked += 1;
            let same = cli_output(&doc, unicode) == gold;
            let known = KNOWN_DIFFS.contains(&(mode, rel.as_str()));
            if !same && !known {
                wrong.push(format!("{mode} {rel}"));
            } else if same && known {
                now_match.push(format!("{mode} {rel}"));
            }
        }
    }
    assert_eq!(checked, 90, "expected 45 originals x 2 modes");
    assert!(wrong.is_empty(), "regressed against Monodraw:\n{}", wrong.join("\n"));
    assert!(now_match.is_empty(), "now matching, drop from KNOWN_DIFFS:\n{}", now_match.join("\n"));
}

/// GUI captures that still differ.
const GUI_KNOWN_DIFFS: &[&str] = &[
    // 13x16: the outline passes within a few thousandths of a cell of a cell corner and the
    // 5,760 samples step over cell (5,11) and its mirror (SPEC §6.2).
    "02-ellipse-thinning/ellipse-H13-W11-16-line",
    "02-ellipse-thinning/ellipse-H13-W11-16-star",
];

/// What File > Export > Text writes with its defaults: trailing whitespace and padding
/// trimmed, no final newline.
fn gui_output(doc: &Document) -> String {
    let o = TextOptions { trim_padding: true, trim_trailing: true, ..Default::default() };
    canvas_to_text(doc, None, &o).unwrap_or_default()
}

#[test]
fn matches_monodraw_gui_exports() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gui");
    let (mut checked, mut wrong, mut now_match) = (0, Vec::new(), Vec::new());
    for step in ["01-diagram-router", "02-ellipse-thinning", "03-er-crowsfoot"] {
        let mut names: Vec<PathBuf> = std::fs::read_dir(root.join(step))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "txt") && !p.ends_with("NOTES.md"))
            .collect();
        names.sort();
        for gold in names {
            let name = format!("{step}/{}", gold.file_stem().unwrap().to_str().unwrap());
            let Ok(doc) = Document::open(&gold.with_extension("monopic")) else { continue };
            checked += 1;
            let same = gui_output(&doc) == std::fs::read_to_string(&gold).unwrap();
            let known = GUI_KNOWN_DIFFS.contains(&name.as_str());
            if !same && !known {
                wrong.push(name);
            } else if same && known {
                now_match.push(name);
            }
        }
    }
    assert_eq!(checked, 128 + 107 + 18, "expected router + ellipse + ER pairs");
    assert!(wrong.is_empty(), "regressed against Monodraw:\n{}", wrong.join("\n"));
    assert!(now_match.is_empty(), "now matching, drop from GUI_KNOWN_DIFFS:\n{}", now_match.join("\n"));
}
