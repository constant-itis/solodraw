//! Pointer and keyboard interaction on the canvas.

use solodraw_core::edit::flood_region;
use solodraw_core::table::{DEFAULT_TRACKS, TABLE_TYPE};

use crate::prelude::*;

/// Default sizes for a click (no drag) with a creation tool.
const DEFAULT_RECT: (i32, i32) = (12, 5);
const DEFAULT_TEXT: (i32, i32) = (6, 3);
const DEFAULT_RADIUS: i32 = 3;
/// Handle hit radius in pixels.
const HANDLE_HIT: f64 = 6.0;

/// What a pointer press on a selection handle grabs.
enum Handle {
    Frame(i32, i32),
    Radius,
    LineEnd(usize),
}

/// Handles of the single selected shape: (pixel position, handle).
fn handles(area: &gtk::DrawingArea, s: &App) -> Vec<((f64, f64), Handle)> {
    let Some(id) = s.single() else { return vec![] };
    if s.flag(&id, "locked") {
        return vec![];
    }
    let (cw, ch) = cell_size(area, s);
    let off = s.view_offset;
    let px = |x: f64, y: f64| ((x + off.x as f64) * cw, (y + off.y as f64) * ch);
    let centre = |p: Point| px(p.x as f64 + 0.5, p.y as f64 + 0.5);
    let scene = s.scene();
    let Some(o) = scene.obj(&id) else { return vec![] };
    match o.type_id() {
        20 | 25 | 26 | 28 => {
            let Some(f) = scene.frame(&id) else { return vec![] };
            let (x0, y0, x1, y1) = (f.x as f64, f.y as f64, (f.x + f.w) as f64, (f.y + f.h) as f64);
            let mut v = Vec::new();
            for hy in -1..=1 {
                for hx in -1..=1 {
                    if hx == 0 && hy == 0 {
                        continue;
                    }
                    let x = [x0, (x0 + x1) / 2.0, x1][(hx + 1) as usize];
                    let y = [y0, (y0 + y1) / 2.0, y1][(hy + 1) as usize];
                    v.push((px(x, y), Handle::Frame(hx, hy)));
                }
            }
            v
        }
        21 => {
            let Some((c, r)) = scene.diamond_geometry(&id) else { return vec![] };
            [c.offset(0, -r), c.offset(r, 0), c.offset(0, r), c.offset(-r, 0)].into_iter().map(|p| (centre(p), Handle::Radius)).collect()
        }
        5 => o
            .refs("segment_end_refs")
            .iter()
            .enumerate()
            .filter_map(|(i, e)| scene.resolve(e).map(|r| (centre(r.p), Handle::LineEnd(i))))
            .collect(),
        _ => vec![],
    }
}

/// Handle positions for drawing (pixels).
pub fn handle_points(area: &gtk::DrawingArea, s: &App) -> Vec<(f64, f64)> {
    handles(area, s).into_iter().map(|(p, _)| p).collect()
}

fn handle_at(area: &gtk::DrawingArea, s: &App, x: f64, y: f64) -> Option<Handle> {
    handles(area, s)
        .into_iter()
        .filter(|((hx, hy), _)| (hx - x).abs() <= HANDLE_HIT && (hy - y).abs() <= HANDLE_HIT)
        .min_by(|a, b| {
            let d = |((hx, hy), _): &((f64, f64), Handle)| (hx - x).powi(2) + (hy - y).powi(2);
            d(a).total_cmp(&d(b))
        })
        .map(|(_, h)| h)
}

/// Movement guides for a selection moved by `want` (pointer offset since the drag began;
/// `applied` = offset already applied), as Monodraw: on each side of
/// the moved selection, the nearest edge of another visible shape outward from it, if within
/// [`GUIDE_RANGE`] cells. Display only, no snapping (Monodraw doesn't snap either).
const GUIDE_RANGE: i32 = 5;

fn move_guides(s: &App, applied: Point, want: Point) -> Vec<(bool, i32)> {
    let Some(f) = s.selection_frame() else { return vec![] };
    let f = Rect::new(f.x - applied.x + want.x, f.y - applied.y + want.y, f.w, f.h);
    let scene = s.scene();
    let moving = s.selection_roots();
    let inside = |id: &str| {
        let mut cur = Some(id);
        while let Some(c) = cur {
            if moving.iter().any(|m| m == c) {
                return true;
            }
            cur = scene.parent(c);
        }
        false
    };
    let others: Vec<Rect> = s
        .doc
        .get(&s.canvas)
        .map(|c| c.refs("subshape_refs").into_iter().map(str::to_string).collect::<Vec<_>>())
        .unwrap_or_default()
        .iter()
        .filter(|id| !inside(id) && !s.flag(id, "hidden"))
        .filter_map(|id| scene.frame(id))
        .collect();
    let mut guides = Vec::new();
    for vertical in [true, false] {
        let (lo, hi) = if vertical { (f.x, f.x1()) } else { (f.y, f.y1()) };
        let edges: Vec<i32> = others.iter().flat_map(|o| if vertical { [o.x, o.x1()] } else { [o.y, o.y1()] }).collect();
        // Nearest edge at or before the low side, and at or after the high side.
        if let Some(e) = edges.iter().copied().filter(|&e| e <= lo && lo - e <= GUIDE_RANGE).max() {
            guides.push((vertical, e));
        }
        if let Some(e) = edges.iter().copied().filter(|&e| e >= hi && e - hi <= GUIDE_RANGE).min() {
            guides.push((vertical, e));
        }
    }
    guides.sort();
    guides.dedup();
    guides
}

