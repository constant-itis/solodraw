//! Writes the README showcase document: `showcase OUT.monopic [ascii]`. The title uses the `standard`
//! FIGlet font (`/usr/share/figlet`).
use serde_json::json;
use solodraw_core::edit::LineEnd;
use solodraw_core::geom::{Point, Rect};
use solodraw_core::{Document, ObjExt};

fn boxed(doc: &mut Document, canvas: &str, r: Rect, border: i64, text: &str) -> String {
    let id = doc.add_rect(canvas, r, text);
    doc.set_value(&id, "rectangular_border", json!(border));
    id
}

fn round(doc: &mut Document, id: &str) {
    for (k, c) in [("tl", "╭"), ("tr", "╮"), ("bl", "╰"), ("br", "╯")] {
        doc.set_value(id, &format!("border_tixel_{k}"), json!(c));
    }
}

fn link(doc: &mut Document, canvas: &str, a: &str, pa: i64, b: &str, pb: i64) -> String {
    let end = |s: &str, p| LineEnd::Attached { shape: s.to_string(), rect_position: p };
    doc.add_line(canvas, Point::default(), end(a, pa), end(b, pb), 1)
}

fn main() {
    let out = std::env::args().nth(1).expect("usage: showcase OUT.monopic");
    let ascii = std::env::args().nth(2).as_deref() == Some("ascii");
    let (mut doc, canvas) = Document::new_empty();

    let title = doc.add_rect_typed(&canvas, Rect::new(2, 0, 44, 6), "solodraw", 4);
    doc.set_value(&title, "rectangular_border_enabled", json!(false));
    let t = doc.get(&title).unwrap().str_("model_text").unwrap().to_string();
    doc.set_value(&t, "letter_font_id", json!("com.helftone.monodraw.figlet-font.standard"));

    let browser = boxed(&mut doc, &canvas, Rect::new(2, 11, 14, 5), 6, "Browser");
    let api = boxed(&mut doc, &canvas, Rect::new(28, 11, 16, 5), 5, "API\ngateway");
    doc.set_value(&api, "shadow_enabled", json!(true));
    doc.set_value(&api, "shadow_offset", json!("1,1"));
    doc.set_value(&api, "shadow_intensity", json!(1));
    let auth = boxed(&mut doc, &canvas, Rect::new(58, 6, 12, 3), 4, "Auth");
    if !ascii {
        round(&mut doc, &auth);
    }
    let cache = boxed(&mut doc, &canvas, Rect::new(58, 17, 12, 3), 4, "Cache");
    if !ascii {
        round(&mut doc, &cache);
    }
    let db = boxed(&mut doc, &canvas, Rect::new(29, 22, 14, 3), 4, "Postgres");

    let https = link(&mut doc, &canvas, &browser, 16, &api, 15);
    doc.add_line_label(&canvas, &https, "HTTPS");
    link(&mut doc, &canvas, &api, 16, &auth, 15);
    link(&mut doc, &canvas, &api, 16, &cache, 15);
    link(&mut doc, &canvas, &api, 19, &db, 12);

    if ascii {
        let root = doc.root().unwrap().0.to_string();
        doc.set_value(&root, "alphabet", json!(2));
    }
    doc.save(&out).unwrap();
}
