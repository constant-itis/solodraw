//! Writes a small demo document (two boxes and an arrow) for screenshots: `demo_doc OUT.monopic`.
use solodraw_core::edit::LineEnd;
use solodraw_core::geom::{Point, Rect};
use solodraw_core::Document;

fn main() {
    let out = std::env::args().nth(1).expect("usage: demo_doc OUT.monopic");
    let (mut doc, canvas) = Document::new_empty();
    let a = doc.add_rect(&canvas, Rect::new(2, 2, 11, 3), "Client");
    doc.set_value(&a, "rectangular_border", serde_json::json!(6));
    doc.set_value(&a, "shadow_enabled", serde_json::json!(true));
    doc.set_value(&a, "shadow_offset", serde_json::json!("1,1"));
    let b = doc.add_rect(&canvas, Rect::new(22, 2, 11, 3), "Server");
    doc.add_line(&canvas, Point::default(), LineEnd::Attached { shape: a.clone(), rect_position: 16 }, LineEnd::Attached { shape: b, rect_position: 15 }, 1);
    if std::env::args().nth(2).as_deref() == Some("glyph") {
        doc.add_rect(&canvas, Rect::new(2, 8, 16, 3), "Missing: \u{0378}");
    }
    if std::env::args().nth(2).as_deref() == Some("diamond") {
        doc.add_diamond(&canvas, Point::new(48, 3), 3);
    }
    if std::env::args().nth(2).as_deref() == Some("image") {
        let png = std::fs::read(std::env::args().nth(3).expect("demo_doc OUT image PNG")).unwrap();
        doc.add_image(&canvas, &png, Rect::new(4, 1, 20, 6), 0.5);
    }
    if std::env::args().nth(2).as_deref() == Some("tables") {
        doc.add_table(&canvas, Rect::new(40, 8, 13, 5), 2, 2);
    }
    if matches!(std::env::args().nth(2).as_deref(), Some("table" | "tables")) {
        let t = doc.add_table(&canvas, Rect::new(2, 8, 31, 7), 3, 3);
        for (r, c, s) in [(0, 0, "Name"), (0, 1, "Port"), (0, 2, "Host"), (1, 0, "api"), (1, 1, "8080"), (2, 0, "db"), (2, 1, "5432")] {
            doc.set_table_cell_text(&t, r, c, s);
        }
    }
    if std::env::args().nth(2).as_deref() == Some("group") {
        let c = doc.add_rect(&canvas, Rect::new(2, 8, 10, 3), "In A");
        let d = doc.add_rect(&canvas, Rect::new(14, 8, 10, 3), "In B");
        doc.group_shapes(&canvas, &[c, d]);
    }
    if std::env::args().nth(2).as_deref() == Some("multi") {
        let c2 = doc.add_canvas("Second Page");
        doc.add_rect(&c2, Rect::new(4, 4, 20, 5), "Page two");
    }
    doc.save(out).unwrap();
}