/// Topmost rect/ellipse/diamond under `p` (or on one of its outside attachment points) that a
/// line end can attach to.
fn attach_target(s: &App, p: Point, exclude: &str) -> Option<String> {
    let scene = s.scene();
    scene
        .draw_order()
        .into_iter()
        .rev()
        .filter(|id| id != exclude && !s.flag(id, "locked"))
        .filter(|id| scene.obj(id).is_some_and(|o| matches!(o.type_id(), 20 | 21 | 28) && o.boolean("attachable")))
        .find(|id| scene.attach_hit(id, p))
}

/// Where a line end dropped at `p` attaches: the attachment point under the pointer if it is on
/// one, else the side (diamond tip) facing `toward`, the line's other end. Returns (shape, its
/// attachment points, chosen position).
pub fn attach_choice(s: &App, p: Point, toward: Point, exclude: &str) -> Option<(String, Vec<(i64, Point)>, i64)> {
    let id = attach_target(s, p, exclude)?;
    let scene = s.scene();
    let points = scene.attach_points(&id);
    let dist = |q: Point| (q.x - toward.x).pow(2) + (q.y - toward.y).pow(2);
    let pos = points
        .iter()
        .find(|&&(_, q)| q == p)
        .or(scene.attach_side_points(&id).iter().min_by_key(|&&(_, q)| dist(q)))
        .map(|&(k, _)| k)?;
    Some((id, points, pos))
}

/// The neighbouring point of end `index` of `line` (what the end's segment heads toward).
pub fn line_other_end(s: &App, line: &str, index: usize, cur: Point) -> Point {
    let scene = s.scene();
    let ends = scene.obj(line).map(|o| o.refs("segment_end_refs").into_iter().map(str::to_string).collect::<Vec<_>>()).unwrap_or_default();
    let n = if index == 0 { 1 } else { index - 1 };
    ends.get(n).and_then(|e| scene.resolve(e)).map(|r| r.p).unwrap_or(cur)
}

/// Whether end `index` is the first or last end of `line` (waypoints never attach).
pub fn is_outer_end(s: &App, line: &str, index: usize) -> bool {
    index == 0 || s.doc.get(line).is_some_and(|o| index + 1 == o.refs("segment_end_refs").len())
}

/// Shape that receives pencil / eraser / bucket cells at `p`: the rect or diamond under the
/// pointer, else the selected one.
fn paint_target(s: &App, p: Point) -> Option<String> {
    if let Some(e) = &s.edit_shape {
        return Some(e.clone());
    }
    let paintable = |id: &str| s.doc.get(id).is_some_and(|o| matches!(o.type_id(), 20 | 21)) && !s.flag(id, "locked");
    let scene = s.scene();
    scene.hit_test(p).filter(|h| paintable(h)).or_else(|| s.primary().filter(|h| paintable(h)))
}

