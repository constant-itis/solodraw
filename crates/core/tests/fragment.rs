use solodraw_core::edit::LineEnd;
use solodraw_core::export::TextOptions;
use solodraw_core::geom::{Point, Rect};
use solodraw_core::{canvas_to_text, Document, ObjExt, Scene};

mod common;

fn text(doc: &Document) -> String {
    canvas_to_text(doc, None, &TextOptions { trim_trailing: true, ..Default::default() }).unwrap_or_default()
}

/// Two boxes joined by an arrow.
fn pair() -> (Document, String, String, String, String) {
    let (mut doc, canvas) = Document::new_empty();
    let a = doc.add_rect(&canvas, Rect::new(0, 0, 7, 3), "A");
    let b = doc.add_rect(&canvas, Rect::new(14, 0, 7, 3), "B");
    let l = doc.add_line(
        &canvas,
        Point::default(),
        LineEnd::Attached { shape: a.clone(), rect_position: 16 },
        LineEnd::Attached { shape: b.clone(), rect_position: 15 },
        1,
    );
    (doc, canvas, a, b, l)
}

fn frame(doc: &Document, canvas: &str, id: &str) -> Rect {
    Scene::new(doc, canvas, true).frame(id).unwrap()
}

#[test]
fn copy_paste_keeps_attachments_inside_the_fragment() {
    let (mut doc, canvas, a, b, l) = pair();
    let before = text(&doc);
    // Selection order doesn't matter; the copy keeps the z-order (line on top).
    let frag = doc.extract_fragment(&canvas, &[a.clone(), b.clone(), l.clone()]);
    let pasted = doc.paste_fragment(&frag, &canvas, Point::default(), Point::new(0, 4)).unwrap();
    assert_eq!(pasted.len(), 3);
    let top: Vec<&str> = doc.get(&canvas).unwrap().refs("subshape_refs")[..3].to_vec();
    assert_eq!(top, pasted.iter().map(String::as_str).collect::<Vec<_>>());
    assert_eq!(doc.get(&pasted[0]).unwrap().int("shape_type"), Some(2));
    let t = text(&doc);
    assert_eq!(t, format!("{before}\n\n{before}"), "{t}");
    // The copy's line is attached to the copy's boxes: moving the copied B bends only that line.
    let b2 = pasted[1].clone();
    doc.translate_shape(&b2, Point::new(0, 3));
    assert_eq!(text(&doc).lines().take(3).collect::<Vec<_>>().join("\n"), before);
    // Fresh ids, and the saved file reloads to the same picture.
    assert!(pasted.iter().all(|p| ![&a, &b, &l].contains(&p)));
    let back = Document::from_bytes(&doc.to_monopic(false)).unwrap();
    assert_eq!(text(&back), text(&doc));
}

#[test]
fn copying_a_line_alone_detaches_it_in_place() {
    let (mut doc, canvas, a, _b, l) = pair();
    let frag = doc.extract_fragment(&canvas, &[l.clone()]);
    let pasted = doc.paste_fragment(&frag, &canvas, Point::default(), Point::new(0, 5)).unwrap();
    // The copy is a free line at the same place, five rows down.
    assert_eq!(frame(&doc, &canvas, &pasted[0]), {
        let f = frame(&doc, &canvas, &l);
        Rect::new(f.x, f.y + 5, f.w, f.h)
    });
    // Moving A drags only the original line.
    doc.translate_shape(&a, Point::new(0, 1));
    assert_eq!(frame(&doc, &canvas, &pasted[0]).y, 6);
}

#[test]
fn group_then_ungroup_keeps_the_picture() {
    let (mut doc, canvas, a, b, l) = pair();
    let before = text(&doc);
    let g = doc.group_shapes(&canvas, &[a.clone(), b.clone()]).unwrap();
    assert_eq!(text(&doc), before);
    let kids = doc.get(&g).unwrap().refs("subshape_refs").into_iter().map(str::to_string).collect::<Vec<_>>();
    assert_eq!(kids, vec![b.clone(), a.clone()]);
    // Moving the group moves both boxes, and the line follows.
    doc.translate_shape(&g, Point::new(2, 0));
    let shifted: Vec<String> = before.lines().map(|l| format!("  {l}")).collect();
    assert_eq!(text(&doc), shifted.join("\n"));
    let back = doc.ungroup(&canvas, &g);
    assert_eq!(back, vec![b.clone(), a.clone()]);
    assert!(doc.get(&g).is_none());
    assert_eq!(frame(&doc, &canvas, &a).x, 2);
    assert_eq!(text(&doc), shifted.join("\n"));
    let _ = l;
}

#[test]
fn flatten_turns_shapes_into_one_surface() {
    let (mut doc, canvas, a, b, l) = pair();
    let before = text(&doc);
    let s = doc.flatten(&canvas, &[a.clone(), b.clone(), l.clone()]).unwrap();
    assert_eq!(text(&doc), before);
    let root = doc.get(&canvas).unwrap().refs("subshape_refs");
    assert_eq!(root, vec![s.as_str()]);
    assert_eq!(doc.get(&s).unwrap().int("shape_type"), Some(5));
}

