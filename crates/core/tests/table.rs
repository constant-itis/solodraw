use solodraw_core::export::TextOptions;
use solodraw_core::geom::{Point, Rect};
use solodraw_core::table::{cell_key, distribute, split_key, TableGrid};
use solodraw_core::{canvas_to_text, Document, ObjExt, Scene};
use serde_json::json;

fn text(doc: &Document) -> String {
    canvas_to_text(doc, None, &TextOptions { trim_trailing: true, ..Default::default() }).unwrap_or_default()
}

#[test]
fn keys_and_distribution() {
    assert_eq!(cell_key(1, 2), "4294967298");
    assert_eq!(split_key("4294967298"), Some((1, 2)));
    assert_eq!(split_key("-1"), None);
    assert_eq!(split_key("12a"), None);

    // One unit at a time, ties to the lower index.
    assert_eq!(distribute(&[1, 1], 5), vec![3, 2]);
    assert_eq!(distribute(&[1, 1, 1], 7), vec![3, 2, 2]);
    assert_eq!(distribute(&[2, 1], 6), vec![4, 2]);
    assert_eq!(distribute(&[1, 0, 1], 4), vec![2, 0, 2]);
    assert_eq!(distribute(&[0, 0], 3), vec![2, 1], "all-zero ratios split evenly");
    assert_eq!(distribute(&[1, 3], 0), vec![0, 0]);
    for (r, s) in [(&[1u64, 2, 3][..], 17), (&[5, 1, 1, 1][..], 9), (&[1; 9][..], 40)] {
        assert_eq!(distribute(r, s).iter().sum::<i32>(), s);
    }

    // Cells share borders: x = Σ(size + 1), w = size + 2.
    let g = TableGrid::new(Rect::new(10, 5, 7, 5), &[1, 1], &[1, 1]).unwrap();
    assert_eq!(g.cell_rect(0, 0), Rect::new(10, 5, 4, 3));
    assert_eq!(g.cell_rect(1, 1), Rect::new(13, 7, 4, 3));
    assert_eq!(g.content_rect(1, 1), Rect::new(14, 8, 2, 1));
    assert_eq!(g.cell_at(Point::new(11, 6)), Some((0, 0)));
    assert_eq!(g.cell_at(Point::new(13, 6)), Some((0, 1)), "shared border goes to the right cell");
    assert_eq!(g.cell_at(Point::new(16, 9)), Some((1, 1)));
    assert_eq!(g.cell_at(Point::new(17, 9)), None);
    assert!(TableGrid::new(Rect::new(0, 0, 2, 5), &[1], &[1, 1]).is_none(), "too narrow for 2 columns");
}

#[test]
fn renders_with_junctions_and_text() {
    let (mut doc, canvas) = Document::new_empty();
    let t = doc.add_table(&canvas, Rect::new(0, 0, 11, 5), 2, 2);
    assert_eq!(text(&doc), "┌────┬────┐\n│    │    │\n├────┼────┤\n│    │    │\n└────┴────┘");
    doc.set_table_cell_text(&t, 0, 0, "A");
    doc.set_table_cell_text(&t, 1, 1, "Bee");
    assert_eq!(text(&doc), "┌────┬────┐\n│ A  │    │\n├────┼────┤\n│    │Bee │\n└────┴────┘");
    assert_eq!(doc.table_cell_text(&t, 1, 1), "Bee");

    // Borders merge with shapes below (blend mode 0).
    let (mut doc, canvas) = Document::new_empty();
    doc.add_rect(&canvas, Rect::new(4, 0, 5, 3), "");
    doc.add_table(&canvas, Rect::new(0, 2, 7, 3), 1, 2);
    assert_eq!(text(&doc), "    ┌───┐\n    │   │\n┌──┬┴─┬─┘\n│  │  │\n└──┴──┘");

    // Hit test covers the frame.
    let (mut doc, canvas) = Document::new_empty();
    let t = doc.add_table(&canvas, Rect::new(2, 2, 7, 3), 1, 2);
    let scene = Scene::new(&doc, &canvas, true);
    assert_eq!(scene.hit_test(Point::new(4, 3)).as_deref(), Some(t.as_str()));
    assert_eq!(scene.table_grid(&t).unwrap().cols(), 2);
}