pub fn drag_begin(ui: &U, st: &St, x: f64, y: f64, shift: bool, button: u32) {
    let mut s = st.borrow_mut();
    let p = to_cell(&ui.area, &s, x, y);
    if s.tool != Tool::Pencil {
        s.cursor = None;
    }
    if button == 2 || s.tool == Tool::Pan || s.space_down {
        let (h, v) = (ui.scroller.hadjustment(), ui.scroller.vadjustment());
        s.drag = Some(Drag::Pan { start: (x, y), scroll0: (h.value(), v.value()) });
        if s.space_down {
            s.space_used = true;
        }
        return;
    }
    if button == 3 && s.tool != Tool::Pencil {
        return;
    }
    match s.tool {
        Tool::Select => {
            match handle_at(&ui.area, &s, x, y) {
                Some(Handle::Frame(hx, hy)) => {
                    let origin = s.selection_frame().unwrap_or_default();
                    s.checkpoint();
                    s.drag = Some(Drag::Resize { origin, hx, hy });
                    return;
                }
                Some(Handle::Radius) => {
                    let id = s.primary().unwrap_or_default();
                    let Some((centre, _)) = s.scene().diamond_geometry(&id) else { return };
                    s.checkpoint();
                    s.drag = Some(Drag::Radius { centre });
                    return;
                }
                Some(Handle::LineEnd(index)) => {
                    let line = s.primary().unwrap_or_default();
                    s.checkpoint();
                    s.drag = Some(Drag::LineEnd { line, index, cur: p });
                    return;
                }
                None => {}
            }
            let scene = s.scene();
            let hit = s.hit_unlocked(p);
            let target = hit.map(|h| scene.click_target(&h));
            drop(scene);
            let before = s.selection.clone();
            match target {
                Some(t) if shift => {
                    s.toggle_selected(&t);
                    if s.is_selected(&t) {
                        s.drag = Some(Drag::Move { start: p, applied: Point::default(), moved: false });
                    }
                }
                Some(t) => {
                    if !s.is_selected(&t) {
                        s.select_only(Some(t));
                    } else {
                        // Keep a multi-selection so it can be dragged; make `t` primary.
                        s.selection.retain(|x| *x != t);
                        s.selection.push(t);
                    }
                    s.drag = Some(Drag::Move { start: p, applied: Point::default(), moved: false });
                }
                None => {
                    let base = if shift { s.selection.clone() } else { vec![] };
                    s.selection = base.clone();
                    s.drag = Some(Drag::Marquee { start: p, cur: p, base });
                }
            }
            let changed = s.selection != before;
            drop(s);
            if changed {
                sync(ui, st);
            }
        }
        Tool::Rect | Tool::Table | Tool::Text | Tool::Surface | Tool::Ellipse => {
            s.drag = Some(Drag::Create { start: p, cur: p });
            drop(s);
        }
        Tool::Diamond => {
            s.drag = Some(Drag::Diamond { centre: p, cur: p });
            drop(s);
        }
        Tool::Line => {
            s.drag = Some(Drag::Line { start: p, cur: p });
            drop(s);
        }
        Tool::Pencil | Tool::Eraser => {
            let erase = s.tool == Tool::Eraser || button == 3;
            s.checkpoint();
            let id = match paint_target(&s, p) {
                Some(t) => t,
                None if erase => {
                    s.undo.pop();
                    return;
                }
                None => {
                    // A borderless, transparent surface to draw on.
                    let (parent, _) = s.insert_parent();
                    let id = s.doc.add_rect_typed(&parent, Rect::new(p.x, p.y, 1, 1), "", 5);
                    s.doc.set_value(&id, "rectangular_border_enabled", json!(false));
                    s.doc.set_value(&id, "background_enabled", json!(false));
                    id
                }
            };
            let c = if erase { None } else { Some(s.pencil_char) };
            put_cell(&mut s, &id, p, c);
            if s.tool == Tool::Pencil {
                s.cursor = Some((id.clone(), p.offset(if c.is_some() { 1 } else { 0 }, 0), p.x));
            }
            let changed = s.selection != [id.clone()];
            s.select_only(Some(id.clone()));
            s.drag = Some(Drag::Paint { shape: id, c, last: p });
            s.rerender();
            drop(s);
            if changed {
                sync(ui, st);
            }
        }
        Tool::Bucket => {
            let Some(id) = paint_target(&s, p) else { return };
            let Some(bounds) = s.scene().frame(&id) else { return };
            let region = flood_region(p, bounds, |q| s.buffer.get(q).and_then(|c| c.t.ch));
            if region.is_empty() {
                return;
            }
            s.checkpoint();
            let c = s.pencil_char;
            for q in region {
                put_cell(&mut s, &id, q, Some(c));
            }
            s.select_only(Some(id));
            s.rerender();
            drop(s);
            sync(ui, st);
        }
        Tool::Picker => {
            if let Some(c) = s.buffer.get(p).and_then(|c| c.t.ch).filter(|c| *c != ' ') {
                s.pencil_char = c;
            }
            drop(s);
        }
        Tool::Pan => drop(s),
    }
    refresh(ui, &st.borrow());
}