#[test]
fn style_copy_applies_border_and_text_settings() {
    let (mut doc, canvas) = Document::new_empty();
    let a = doc.add_rect(&canvas, Rect::new(0, 0, 7, 3), "A");
    let b = doc.add_rect(&canvas, Rect::new(10, 0, 7, 3), "B");
    doc.set_value(&a, "rectangular_border", serde_json::json!(2));
    let ta = doc.text_of(&a).unwrap();
    doc.set_value(&ta, "alignment", serde_json::json!(0));
    let style = doc.style_of(&a).unwrap();
    doc.apply_style(&b, &style);
    assert_eq!(doc.get(&b).unwrap().int("rectangular_border"), Some(2));
    let tb = doc.text_of(&b).unwrap();
    assert_eq!(doc.get(&tb).unwrap().int("alignment"), Some(0));
    // Text and geometry stay.
    assert_eq!(doc.get(&tb).unwrap().str_("text"), Some("B"));
    assert_eq!(frame(&doc, &canvas, &b).x, 10);
}

#[test]
fn text_surface_from_plain_text() {
    let (mut doc, canvas) = Document::new_empty();
    let s = doc.add_text_surface(&canvas, Point::new(3, 2), "+--+\n|ab|\n+--+").unwrap();
    assert_eq!(frame(&doc, &canvas, &s), Rect::new(3, 2, 4, 3));
    assert_eq!(text(&doc), "\n\n   +--+\n   |ab|\n   +--+");
}

#[test]
fn bundled_snippet_loads_when_extracted() {
    let Some(p) = common::monodraw_resource("snippets/Standard Snippets/Phone.monosnippet/snippet-content.json") else { return };
    let data = std::fs::read(p).unwrap();
    let frag: serde_json::Value = serde_json::from_slice(&data).unwrap();
    let (mut doc, canvas) = Document::new_empty();
    let roots = doc.paste_fragment(&frag, &canvas, Point::default(), Point::default()).unwrap();
    assert_eq!(roots.len(), 1);
    let t = text(&doc);
    assert!(t.lines().count() > 10, "{t}");
}

#[test]
fn shipped_snippets_render() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/snippets");
    let mut n = 0;
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let data = std::fs::read(e.path().join("snippet-content.json")).unwrap();
        let frag: serde_json::Value = serde_json::from_slice(&data).unwrap();
        let (mut doc, canvas) = Document::new_empty();
        let roots = doc.paste_fragment(&frag, &canvas, Point::default(), Point::default()).unwrap();
        assert_eq!(roots.len(), 1, "{:?}", e.path());
        let t = text(&doc);
        println!("{}\n{t}\n", e.file_name().to_string_lossy());
        assert!(!t.trim().is_empty());
        n += 1;
    }
    assert!(n >= 7);
}

#[test]
fn comment_styles_and_wrapping() {
    use solodraw_core::export::{decorate, wrap_output, Decoration};
    let t = "ab\nc";
    let d = |comment, padding| Decoration { comment, padding, ..Default::default() };
    assert_eq!(decorate(t, &d(1, 1), true), "// ab\n// c");
    assert_eq!(decorate(t, &d(9, 0), true), "#ab\n#c");
    assert_eq!(decorate(t, &d(2, 1), true), "/*\n ab\n c\n*/");
    assert_eq!(decorate(t, &d(7, 1), true), "/* ab\n * c\n */");
    assert_eq!(decorate(t, &d(8, 0), true), "/*ab\n**c\n*/");
    assert_eq!(decorate(t, &d(6, 0), true), "<!--\nab\nc\n-->");
    assert_eq!(decorate(t, &d(4, 0), false), "/**\\\n*ab*\n*c *\n\\**/");
    assert_eq!(decorate(t, &d(13, 1), false), "******\n*    *\n* ab *\n* c  *\n*    *\n******");
    assert_eq!(decorate(t, &d(10, 3), true), "\tab\n\tc");
    assert_eq!(decorate(t, &d(11, 0), true), "    ab\n    c");
    assert_eq!(decorate(t, &d(12, 0), true), "```\nab\nc\n```");
    let f = decorate("x", &Decoration { footer: true, footer_right: true, ..Default::default() }, true);
    assert!(f.ends_with("Created with solodraw") && f.starts_with("x\n\n"), "{f}");
    assert_eq!(wrap_output("a<b", 0), "<pre>\na&lt;b\n</pre>");
    assert_eq!(wrap_output("\"$é", 3), "echo \"\\x22\\x24\\xc3\\xa9\";\n");
    assert_eq!(wrap_output("é\"", 5), "print(\"\\u00e9\\\"\");\n");
    assert_eq!(wrap_output("é\"", 4), "puts \"é\\\"\";\n");
}

