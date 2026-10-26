use solodraw_core::edit::LineEnd;
use solodraw_core::export::TextOptions;
use solodraw_core::geom::{Point, Rect};
use solodraw_core::{canvas_to_text, Document, ObjExt, Scene};

fn text(doc: &Document) -> String {
    canvas_to_text(doc, None, &TextOptions { trim_trailing: true, ..Default::default() }).unwrap_or_default()
}

#[test]
fn attached_line_follows_and_survives_delete() {
    let (mut doc, canvas) = Document::new_empty();
    let a = doc.add_rect(&canvas, Rect::new(0, 0, 7, 3), "A");
    let b = doc.add_rect(&canvas, Rect::new(14, 0, 7, 3), "B");
    doc.add_line(
        &canvas,
        Point::default(),
        LineEnd::Attached { shape: a.clone(), rect_position: 16 },
        LineEnd::Attached { shape: b.clone(), rect_position: 15 },
        1,
    );
    assert_eq!(text(&doc), "┌─────┐       ┌─────┐\n│  A  │──────▶│  B  │\n└─────┘       └─────┘");

    doc.translate_shape(&b, Point::new(0, 4));
    let t = text(&doc);
    assert!(t.lines().nth(5).unwrap().contains("▶│  B  │"), "{t}");

    // Hit test finds the topmost shape.
    let scene = Scene::new(&doc, &canvas, true);
    assert_eq!(scene.hit_test(Point::new(15, 5)).as_deref(), Some(b.as_str()));
    drop(scene);

    doc.delete_shape(&canvas, &b);
    let t = text(&doc);
    // The freed end has no exits, so it is entered opposite the start exit that points at it
    // a Z ending in ▶.
    assert!(!t.contains('B') && t.contains('▶'), "{t:?}");
    // No dangling references remain.
    let back = Document::from_bytes(&doc.to_monopic(false)).unwrap();
    assert_eq!(text(&back), t);
}

#[test]
fn raw_cells_edit() {
    let (mut doc, canvas) = Document::new_empty();
    let r = doc.add_rect(&canvas, Rect::new(0, 0, 5, 3), "");
    doc.set_tixmap_cell(&r, "content_tixels", Point::new(1, 1), Some('x'));
    doc.set_tixmap_cell(&r, "content_tixels", Point::new(2, 1), Some('y'));
    doc.set_tixmap_cell(&r, "content_tixels", Point::new(1, 1), None);
    assert_eq!(text(&doc), "┌───┐\n│ y │\n└───┘");
}

#[test]
fn ellipse_and_diamond_constructors() {
    let (mut doc, canvas) = Document::new_empty();
    let e = doc.add_ellipse(&canvas, Rect::new(0, 0, 11, 5));
    let d = doc.add_diamond(&canvas, Point::new(20, 3), 3);
    let scene = Scene::new(&doc, &canvas, true);
    assert_eq!(scene.frame(&e), Some(Rect::new(0, 0, 11, 5)));
    assert_eq!(scene.frame(&d), Some(Rect::new(17, 0, 7, 7)));
    assert_eq!(scene.hit_test(Point::new(20, 3)).as_deref(), Some(d.as_str()));
    drop(scene);
    assert_eq!(doc.get(&e).and_then(|o| o.str_("name")), Some("Ellipse 1"));
    assert_eq!(doc.get(&d).and_then(|o| o.str_("name")), Some("Diamond 1"));
    let t = text(&doc);
    assert!(t.contains('Λ') && t.contains('V'), "{t}");
    assert!(t.lines().count() >= 5, "{t}");
    // Round trip keeps both shapes.
    let back = Document::from_bytes(&doc.to_monopic(false)).unwrap();
    assert_eq!(text(&back), t);
}

#[test]
fn flood_region_stops_at_borders() {
    use solodraw_core::edit::flood_region;
    let (mut doc, canvas) = Document::new_empty();
    doc.add_rect(&canvas, Rect::new(0, 0, 6, 4), "");
    let buf = solodraw_core::render::render_scene(&Scene::new(&doc, &canvas, true));
    let at = |p: Point| buf.get(p).and_then(|c| c.t.ch);
    let inside = flood_region(Point::new(2, 1), Rect::new(0, 0, 6, 4), at);
    assert_eq!(inside.len(), 4 * 2);
    let border = flood_region(Point::new(1, 0), Rect::new(0, 0, 6, 4), at);
    assert_eq!(border.len(), 4);
}