pub fn drag_update(ui: &U, st: &St, x: f64, y: f64) {
    let mut s = st.borrow_mut();
    let p = to_cell(&ui.area, &s, x, y);
    match s.drag.clone() {
        Some(Drag::Move { start, applied, moved }) => {
            let total = p.sub(start);
            s.guides = if crate::prefs::get_bool(crate::prefs::GUIDES, false) { move_guides(&s, applied, total) } else { vec![] };
            let d = total.sub(applied);
            if d != Point::default() {
                if !moved {
                    s.checkpoint();
                }
                for sel in s.selection_roots() {
                    s.doc.move_shape(&sel, d);
                }
                s.drag = Some(Drag::Move { start, applied: total, moved: true });
                s.rerender();
            }
        }
        Some(Drag::Resize { origin, hx, hy }) => {
            if let Some(sel) = s.single() {
                let (mut x0, mut y0, mut x1, mut y1) = (origin.x, origin.y, origin.x1(), origin.y1());
                match hx {
                    -1 => x0 = p.x.min(x1 - 1),
                    1 => x1 = p.x.max(x0 + 1),
                    _ => {}
                }
                match hy {
                    -1 => y0 = p.y.min(y1),
                    1 => y1 = p.y.max(y0),
                    _ => {}
                }
                let target = Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1);
                s.doc.resize_shape(&sel, target.w, target.h);
                if let Some(f) = s.scene().frame(&sel) {
                    s.doc.move_shape(&sel, Point::new(target.x - f.x, target.y - f.y));
                }
                s.rerender();
            }
        }
        Some(Drag::Radius { centre }) => {
            if let Some(sel) = s.single() {
                let r = (p.x - centre.x).abs().max((p.y - centre.y).abs()).max(1);
                s.doc.set_value(&sel, "radius", json!(r));
                s.rerender();
            }
        }
        Some(Drag::LineEnd { line, index, .. }) => {
            let t = s.scene().translation(&line);
            s.doc.set_line_end(&line, index, LineEnd::Free(p), t);
            s.drag = Some(Drag::LineEnd { line, index, cur: p });
            s.rerender();
        }
        Some(Drag::Create { start, .. }) => s.drag = Some(Drag::Create { start, cur: p }),
        Some(Drag::Diamond { centre, .. }) => s.drag = Some(Drag::Diamond { centre, cur: p }),
        Some(Drag::Line { start, .. }) => s.drag = Some(Drag::Line { start, cur: p }),
        Some(Drag::Marquee { start, base, .. }) => {
            let r = Rect::from_points(start, p);
            let scene = s.scene();
            let mut sel = base.clone();
            let kids: Vec<String> = s.doc.get(&s.canvas).map(|c| c.refs("subshape_refs").into_iter().map(str::to_string).collect()).unwrap_or_default();
            // A shape is hit when its frame or its shadow intersects the marquee; a hit flips
            // its state from the drag start, so Shift+marquee toggles (as Monodraw).
            for k in kids.iter().rev() {
                if s.flag(k, "locked") || s.flag(k, "hidden") {
                    continue;
                }
                let Some(f) = scene.frame(k) else { continue };
                let o = s.doc.get(k);
                let shadow = o.filter(|o| o.boolean("shadow_enabled")).map(|o| f.translate(o.point("shadow_offset").unwrap_or_default()));
                if f.intersects(&r) || shadow.is_some_and(|sh| sh.intersects(&r)) {
                    if let Some(i) = sel.iter().position(|x| x == k) {
                        sel.remove(i);
                    } else {
                        sel.push(k.clone());
                    }
                }
            }
            drop(scene);
            let changed = sel != s.selection;
            s.selection = sel;
            s.drag = Some(Drag::Marquee { start, cur: p, base });
            if changed {
                drop(s);
                sync(ui, st);
                return;
            }
        }
        Some(Drag::Pan { start, scroll0 }) => {
            let (h, v) = (ui.scroller.hadjustment(), ui.scroller.vadjustment());
            // The canvas moves under the pointer while scrolling: subtract the scroll so far.
            let sx = (x - start.0) - (h.value() - scroll0.0);
            let sy = (y - start.1) - (v.value() - scroll0.1);
            h.set_value(scroll0.0 - sx);
            v.set_value(scroll0.1 - sy);
            return;
        }
        Some(Drag::Paint { shape, c, last }) => {
            if p != last {
                for q in cells_between(last, p).into_iter().skip(1) {
                    put_cell(&mut s, &shape, q, c);
                }
                if let Some((id, _, home)) = s.cursor.clone() {
                    s.cursor = Some((id, p.offset(if c.is_some() { 1 } else { 0 }, 0), home));
                }
                s.drag = Some(Drag::Paint { shape, c, last: p });
                s.rerender();
            }
        }
        None => {}
    }
    refresh(ui, &s);
}

/// Cells on the straight path from `a` to `b`, both included (Bresenham).
fn cells_between(a: Point, b: Point) -> Vec<Point> {
    let (dx, dy) = ((b.x - a.x).abs(), -(b.y - a.y).abs());
    let (sx, sy) = ((b.x - a.x).signum(), (b.y - a.y).signum());
    let (mut x, mut y, mut err) = (a.x, a.y, dx + dy);
    let mut out = vec![a];
    while (x, y) != (b.x, b.y) {
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
        out.push(Point::new(x, y));
    }
    out
}