#[test]
fn round_trips_and_edits_tracks() {
    let (mut doc, canvas) = Document::new_empty();
    let t = doc.add_table(&canvas, Rect::new(0, 0, 13, 7), 3, 3);
    doc.set_table_cell_text(&t, 0, 0, "a");
    doc.set_table_cell_text(&t, 1, 2, "b");
    doc.set_table_cell_text(&t, 2, 1, "c");

    for minify in [false, true] {
        let back = Document::from_bytes(&doc.to_monopic(minify)).unwrap();
        assert_eq!(text(&back), text(&doc));
        let o = back.get(&t).unwrap();
        assert_eq!(o.int("shape_type"), Some(11));
        assert_eq!(o.val("rows"), Some(&json!([1, 1, 1])));
        let cell = back.table_cell(&t, 1, 2).unwrap();
        assert_eq!(back.get(&cell).unwrap().str_("cell_index"), Some("4294967298"));
        assert_eq!(back.get(&cell).unwrap().type_id(), 29);
    }

    // Insert a row before row 1: the frame grows by one track + border, cells move down.
    let before = doc.objects.len();
    doc.insert_table_track(&canvas, &t, true, 1);
    assert_eq!(doc.table_tracks(&t).unwrap().0, vec![1, 1, 1, 1]);
    assert_eq!(doc.get(&t).unwrap().point("frame_size"), Some(Point::new(13, 9)));
    assert_eq!(doc.table_cell_text(&t, 0, 0), "a");
    assert_eq!(doc.table_cell_text(&t, 2, 2), "b");
    assert_eq!(doc.table_cell_text(&t, 3, 1), "c");
    let cell = doc.table_cell(&t, 2, 2).unwrap();
    assert_eq!(doc.get(&cell).unwrap().str_("cell_index"), Some(cell_key(2, 2).as_str()));

    // Delete column 2: its cell goes, the frame shrinks back.
    doc.delete_table_track(&canvas, &t, false, 2);
    assert_eq!(doc.table_tracks(&t).unwrap().1, vec![1, 1]);
    assert_eq!(doc.get(&t).unwrap().point("frame_size"), Some(Point::new(9, 9)));
    assert_eq!(doc.table_cell(&t, 2, 2), None);
    assert_eq!(doc.objects.len(), before - 2, "cell + text removed");
    doc.delete_table_track(&canvas, &t, true, 0);
    doc.delete_table_track(&canvas, &t, true, 0);
    doc.delete_table_track(&canvas, &t, true, 0);
    assert_eq!(doc.table_tracks(&t).unwrap().0, vec![1], "never deletes the last row");

    // Clearing text removes the cell.
    doc.set_table_cell_text(&t, 0, 1, "c2");
    assert!(doc.table_cell(&t, 0, 1).is_some());
    doc.set_table_cell_text(&t, 0, 1, "");
    assert!(doc.table_cell(&t, 0, 1).is_none());
}

#[test]
fn delete_and_copy_follow_cells() {
    let (mut doc, canvas) = Document::new_empty();
    let base = doc.objects.len();
    let t = doc.add_table(&canvas, Rect::new(0, 0, 9, 5), 2, 2);
    doc.set_table_cell_text(&t, 1, 0, "x");

    let frag = doc.extract_fragment(&canvas, &[t.clone()]);
    let pasted = doc.paste_fragment(&frag, &canvas, Point::default(), Point::new(12, 0)).unwrap();
    let copy = &pasted[0];
    assert_eq!(doc.table_cell_text(copy, 1, 0), "x");
    assert_ne!(doc.table_cell(copy, 1, 0), doc.table_cell(&t, 1, 0), "cells get fresh ids");
    doc.set_table_cell_text(copy, 1, 0, "y");
    assert_eq!(doc.table_cell_text(&t, 1, 0), "x", "copy is independent");

    doc.delete_shape(&canvas, copy);
    doc.delete_shape(&canvas, &t);
    assert_eq!(doc.objects.len(), base, "no orphaned cells or texts");
}

/// A table as Monodraw writes one (full key set, no elision for types 26/29).
#[test]
fn loads_monodraw_table_json() {
    let (doc0, canvas) = Document::new_empty();
    let mut v = doc0.to_json(false);
    let list = v["object_list"].as_array_mut().unwrap();
    let root = list.iter().find(|o| o["type_id"] == 0).unwrap()["object_id"].clone();
    for o in list.iter_mut() {
        if o["object_id"] == json!(canvas) {
            o["subshape_refs"] = json!(["T"]);
        }
    }
    list.push(json!({"object_id": "P", "type_id": 11, "dirset": 0, "point": "1,1"}));
    list.push(json!({
        "object_id": "T", "type_id": 26, "doc_ref": root, "shape_type": 11, "name": "Table 1",
        "frame_size": "10,5", "frame_anchor_ref": "P", "frame_corner": 1, "frame_offset": "0,0",
        "position_refs": ["P"], "subshape_refs": [], "background_enabled": true, "background_type": 2,
        "rows": [1, 1], "cols": [2, 1],
        "cells": {"1": "C1", "4294967296": "C2", "bad-key": "C1"}
    }));
    list.push(json!({"object_id": "C1", "type_id": 29, "cell_index": "1", "text_ref": "X1"}));
    list.push(json!({"object_id": "C2", "type_id": 29, "cell_index": "4294967296", "text_ref": "X2"}));
    for (id, t) in [("X1", "R"), ("X2", "L")] {
        list.push(json!({"object_id": id, "type_id": 27, "text": t, "enabled": true, "alignment": 0, "position": 0}));
    }
    let doc = Document::from_json(v).unwrap();
    // cols [2,1] over interior 10-3 = 7 → [5,2].
    assert_eq!(text(&doc), "\n ┌─────┬──┐\n │     │R │\n ├─────┼──┤\n │L    │  │\n └─────┴──┘");
}