#[test]
fn line_end_reattach() {
    let (mut doc, canvas) = Document::new_empty();
    let a = doc.add_rect(&canvas, Rect::new(0, 0, 7, 3), "A");
    let b = doc.add_rect(&canvas, Rect::new(14, 0, 7, 3), "B");
    let l = doc.add_line(&canvas, Point::default(), LineEnd::Attached { shape: a.clone(), rect_position: 16 }, LineEnd::Free(Point::new(10, 1)), 1);
    let before = doc.objects.len();
    doc.set_line_end(&l, 1, LineEnd::Free(Point::new(11, 1)), Point::default());
    assert_eq!(doc.objects.len(), before, "free → free reuses the static position");
    doc.set_line_end(&l, 1, LineEnd::Attached { shape: b.clone(), rect_position: 15 }, Point::default());
    assert_eq!(doc.objects.len(), before, "static dropped, rect position added");
    assert_eq!(text(&doc), "┌─────┐       ┌─────┐\n│  A  │──────▶│  B  │\n└─────┘       └─────┘");
    doc.set_line_end(&l, 1, LineEnd::Free(Point::new(10, 5)), Point::default());
    assert_eq!(doc.get(&b).unwrap().refs("position_refs").len(), 1, "only the frame anchor remains");
    let back = Document::from_bytes(&doc.to_monopic(false)).unwrap();
    assert_eq!(text(&back), text(&doc));
}

#[test]
fn second_canvas_round_trips() {
    let (mut doc, first) = Document::new_empty();
    doc.add_rect(&first, Rect::new(0, 0, 5, 3), "A");
    let second = doc.add_canvas("Page 2");
    doc.add_rect(&second, Rect::new(0, 0, 5, 3), "B");
    let back = Document::from_bytes(&doc.to_monopic(false)).unwrap();
    assert_eq!(back.canvases(), vec![first.clone(), second.clone()]);
    let opts = TextOptions::default();
    assert!(canvas_to_text(&back, Some(&first), &opts).unwrap().contains('A'));
    assert!(canvas_to_text(&back, Some(&second), &opts).unwrap().contains('B'));
}

#[test]
fn waypoints_and_labels() {
    let (mut doc, canvas) = Document::new_empty();
    let l = doc.add_line(&canvas, Point::default(), LineEnd::Free(Point::new(0, 0)), LineEnd::Free(Point::new(10, 0)), 1);
    let before = canvas_to_text(&doc, None, &TextOptions::default()).unwrap();
    // A waypoint below the middle bends the line through it.
    assert_eq!(doc.insert_line_point(&l, 0, Point::new(5, 3), Point::default()), Some(1));
    let scene = Scene::new(&doc, &canvas, true);
    assert!(scene.line_path(&l).unwrap().cells.contains(&Point::new(5, 3)));
    drop(scene);
    assert_eq!(doc.get(&l).unwrap().refs("segment_refs").len(), 2);
    // Removing it restores the straight line and leaves no stray objects.
    let n = doc.objects.len();
    assert!(doc.remove_line_point(&l, 1));
    assert_eq!(doc.objects.len(), n - 3);
    assert_eq!(canvas_to_text(&doc, None, &TextOptions::default()).unwrap(), before);
    assert!(!doc.remove_line_point(&l, 0), "ends are not waypoints");

    // A label sits on the middle of the line and follows it.
    let lab = doc.add_line_label(&canvas, &l, "hi").unwrap();
    let f = Scene::new(&doc, &canvas, true).frame(&lab).unwrap();
    assert_eq!((f.y, f.w), (0, 4));
    assert!(canvas_to_text(&doc, None, &TextOptions::default()).unwrap().contains(" hi "));
    doc.move_shape(&lab, Point::new(0, 1));
    assert_eq!(Scene::new(&doc, &canvas, true).frame(&lab).unwrap().y, 1);
    doc.translate_shape(&l, Point::new(0, 2));
    assert_eq!(Scene::new(&doc, &canvas, true).frame(&lab).unwrap().y, 3);
    let back = Document::from_bytes(&doc.to_monopic(false)).unwrap();
    assert_eq!(Scene::new(&back, &canvas, true).frame(&lab).unwrap().y, 3);
}