pub fn drag_end(ui: &U, st: &St) {
    let mut s = st.borrow_mut();
    let mut created = false;
    let mut edit_text = None;
    let tool = s.tool;
    let drag = s.drag.take();
    s.guides.clear();
    // "Apply selection shape style on create" copies from the primary selection as it was.
    let style_src = s.primary().filter(|_| crate::prefs::get_bool(crate::prefs::APPLY_STYLE_ON_CREATE, false));
    // Geometry edits change the inspector's values.
    let edited = matches!(drag, Some(Drag::Resize { .. } | Drag::Radius { .. } | Drag::LineEnd { .. } | Drag::Move { moved: true, .. }));
    match drag {
        Some(Drag::Create { start, cur }) => {
            let dragged = Rect::from_points(start, cur);
            let clicked = start == cur;
            let (dw, dh) = if tool == Tool::Text { DEFAULT_TEXT } else { DEFAULT_RECT };
            let r = if clicked { Rect::new(start.x, start.y, dw, dh) } else { dragged };
            let r = if tool == Tool::Text { Rect::new(r.x, r.y, r.w.max(DEFAULT_TEXT.0), r.h.max(DEFAULT_TEXT.1)) } else { r };
            if r.w >= 2 && r.h >= 2 {
                s.checkpoint();
                let (parent, t) = s.insert_parent();
                let r = r.translate(Point::new(-t.x, -t.y));
                let id = match tool {
                    Tool::Ellipse => s.doc.add_ellipse(&parent, r),
                    Tool::Table => {
                        // Monodraw's 9 × 9, fewer tracks when the drag is too small for that.
                        let fit = |size: i32| ((size - 1) / 2).clamp(1, DEFAULT_TRACKS as i32) as usize;
                        s.doc.add_table(&parent, r, fit(r.h), fit(r.w))
                    }
                    Tool::Surface => {
                        let id = s.doc.add_rect_typed(&parent, r, "", 5);
                        s.doc.set_value(&id, "rectangular_border_enabled", json!(false));
                        s.doc.set_value(&id, "background_enabled", json!(false));
                        id
                    }
                    Tool::Text => {
                        let id = s.doc.add_rect_typed(&parent, r, "Text", 4);
                        edit_text = Some(id.clone());
                        id
                    }
                    _ => s.doc.add_rect(&parent, r, ""),
                };
                if let Some(src) = &style_src {
                s.doc.apply_create_style(src, &id);
            }
            s.select_only(Some(id));
                s.rerender();
                created = true;
            }
        }
        Some(Drag::Diamond { centre, cur }) => {
            let r = if centre == cur { DEFAULT_RADIUS } else { (cur.x - centre.x).abs().max((cur.y - centre.y).abs()).max(1) };
            s.checkpoint();
            let (parent, t) = s.insert_parent();
            let id = s.doc.add_diamond(&parent, centre.sub(t), r);
            if let Some(src) = &style_src {
                s.doc.apply_create_style(src, &id);
            }
            s.select_only(Some(id));
            s.rerender();
            created = true;
        }
        Some(Drag::Line { start, cur }) if start != cur => {
            s.checkpoint();
            let ends = [(start, cur), (cur, start)].map(|(p, toward)| match attach_choice(&s, p, toward, "") {
                Some((h, _, pos)) => LineEnd::Attached { shape: h, rect_position: pos },
                None => LineEnd::Free(p),
            });
            let [a, b] = ends;
            let (parent, t) = s.insert_parent();
            let id = s.doc.add_line(&parent, t, a, b, 1);
            if let Some(src) = &style_src {
                s.doc.apply_create_style(src, &id);
            }
            s.select_only(Some(id));
            s.rerender();
            created = true;
        }
        Some(Drag::LineEnd { line, index, cur }) => {
            // Drop onto a shape attaches to the attachment point under the pointer, else the
            // side facing the other end.
            let other = line_other_end(&s, &line, index, cur);
            if let Some((h, _, pos)) = attach_choice(&s, cur, other, &line).filter(|_| is_outer_end(&s, &line, index)) {
                let t = s.scene().translation(&line);
                s.doc.set_line_end(&line, index, LineEnd::Attached { shape: h, rect_position: pos }, t);
                s.rerender();
            }
        }
        _ => {}
    }
    let revert = created && !s.sticky;
    drop(s);
    if created || edited {
        sync(ui, st);
    }
    if revert {
        set_tool(ui, st, Tool::Select);
    }
    if let Some(id) = edit_text {
        edit_text_dialog(ui, st, &id);
    }
    refresh(ui, &st.borrow());
}

// ---------------------------------------------------------------------------
// Keyboard