#[test]
fn image_overlay_copies_and_pastes() {
    let (mut doc, canvas) = Document::new_empty();
    let img = doc.add_image(&canvas, b"pixels", Rect::new(3, 2, 8, 4), 0.25);
    let frag = doc.extract_fragment(&canvas, &[img.clone()]);
    let pasted = doc.paste_fragment(&frag, &canvas, Point::default(), Point::new(10, 0)).unwrap();
    assert_eq!(pasted.len(), 1);
    let copy = &pasted[0];
    assert_ne!(copy, &img);
    assert_eq!(doc.image_data(copy).unwrap(), b"pixels");
    assert_eq!(doc.get(copy).unwrap().float("img_opacity"), Some(0.25));
    assert_eq!(frame(&doc, &canvas, copy), Rect::new(13, 2, 8, 4));
}

#[test]
fn flatten_keeps_colours() {
    use solodraw_core::render::render_scene;
    let colours = |doc: &Document, canvas: &str| -> Vec<(i32, i32, u64)> {
        let buf = render_scene(&Scene::new(doc, canvas, true));
        let mut v: Vec<(i32, i32, u64)> = buf.cells.iter().filter(|(_, c)| c.t.ch.is_some_and(|ch| ch != ' ')).map(|(p, c)| (p.x, p.y, c.color)).collect();
        v.sort();
        v
    };
    // One colour: a single surface carrying it.
    let (mut doc, canvas) = Document::new_empty();
    let a = doc.add_rect(&canvas, Rect::new(0, 0, 5, 3), "");
    doc.set_value(&a, "mps_border_color", serde_json::json!(0xff0000u64));
    let before = colours(&doc, &canvas);
    let s = doc.flatten(&canvas, &[a]).unwrap();
    assert_eq!(doc.get(&s).unwrap().int("shape_type"), Some(5));
    assert_eq!(colours(&doc, &canvas), before);

    // Two colours: one surface each, grouped; survives a save.
    let (mut doc, canvas) = Document::new_empty();
    let a = doc.add_rect(&canvas, Rect::new(0, 0, 5, 3), "A");
    let b = doc.add_rect(&canvas, Rect::new(8, 0, 5, 3), "B");
    doc.set_value(&a, "mps_border_color", serde_json::json!(0xff0000u64));
    doc.set_value(&b, "mps_border_color", serde_json::json!(0x00ff00u64));
    let (t0, before) = (text(&doc), colours(&doc, &canvas));
    let g = doc.flatten(&canvas, &[a, b]).unwrap();
    assert_eq!(doc.get(&g).unwrap().type_id(), 3, "grouped");
    assert_eq!(doc.get(&g).unwrap().refs("subshape_refs").len(), 3, "red, green, default (text)");
    let back = Document::from_bytes(&doc.to_monopic(false)).unwrap();
    assert_eq!(text(&back), t0);
    assert_eq!(colours(&back, &canvas), before);
}

#[test]
fn duplicate_names() {
    use solodraw_core::fragment::copy_name;
    assert_eq!(copy_name("Rectangle 1"), "Rectangle 1 Copy");
    assert_eq!(copy_name("Rectangle 1 Copy"), "Rectangle 1 Copy 2");
    assert_eq!(copy_name("Rectangle 1 Copy 2"), "Rectangle 1 Copy 3");
    assert_eq!(copy_name("Rectangle 1 Copy 9"), "Rectangle 1 Copy 10");
    assert_eq!(copy_name("A Copy 1"), "A Copy 3", "numbers below 2 count as 2");
    assert_eq!(copy_name(""), "Copy");
    assert_eq!(copy_name("Copy"), "Copy Copy");
}

#[test]
fn create_style_rules() {
    let (mut doc, canvas) = Document::new_empty();
    let src = doc.add_rect(&canvas, Rect::new(0, 0, 6, 3), "S");
    doc.set_value(&src, "rectangular_border", serde_json::json!(6));
    doc.set_value(&src, "shadow_enabled", serde_json::json!(true));
    let t = doc.text_of(&src).unwrap();
    doc.set_value(&t, "alignment", serde_json::json!(0));
    // Rect-positioned to rect-positioned: shape style, not text style.
    let r = doc.add_rect(&canvas, Rect::new(10, 0, 6, 3), "R");
    assert!(doc.apply_create_style(&src, &r));
    assert_eq!(doc.get(&r).unwrap().int("rectangular_border"), Some(6));
    let rt = doc.text_of(&r).unwrap();
    assert_eq!(doc.get(&rt).unwrap().int("alignment"), Some(1), "text style not copied");
    let e = doc.add_ellipse(&canvas, Rect::new(20, 0, 8, 5));
    assert!(doc.apply_create_style(&src, &e));
    assert!(doc.get(&e).unwrap().boolean("shadow_enabled"));
    assert!(!doc.get(&e).unwrap().contains_key("rectangular_border"), "only keys the ellipse has");
    // Diamonds only from diamonds.
    let d = doc.add_diamond(&canvas, Point::new(40, 3), 2);
    assert!(!doc.apply_create_style(&src, &d));
    assert!(!doc.get(&d).unwrap().boolean("shadow_enabled"));
}
