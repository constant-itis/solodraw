//! Writes the standard snippets shipped with the editor to `data/snippets/`.
//!
//!     cargo run -p solodraw-core --example make_snippets
//!
//! Each snippet is a `<Name>.monosnippet` folder with `snippet-metadata.json` and
//! `snippet-content.json` (a shape fragment, see `fragment.rs`).

use solodraw_core::geom::Rect;
use solodraw_core::Document;
use serde_json::json;

const ROUNDED: [(&str, &str); 4] = [("border_tixel_tl", "╭"), ("border_tixel_tr", "╮"), ("border_tixel_bl", "╰"), ("border_tixel_br", "╯")];

fn text_align(doc: &mut Document, id: &str, alignment: i64, position: i64) {
    if let Some(t) = doc.text_of(id) {
        doc.set_value(&t, "alignment", json!(alignment));
        doc.set_value(&t, "position", json!(position));
        doc.set_value(&t, "inset_left", json!(1));
        doc.set_value(&t, "inset_right", json!(1));
    }
}

fn named(doc: &mut Document, id: &str, name: &str) {
    doc.set_value(id, "name", json!(name));
    doc.set_value(id, "name_edited", json!(true));
}

/// Title bar over a body, grouped. Returns (group, body).
fn window(doc: &mut Document, canvas: &str, title: &str, w: i32, h: i32) -> (String, String) {
    let g = doc.add_group(canvas);
    named(doc, &g, title);
    let body = doc.add_rect(&g, Rect::new(0, 2, w, h - 2), "");
    named(doc, &body, "Body");
    let bar = doc.add_rect(&g, Rect::new(0, 0, w, 3), title);
    named(doc, &bar, "Title Bar");
    text_align(doc, &bar, 0, 1);
    (g, body)
}

fn button(doc: &mut Document, parent: &str, r: Rect, label: &str) -> String {
    let b = doc.add_rect(parent, r, label);
    named(doc, &b, label);
    for (k, v) in ROUNDED {
        doc.set_value(&b, k, json!(v));
    }
    b
}

fn write(name: &str, id: &str, tags: &str, description: &str, build: impl FnOnce(&mut Document, &str) -> String) {
    let (mut doc, canvas) = Document::new_empty();
    let root = build(&mut doc, &canvas);
    let content = doc.extract_fragment(&canvas, &[root]);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/snippets").join(format!("{name}.monosnippet"));
    std::fs::create_dir_all(&dir).unwrap();
    let meta = json!({
        "author": "solodraw",
        "email": "",
        "version": 0,
        "description": description,
        "identifier": format!("io.github.constant_itis.solodraw.snippet.{id}"),
        "tags": tags,
        "url": "",
    });
    std::fs::write(dir.join("snippet-metadata.json"), serde_json::to_string_pretty(&meta).unwrap() + "\n").unwrap();
    std::fs::write(dir.join("snippet-content.json"), serde_json::to_string_pretty(&content).unwrap() + "\n").unwrap();
    println!("{}", dir.display());
}

fn main() {
    for (size, id, w, h) in [("S", "s", 24, 8), ("M", "m", 40, 14), ("L", "l", 60, 20)] {
        write(&format!("Window ({size})"), &format!("window-{id}"), "window frame app", "A window with a title bar.", |doc, c| {
            window(doc, c, "Window", w, h).0
        });
    }
    write("Dialog", "dialog", "dialog alert modal window", "A dialog with a message and two buttons.", |doc, c| {
        let (g, _) = window(doc, c, "Dialog", 36, 11);
        let msg = doc.add_rect_typed(&g, Rect::new(2, 3, 32, 3), "Are you sure?", 4);
        named(doc, &msg, "Message");
        doc.set_value(&msg, "rectangular_border_enabled", json!(false));
        button(doc, &g, Rect::new(10, 7, 12, 3), "Cancel");
        button(doc, &g, Rect::new(23, 7, 11, 3), "OK");
        g
    });
    write("Browser", "browser", "browser web page url", "A browser window with an address bar.", |doc, c| {
        let (g, _) = window(doc, c, "◀ ▶ ⟳  https://example.com", 48, 16);
        named(doc, &g, "Browser");
        g
    });
    write("Button", "button", "button control", "A button with rounded corners.", |doc, c| button(doc, c, Rect::new(0, 0, 12, 3), "Button"));
    write("Note", "note", "note sticky comment", "A note card with a shadow.", |doc, c| {
        let n = doc.add_rect(c, Rect::new(0, 0, 22, 6), "Note");
        named(doc, &n, "Note");
        doc.set_value(&n, "shadow_enabled", json!(true));
        doc.set_value(&n, "shadow_offset", json!("1,1"));
        text_align(doc, &n, 0, 0);
        n
    });
}