pub fn key_pressed(ui: &U, st: &St, key: gdk::Key, state: gdk::ModifierType) -> glib::Propagation {
    let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
    let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
    // Ignore keys while a text entry in the inspector has focus.
    if let Some(focus) = gtk::prelude::GtkWindowExt::focus(&ui.window) {
        if focus.is::<gtk::Text>() || focus.is::<gtk::TextView>() || focus.is::<gtk::SpinButton>() {
            return glib::Propagation::Proceed;
        }
    }
    if ctrl || state.contains(gdk::ModifierType::ALT_MASK) {
        // Menu accelerators handle modified keys.
        return glib::Propagation::Proceed;
    }

    // Pencil typing.
    let in_pencil = st.borrow().cursor.is_some();
    if in_pencil {
        let mut s = st.borrow_mut();
        let Some((shape, p, home)) = s.cursor.clone() else { return glib::Propagation::Proceed };
        let mut np = p;
        match key {
            gdk::Key::Escape => {
                s.cursor = None;
            }
            gdk::Key::Left => np.x -= 1,
            gdk::Key::Right => np.x += 1,
            gdk::Key::Up => np.y -= 1,
            gdk::Key::Down => np.y += 1,
            gdk::Key::Return | gdk::Key::KP_Enter => np = Point::new(home, p.y + 1),
            gdk::Key::BackSpace | gdk::Key::Delete => {
                if key == gdk::Key::BackSpace {
                    np.x -= 1;
                }
                s.checkpoint();
                put_cell(&mut s, &shape, np, None);
            }
            _ => match key.to_unicode().filter(|c| !c.is_control()) {
                Some(c) => {
                    s.checkpoint();
                    put_cell(&mut s, &shape, p, Some(c));
                    np.x += 1;
                }
                None => return glib::Propagation::Proceed,
            },
        }
        if s.cursor.is_some() {
            s.cursor = Some((shape, np, home));
        }
        s.rerender();
        refresh(ui, &s);
        return glib::Propagation::Stop;
    }

    if key == gdk::Key::space {
        space_pressed(ui, st);
        return glib::Propagation::Stop;
    }

    let mut s = st.borrow_mut();
    let arrow = match key {
        gdk::Key::Left => Some(Point::new(-1, 0)),
        gdk::Key::Right => Some(Point::new(1, 0)),
        gdk::Key::Up => Some(Point::new(0, -1)),
        gdk::Key::Down => Some(Point::new(0, 1)),
        _ => None,
    };
    if let (Some(d), false) = (arrow, s.selection.is_empty()) {
        s.checkpoint();
        let d = if shift { Point::new(d.x * 8, d.y * 4) } else { d };
        for sel in s.selection_roots() {
            s.doc.move_shape(&sel, d);
        }
        s.rerender();
        refresh(ui, &s);
        return glib::Propagation::Stop;
    }
    let tool = if key == gdk::Key::Escape {
        Some(Tool::Select)
    } else {
        key.to_unicode().and_then(|c| {
            let c = c.to_ascii_lowercase();
            Tool::ALL.iter().find(|t| t.3 == c && t.0.available()).map(|t| t.0)
        })
    };
    if let Some(t) = tool {
        drop(s);
        // Shift+key makes the tool sticky.
        choose_tool(ui, st, t, shift && t != Tool::Select);
        return glib::Propagation::Stop;
    }
    match key {
        gdk::Key::Delete | gdk::Key::BackSpace => {
            if !s.selection.is_empty() {
                drop(s);
                delete_selection(ui, st);
                return glib::Propagation::Stop;
            }
        }
        gdk::Key::Return => {
            if let Some(sel) = s.single() {
                if s.doc.text_of(&sel).is_some() {
                    drop(s);
                    edit_text_dialog(ui, st, &sel);
                    return glib::Propagation::Stop;
                }
            }
        }
        _ => {}
    }
    glib::Propagation::Proceed
}

pub fn key_released(ui: &U, st: &St, key: gdk::Key) {
    if key != gdk::Key::space {
        return;
    }
    let back = {
        let mut s = st.borrow_mut();
        if !s.space_down {
            return;
        }
        s.space_down = false;
        // Held and dragged: back to the previous tool. Tapped: Pan stays until the next Space.
        if s.space_used {
            s.pan_return.take()
        } else {
            None
        }
    };
    if let Some(t) = back {
        set_tool(ui, st, t);
    }
}

/// Space: press toggles Pan; holding it pans temporarily (see `key_released`).
fn space_pressed(ui: &U, st: &St) {
    let next = {
        let mut s = st.borrow_mut();
        if s.space_down {
            return; // key repeat
        }
        if s.tool == Tool::Pan {
            s.pan_return.take()
        } else {
            s.space_down = true;
            s.space_used = false;
            s.pan_return = Some(s.tool);
            Some(Tool::Pan)
        }
    };
    if let Some(t) = next {
        set_tool(ui, st, t);
    }
}

/// Write a raw character into `shape`'s content tixmap at canvas cell `p`, growing a
/// surface's frame to cover it.
pub fn put_cell(s: &mut App, shape: &str, p: Point, c: Option<char>) {
    let Some(f) = s.scene().frame(shape) else { return };
    let Some(origin) = s.scene().content_origin(shape) else { return };
    let local = p.sub(origin);
    s.doc.set_tixmap_cell(shape, "content_tixels", local, c);
    let is_surface = s.doc.get(shape).is_some_and(|o| o.int_or("shape_type", 1) == 5);
    if is_surface && c.is_some() && !f.contains(p) && local.x >= 0 && local.y >= 0 {
        s.doc.resize_shape(shape, f.w.max(local.x + 1), f.h.max(local.y + 1));
    }
}

/// A tool picked by the user (toolbar or shortcut); `sticky` keeps it after creating a shape.
pub fn choose_tool(ui: &U, st: &St, t: Tool, sticky: bool) {
    {
        let mut s = st.borrow_mut();
        s.pan_return = None;
        s.space_down = false;
    }
    set_tool(ui, st, t);
    st.borrow_mut().sticky = sticky;
}