#[test]
fn line_attaches_to_diamond() {
    let (mut doc, canvas) = Document::new_empty();
    let a = doc.add_rect(&canvas, Rect::new(0, 3, 5, 3), "A");
    let d = doc.add_diamond(&canvas, Point::new(14, 4), 3);
    let anchors = doc.get(&d).unwrap().refs("position_refs").len();

    let scene = Scene::new(&doc, &canvas, true);
    let pts = scene.attach_points(&d);
    assert_eq!(pts, vec![(3, Point::new(14, 0)), (5, Point::new(14, 8)), (7, Point::new(10, 4)), (9, Point::new(18, 4))]);
    assert!(scene.attach_hit(&d, Point::new(14, 4)) && scene.attach_hit(&d, Point::new(10, 4)));
    assert!(!scene.attach_hit(&d, Point::new(11, 1)), "bounding-box corner is outside the diamond");
    drop(scene);

    let l = doc.add_line(&canvas, Point::default(), LineEnd::Attached { shape: a.clone(), rect_position: 16 }, LineEnd::Attached { shape: d.clone(), rect_position: 7 }, 1);
    let pos = doc.get(&d).unwrap().refs("position_refs").last().unwrap().to_string();
    let po = doc.get(&pos).unwrap();
    assert_eq!((po.type_id(), po.int("dpos")), (22, Some(7)));
    let t = text(&doc);
    assert!(t.lines().any(|l| l.contains("│ A │─────▶▕     ▏")), "{t}");

    // Re-attaching to the same point keeps the position; the line follows the diamond.
    doc.set_line_end(&l, 1, LineEnd::Attached { shape: d.clone(), rect_position: 7 }, Point::default());
    assert!(doc.get(&pos).is_some());
    doc.translate_shape(&d, Point::new(0, 4));
    let t = text(&doc);
    let row = |t: &str, pat: &str| t.lines().position(|l| l.contains(pat));
    assert_eq!(row(&t, "▶▕").unwrap(), row(&t, "│ A │").unwrap() + 4, "{t}");
    let back = Document::from_bytes(&doc.to_monopic(false)).unwrap();
    assert_eq!(text(&back), t);

    // Detaching drops the diamond position.
    doc.set_line_end(&l, 1, LineEnd::Free(Point::new(8, 0)), Point::default());
    assert!(doc.get(&pos).is_none());
    assert_eq!(doc.get(&d).unwrap().refs("position_refs").len(), anchors);
}

#[test]
fn click_target_honours_subshape_selection() {
    let (mut doc, canvas) = Document::new_empty();
    let outer = doc.add_group(&canvas);
    let inner = doc.add_group(&outer);
    let leaf = doc.add_rect(&inner, Rect::new(0, 0, 5, 3), "x");
    let loose = doc.add_rect(&canvas, Rect::new(10, 0, 5, 3), "y");
    let pick = |doc: &Document, id: &str| Scene::new(doc, &canvas, true).click_target(id);

    assert_eq!(pick(&doc, &leaf), outer, "groups select as a whole by default");
    assert_eq!(pick(&doc, &loose), loose);
    doc.set_value(&outer, "subshape_selection", serde_json::json!(true));
    assert_eq!(pick(&doc, &leaf), inner, "the flag passes the click one level down");
    doc.set_value(&inner, "subshape_selection", serde_json::json!(true));
    assert_eq!(pick(&doc, &leaf), leaf);
    doc.set_value(&outer, "subshape_selection", serde_json::json!(false));
    assert_eq!(pick(&doc, &leaf), outer, "an inner flag alone does not open the outer group");
}

#[test]
fn image_overlay() {
    use solodraw_core::b64;
    for data in [&b""[..], b"f", b"fo", b"foo", b"foob", b"\x89PNG\r\n\x1a\n\x00\xff"] {
        assert_eq!(b64::decode(&b64::encode(data)).unwrap(), data);
    }
    assert_eq!(b64::encode(b"foob"), "Zm9vYg==");

    let (mut doc, canvas) = Document::new_empty();
    let a = doc.add_rect(&canvas, Rect::new(0, 0, 5, 3), "A");
    let img = doc.add_image(&canvas, b"not really a png", Rect::new(2, 1, 10, 4), 0.5);
    assert_eq!(text(&doc), "┌───┐\n│ A │\n└───┘", "images draw no characters");
    let scene = Scene::new(&doc, &canvas, true);
    assert_eq!(scene.frame(&img), Some(Rect::new(2, 1, 10, 4)));
    assert_eq!(scene.hit_test(Point::new(3, 1)).as_deref(), Some(img.as_str()), "image is on top");
    drop(scene);

    doc.set_value(&img, "cthrough", serde_json::json!(true));
    assert_eq!(Scene::new(&doc, &canvas, true).hit_test(Point::new(3, 1)).as_deref(), Some(a.as_str()), "click-through");

    doc.resize_shape(&img, 6, 2);
    doc.translate_shape(&img, Point::new(1, 1));
    assert_eq!(Scene::new(&doc, &canvas, true).frame(&img), Some(Rect::new(3, 2, 6, 2)));

    let back = Document::from_bytes(&doc.to_monopic(false)).unwrap();
    assert_eq!(back.image_data(&img).unwrap(), b"not really a png");
    assert_eq!(back.get(&img).unwrap().float("img_opacity"), Some(0.5));
    assert_eq!(Scene::new(&back, &canvas, true).frame(&img), Some(Rect::new(3, 2, 6, 2)));
}