pub fn set_tool(ui: &U, st: &St, t: Tool) {
    {
        let mut s = st.borrow_mut();
        s.tool = t;
        if !matches!(t, Tool::Pencil | Tool::Eraser | Tool::Bucket | Tool::Picker | Tool::Pan) && s.edit_shape.is_some() {
            s.stop_editing();
        }
        s.sticky = false;
        s.drag = None;
        if t != Tool::Pencil {
            s.cursor = None;
        }
    }
    if t == Tool::Pencil && crate::prefs::get_bool(crate::prefs::CHARS_FOR_PENCIL, false) {
        ui.assistant_pane.set_visible(true);
        ui.assistant.set_visible_child_name("characters");
    }
    for (tool, btn) in &ui.tools {
        if *tool == t && !btn.is_active() {
            btn.set_active(true);
        }
    }
    let cursor = match t {
        Tool::Pan => "grab",
        Tool::Select => "default",
        Tool::Text => "text",
        _ => "crosshair",
    };
    ui.area.set_cursor_from_name(Some(cursor));
    refresh(ui, &st.borrow());
}

/// Shape ▸ Edit: toggle editing the selected shape's custom content. While editing, Pencil,
/// Eraser, Bucket Fill and Picker work on that shape wherever you click.
pub fn toggle_edit_mode(ui: &U, st: &St) {
    let start = {
        let mut s = st.borrow_mut();
        if s.edit_shape.is_some() {
            s.stop_editing();
            None
        } else {
            s.primary().filter(|p| s.doc.get(p).is_some_and(|o| matches!(o.type_id(), 20 | 21)) && !s.flag(p, "locked"))
        }
    };
    match start {
        Some(id) => {
            set_tool(ui, st, Tool::Pencil);
            let mut s = st.borrow_mut();
            s.start_editing(id.clone());
            s.select_only(Some(id));
        }
        None => set_tool(ui, st, Tool::Select),
    }
    sync(ui, st);
}

// ---------------------------------------------------------------------------
// Canvas context menu and line commands (docs/UI_INVENTORY.md §1.3)

/// Right-click with the Select tool: select what is under the pointer and show its menu.
pub fn canvas_context_menu(ui: &U, st: &St, x: f64, y: f64) {
    let is_line = {
        let mut s = st.borrow_mut();
        if s.tool != Tool::Select {
            return;
        }
        let p = to_cell(&ui.area, &s, x, y);
        s.context_point = Some(p);
        let hit = s.hit_unlocked(p).map(|h| s.scene().click_target(&h));
        if let Some(h) = &hit {
            if !s.is_selected(h) {
                s.select_only(Some(h.clone()));
            }
        }
        hit.is_some_and(|h| s.doc.get(&h).is_some_and(|o| o.type_id() == 5))
    };
    let table = context_cell(&st.borrow()).filter(|(id, _, _)| !st.borrow().flag(id, "locked")).map(|(id, _, _)| id);
    sync(ui, st);
    let m = gtk::gio::Menu::new();
    if is_line {
        let mut items = vec![("Add Midpoint", "win.add-midpoint")];
        if waypoint_near(&st.borrow()).is_some() {
            items.push(("Delete Midpoint", "win.delete-midpoint"));
        }
        items.push(("Add Label", "win.add-label"));
        m.append_section(None, &menu_section(&items));
    }
    if let Some(id) = table {
        let (rows, cols) = st.borrow().doc.table_tracks(&id).unwrap_or_default();
        let mut items = vec![
            ("Edit Cell Text…", "win.table-edit-cell"),
            ("Insert Row Above", "win.table-row-above"),
            ("Insert Row Below", "win.table-row-below"),
            ("Insert Column Left", "win.table-col-left"),
            ("Insert Column Right", "win.table-col-right"),
        ];
        if rows.len() > 1 {
            items.push(("Delete Row", "win.table-delete-row"));
        }
        if cols.len() > 1 {
            items.push(("Delete Column", "win.table-delete-col"));
        }
        m.append_section(None, &menu_section(&items));
    }
    if st.borrow().selection.is_empty() {
        m.append_section(None, &menu_section(&[("Paste", "win.paste-here"), ("Select All", "win.select-all")]));
    } else {
        let shapes = shape_menu();
        for i in 0..shapes.n_items() {
            if let Some(sec) = shapes.item_link(i, gtk::gio::MENU_LINK_SECTION) {
                m.append_section(None, &sec);
            }
        }
    }
    popup_menu(&ui.area, &m, x, y);
}

/// The selected line and the waypoint (interior end) within a cell of the right-click point.
fn waypoint_near(s: &App) -> Option<(String, usize)> {
    let line = s.single().filter(|l| s.doc.get(l).is_some_and(|o| o.type_id() == 5))?;
    let p = s.context_point?;
    let scene = s.scene();
    let ends: Vec<String> = scene.obj(&line)?.refs("segment_end_refs").into_iter().map(str::to_string).collect();
    let n = ends.len();
    (1..n.saturating_sub(1))
        .find(|&i| scene.resolve(&ends[i]).is_some_and(|r| (r.p.x - p.x).abs() <= 1 && (r.p.y - p.y).abs() <= 1))
        .map(|i| (line.clone(), i))
}

/// Add a waypoint to the selected line at the right-click point, in the segment nearest to it.
pub fn add_midpoint(ui: &U, st: &St) {
    {
        let mut s = st.borrow_mut();
        let (Some(line), Some(p)) = (s.single().filter(|l| s.doc.get(l).is_some_and(|o| o.type_id() == 5)), s.context_point) else { return };
        let pts: Vec<Point> = {
            let scene = s.scene();
            scene.obj(&line).map(|o| o.refs("segment_end_refs").iter().filter_map(|e| scene.resolve(e).map(|r| r.p)).collect()).unwrap_or_default()
        };
        if pts.len() < 2 {
            return;
        }
        // Distance from p to the box spanned by each segment's ends.
        let dist = |a: Point, b: Point| {
            let dx = (a.x.min(b.x) - p.x).max(p.x - a.x.max(b.x)).max(0);
            let dy = (a.y.min(b.y) - p.y).max(p.y - a.y.max(b.y)).max(0);
            dx * dx + dy * dy
        };
        let seg = (0..pts.len() - 1).min_by_key(|&i| dist(pts[i], pts[i + 1])).unwrap_or(0);
        s.checkpoint();
        let t = s.scene().translation(&line);
        let was_diagram = s.doc.get(&line).is_some_and(|o| o.int_or("line_type", 3) == 3);
        s.doc.insert_line_point(&line, seg, p, t);
        s.rerender();
        drop(s);
        if was_diagram {
            ui.status.set_text("Midpoint added; the line is now Automatic so it passes through it");
        }
    }
    sync(ui, st);
}

pub fn delete_midpoint(ui: &U, st: &St) {
    {
        let mut s = st.borrow_mut();
        let Some((line, i)) = waypoint_near(&s) else { return };
        s.checkpoint();
        s.doc.remove_line_point(&line, i);
        s.rerender();
    }
    sync(ui, st);
}

/// Add a text label on the selected line and edit its text.
pub fn add_label(ui: &U, st: &St) {
    let label = {
        let mut s = st.borrow_mut();
        let Some(line) = s.single().filter(|l| s.doc.get(l).is_some_and(|o| o.type_id() == 5)) else { return };
        let (parent, _) = s.insert_parent();
        s.checkpoint();
        let label = s.doc.add_line_label(&parent, &line, "Label");
        s.select_only(label.clone());
        s.rerender();
        label
    };
    sync(ui, st);
    if let Some(id) = label {
        edit_text_dialog(ui, st, &id);
    }
}

// ---------------------------------------------------------------------------
// Tables

/// The selected table and the cell under (x, y), if any.
fn table_cell_under(ui: &U, s: &App, x: f64, y: f64) -> Option<(String, usize, usize)> {
    let id = s.single().filter(|id| s.doc.get(id).is_some_and(|o| o.type_id() == TABLE_TYPE))?;
    let p = to_cell(&ui.area, s, x, y);
    let (r, c) = s.scene().table_grid(&id)?.cell_at(p)?;
    Some((id, r, c))
}

/// Double-click on a selected table: edit the text of the cell under the pointer.
pub fn edit_table_cell_at(ui: &U, st: &St, x: f64, y: f64) -> bool {
    let hit = {
        let s = st.borrow();
        if s.single().is_some_and(|id| s.flag(&id, "locked")) {
            return false;
        }
        table_cell_under(ui, &s, x, y)
    };
    let Some((id, r, c)) = hit else { return false };
    crate::textedit::edit_cell_dialog(ui, st, &id, r, c);
    true
}

/// The selected table and the cell at the context-menu point.
fn context_cell(s: &App) -> Option<(String, usize, usize)> {
    let id = s.single().filter(|id| s.doc.get(id).is_some_and(|o| o.type_id() == TABLE_TYPE))?;
    let (r, c) = s.scene().table_grid(&id)?.cell_at(s.context_point?)?;
    Some((id, r, c))
}

/// Table context-menu edits: insert (offset 0 before, 1 after) or delete a row or column at
/// the right-clicked cell.
pub fn table_track(ui: &U, st: &St, rows: bool, insert: Option<usize>) {
    {
        let mut s = st.borrow_mut();
        let Some((id, r, c)) = context_cell(&s) else { return };
        let at = if rows { r } else { c };
        s.checkpoint();
        let canvas = s.canvas.clone();
        match insert {
            Some(after) => s.doc.insert_table_track(&canvas, &id, rows, at + after),
            None => s.doc.delete_table_track(&canvas, &id, rows, at),
        }
        s.rerender();
    }
    sync(ui, st);
    refresh(ui, &st.borrow());
}
