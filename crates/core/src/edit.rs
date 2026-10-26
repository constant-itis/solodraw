//! Document construction and editing primitives (used by the editor and by tests).

use serde_json::{json, Value};

use crate::format::{new_id, Document, Obj, ObjExt};
use crate::geom::{format_point, Point, Rect};

/// Shape kinds with their `shape_type` and display name (SPEC §1.5).
pub fn shape_display_name(shape_type: i64) -> &'static str {
    match shape_type {
        1 => "Rectangle",
        2 => "Line",
        3 => "Triangle",
        4 => "Text",
        5 => "Surface",
        6 => "Canvas",
        8 => "Group",
        9 => "Diamond",
        10 => "Image",
        11 => "Table",
        12 => "Ellipse",
        _ => "Shape",
    }
}

impl Document {
    /// A new empty document with one canvas. Returns (document, canvas id).
    pub fn new_empty() -> (Document, String) {
        let mut doc = Document::default();
        let root = new_id();
        let canvas = new_id();
        let mut r = Obj::new();
        r.insert("object_id".into(), json!(root));
        r.insert("type_id".into(), json!(0));
        r.insert("alphabet".into(), json!(1));
        r.insert("canvas_refs".into(), json!([canvas]));
        r.insert("shape_seq_numbers".into(), json!({}));
        doc.objects.insert(root.clone(), Value::Object(r));
        let mut c = doc.base_shape(1, 6);
        c.insert("object_id".into(), json!(canvas));
        c.insert("name".into(), json!("Canvas 1"));
        doc.objects.insert(canvas.clone(), Value::Object(c));
        (doc, canvas)
    }

    /// Append a new canvas named `name` to the document. Returns its id.
    pub fn add_canvas(&mut self, name: &str) -> String {
        let mut c = self.base_shape(1, 6);
        c.insert("name".into(), json!(name));
        let id = self.insert(c);
        let root = self.root_id();
        if let Some(r) = self.get_mut(&root) {
            let list = r.entry("canvas_refs").or_insert_with(|| json!([]));
            if let Some(a) = list.as_array_mut() {
                a.push(json!(id));
            }
        }
        id
    }

    fn root_id(&self) -> String {
        self.root().map(|(k, _)| k.to_string()).unwrap_or_default()
    }

    fn base_shape(&self, type_id: i64, shape_type: i64) -> Obj {
        let mut o = Obj::new();
        o.insert("type_id".into(), json!(type_id));
        o.insert("doc_ref".into(), json!(self.root_id()));
        o.insert("shape_type".into(), json!(shape_type));
        if let Some(d) = crate::format::defaults(type_id) {
            for (k, v) in d {
                o.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        o
    }

    /// Next auto name `"<DisplayName> <n+1>"`, updating `shape_seq_numbers`.
    pub(crate) fn next_name(&mut self, shape_type: i64) -> String {
        let root = self.root_id();
        let mut n = 1;
        if let Some(r) = self.get_mut(&root) {
            let seq = r.entry("shape_seq_numbers").or_insert_with(|| json!({}));
            if let Some(m) = seq.as_object_mut() {
                let key = shape_type.to_string();
                n = m.get(&key).and_then(|v| v.as_i64()).unwrap_or(0) + 1;
                m.insert(key, json!(n));
            }
        }
        format!("{} {}", shape_display_name(shape_type), n)
    }

    pub(crate) fn add_object(&mut self, type_id: i64, fields: Obj) -> String {
        let mut o = Obj::new();
        o.insert("type_id".into(), json!(type_id));
        o.insert("doc_ref".into(), json!(self.root_id()));
        if let Some(d) = crate::format::defaults(type_id) {
            for (k, v) in d {
                o.insert(k.clone(), v.clone());
            }
        }
        for (k, v) in fields {
            o.insert(k, v);
        }
        self.insert(o)
    }

    /// Insert `child` into `parent`'s subshape list at `index` (0 = top of the z-order).
    pub fn attach_child(&mut self, parent: &str, child: &str, index: usize) {
        if let Some(p) = self.get_mut(parent) {
            let list = p.entry("subshape_refs").or_insert_with(|| json!([]));
            if let Some(a) = list.as_array_mut() {
                let i = index.min(a.len());
                a.insert(i, json!(child));
            }
        }
    }

    /// Remove `child` from `parent`'s subshape list; returns its former index.
    pub fn detach_child(&mut self, parent: &str, child: &str) -> Option<usize> {
        let p = self.get_mut(parent)?;
        let a = p.get_mut("subshape_refs")?.as_array_mut()?;
        let i = a.iter().position(|v| v.as_str() == Some(child))?;
        a.remove(i);
        Some(i)
    }

    pub fn set_value(&mut self, id: &str, key: &str, value: Value) {
        if let Some(o) = self.get_mut(id) {
            o.insert(key.to_string(), value);
        }
    }

    /// Add a static position owned by `owner` (appended to its `position_refs`).
    pub fn add_static_position(&mut self, owner: &str, p: Point) -> String {
        let mut f = Obj::new();
        f.insert("point".into(), json!(format_point(p)));
        let id = self.add_object(11, f);
        if let Some(o) = self.get_mut(owner) {
            let list = o.entry("position_refs").or_insert_with(|| json!([]));
            if let Some(a) = list.as_array_mut() {
                a.push(json!(id));
            }
        }
        id
    }

    /// Add a group under `parent` at the top of its z-order.
    pub fn add_group(&mut self, parent: &str) -> String {
        let mut o = self.base_shape(3, 8);
        let name = self.next_name(8);
        o.insert("name".into(), json!(name));
        let id = self.insert(o);
        self.attach_child(parent, &id, 0);
        id
    }

    /// Add a rectangle (shape_type 1) with a text object, at the top of `parent`'s z-order.
    /// The frame is anchored by a static position at its top-left in canvas coordinates
    /// minus the parent's translation (callers pass absolute coordinates for top-level use).
    pub fn add_rect(&mut self, parent: &str, frame: Rect, text: &str) -> String {
        self.add_rect_typed(parent, frame, text, 1)
    }

    /// Like [`Document::add_rect`] with an explicit `shape_type` (1 rectangle, 4 text, 5 surface).
    pub fn add_rect_typed(&mut self, parent: &str, frame: Rect, text: &str, shape_type: i64) -> String {
        let mut o = self.base_shape(20, shape_type);
        let name = self.next_name(shape_type);
        o.insert("name".into(), json!(name));
        o.insert("frame_size".into(), json!(format_point(Point::new(frame.w, frame.h))));
        let mut t = Obj::new();
        t.insert("text".into(), json!(text));
        let tid = self.add_object(27, t);
        o.insert("model_text".into(), json!(tid));
        let tix = self.add_object(18, Obj::new());
        o.insert("content_tixels".into(), json!(tix));
        let tix2 = self.add_object(18, Obj::new());
        o.insert("border_tixels".into(), json!(tix2));
        o.insert("position_refs".into(), json!([]));
        let id = self.insert(o);
        let anchor = self.add_static_position(&id, Point::new(frame.x, frame.y));
        self.set_value(&id, "frame_anchor_ref", json!(anchor));
        self.attach_child(parent, &id, 0);
        id
    }

    /// Add an ellipse (type 28) bounded by `frame`, at the top of `parent`'s z-order.
    pub fn add_ellipse(&mut self, parent: &str, frame: Rect) -> String {
        let mut o = self.base_shape(28, 12);
        let name = self.next_name(12);
        o.insert("name".into(), json!(name));
        o.insert("frame_size".into(), json!(format_point(Point::new(frame.w, frame.h))));
        let mut t = Obj::new();
        t.insert("text".into(), json!(""));
        let tid = self.add_object(27, t);
        o.insert("model_text_ref".into(), json!(tid));
        o.insert("position_refs".into(), json!([]));
        let id = self.insert(o);
        let anchor = self.add_static_position(&id, Point::new(frame.x, frame.y));
        self.set_value(&id, "frame_anchor_ref", json!(anchor));
        self.attach_child(parent, &id, 0);
        id
    }

    /// Add a diamond (type 21) with centre `centre` and `radius`, at the top of `parent`'s z-order.
    pub fn add_diamond(&mut self, parent: &str, centre: Point, radius: i32) -> String {
        let mut o = self.base_shape(21, 9);
        let name = self.next_name(9);
        o.insert("name".into(), json!(name));
        o.insert("radius".into(), json!(radius.max(1)));
        o.insert("position_refs".into(), json!([]));
        let tix = self.add_object(18, Obj::new());
        o.insert("content_tixels".into(), json!(tix));
        let id = self.insert(o);
        let anchor = self.add_static_position(&id, centre);
        self.set_value(&id, "anchor_point", json!(anchor));
        self.attach_child(parent, &id, 0);
        id
    }

    /// Replace the cells of the tixmap referenced by `shape[key]`.
    pub fn set_tixmap_cells(&mut self, shape: &str, key: &str, cells: &[(Point, char)]) {
        let Some(tid) = self.get(shape).and_then(|o| o.str_(key)).map(str::to_string) else { return };
        let mut sorted: Vec<(Point, char)> = cells.to_vec();
        sorted.sort_by_key(|(p, _)| (p.y, p.x));
        let content: Vec<Value> = sorted.iter().map(|(_, c)| json!(c.to_string())).collect();
        let points: Vec<Value> = sorted.iter().map(|(p, _)| json!(format_point(*p))).collect();
        self.set_value(&tid, "tixels", json!({"text_content": content, "text_points": points}));
    }

    /// The text object of a rect / ellipse.
    pub fn text_of(&self, shape: &str) -> Option<String> {
        let o = self.get(shape)?;
        o.str_("model_text").or_else(|| o.str_("model_text_ref")).map(str::to_string)
    }
}

/// One end of a new line.
#[derive(Clone, Debug)]
pub enum LineEnd {
    /// A free point in canvas coordinates.
    Free(Point),
    /// Attached to a keypad position (SPEC §4) of a shape, or for a diamond a `dpos` (SPEC §6.3).
    Attached { shape: String, rect_position: i64 },
}

impl Document {
    /// Add the position a line end attaches to on `shape`: a `RectPosition` (type 12) or, on a
    /// diamond, a `DiamondStandardPosition` (type 22, `rect_position` is then the `dpos`).
    pub fn add_rect_position(&mut self, shape: &str, rect_position: i64) -> String {
        let (ty, key) = self.attach_kind(shape);
        let mut f = Obj::new();
        f.insert(key.into(), json!(rect_position));
        let id = self.add_object(ty, f);
        if let Some(o) = self.get_mut(shape) {
            let list = o.entry("position_refs").or_insert_with(|| json!([]));
            if let Some(a) = list.as_array_mut() {
                a.push(json!(id));
            }
        }
        id
    }

    /// Add a Diagram line (line_type 3) under `parent`, top of the z-order.
    /// `parent_translation` converts free points to the parent's coordinates.
    pub fn add_line(&mut self, parent: &str, parent_translation: Point, start: LineEnd, end: LineEnd, end_head: i64) -> String {
        let mut o = self.base_shape(5, 2);
        let name = self.next_name(2);
        o.insert("name".into(), json!(name));
        o.insert("attachable".into(), json!(false));
        o.insert("position_refs".into(), json!([]));
        let tix = self.add_object(18, Obj::new());
        o.insert("content_tixmap_ref".into(), json!(tix));
        let id = self.insert(o);
        let mut ends = Vec::new();
        for (i, e) in [start, end].into_iter().enumerate() {
            let pos = match e {
                LineEnd::Free(p) => self.add_static_position(&id, p.sub(parent_translation)),
                LineEnd::Attached { shape, rect_position } => self.add_rect_position(&shape, rect_position),
            };
            let mut f = Obj::new();
            f.insert("position_ref".into(), json!(pos));
            f.insert("head_type".into(), json!(if i == 1 { end_head } else { 0 }));
            let eid = self.add_object(10, f);
            ends.push(eid);
        }
        let mut seg = Obj::new();
        seg.insert("direction".into(), json!(1));
        let sid = self.add_object(9, seg);
        if let Some(o) = self.get_mut(&id) {
            o.insert("segment_end_refs".into(), json!(ends.clone()));
            o.insert("segment_refs".into(), json!([sid]));
            if let Some(a) = o.get_mut("position_refs").and_then(|v| v.as_array_mut()) {
                for e in ends.iter().rev() {
                    a.insert(0, json!(e));
                }
            }
        }
        self.attach_child(parent, &id, 0);
        id
    }

    /// Re-point end `index` (0 = start, 1 = end) of `line`. `line_translation` is the line's
    /// scene translation (free points are stored relative to it). A position the end no longer
    /// uses is removed when nothing else refers to it.
    pub fn set_line_end(&mut self, line: &str, index: usize, end: LineEnd, line_translation: Point) {
        let Some(eid) = self.get(line).and_then(|o| o.refs("segment_end_refs").get(index).map(|s| s.to_string())) else { return };
        let old = self.get(&eid).and_then(|o| o.str_("position_ref")).map(str::to_string);
        let owned_by_line = |d: &Document, pid: &str| {
            d.get(pid).is_some_and(|o| o.type_id() == 11) && d.get(line).is_some_and(|l| l.refs("position_refs").contains(&pid))
        };
        let new = match end {
            LineEnd::Free(p) => {
                if let Some(o) = old.as_deref().filter(|o| owned_by_line(self, o)) {
                    self.set_value(o, "point", json!(format_point(p.sub(line_translation))));
                    return;
                }
                self.add_static_position(line, p.sub(line_translation))
            }
            LineEnd::Attached { shape, rect_position } => {
                let (ty, key) = self.attach_kind(&shape);
                let existing = old.as_deref().filter(|o| {
                    self.get(o).is_some_and(|po| po.type_id() == ty && po.int(key) == Some(rect_position))
                        && self.get(&shape).is_some_and(|s| s.refs("position_refs").contains(o))
                });
                if existing.is_some() {
                    return;
                }
                self.add_rect_position(&shape, rect_position)
            }
        };
        self.set_value(&eid, "position_ref", json!(new));
        if let Some(old) = old {
            self.drop_position_if_unused(&old);
        }
    }

    /// Object type and value key of the attach positions `shape` owns.
    fn attach_kind(&self, shape: &str) -> (i64, &'static str) {
        if self.get(shape).is_some_and(|o| o.type_id() == 21) {
            (22, "dpos")
        } else {
            (12, "rect_position")
        }
    }

    /// Remove a static, rect or diamond position that no object refers to any more.
    fn drop_position_if_unused(&mut self, pid: &str) {
        if !self.get(pid).is_some_and(|o| matches!(o.type_id(), 11 | 12 | 22)) {
            return;
        }
        let used = self.objects.values().filter_map(|v| v.as_object()).any(|o| {
            ["position_ref", "frame_anchor_ref", "anchor_point"].iter().any(|k| o.str_(k) == Some(pid))
        });
        if used {
            return;
        }
        for v in self.objects.values_mut() {
            if let Some(a) = v.as_object_mut().and_then(|o| o.get_mut("position_refs")).and_then(|v| v.as_array_mut()) {
                a.retain(|x| x.as_str() != Some(pid));
            }
        }
        self.objects.shift_remove(pid);
    }

    /// Move a shape by adding to its `origin` (children and owned positions follow).
    pub fn translate_shape(&mut self, id: &str, d: Point) {
        if let Some(o) = self.get_mut(id) {
            let cur = o.point("origin").unwrap_or_default();
            o.insert("origin".into(), json!(format_point(cur.add(d))));
        }
    }

    /// Set a rect/ellipse frame size.
    pub fn resize_shape(&mut self, id: &str, w: i32, h: i32) {
        if self.get(id).is_some_and(|o| o.type_id() == 25) {
            let b = self.get(id).and_then(|o| o.str_("img_bounds")).and_then(crate::geom::parse_rect).unwrap_or_default();
            self.set_value(id, "img_bounds", json!(crate::geom::format_rect(Rect::new(b.x, b.y, w.max(1), h.max(1)))));
            return;
        }
        self.set_value(id, "frame_size", json!(format_point(Point::new(w.max(1), h.max(1)))));
    }

    /// Add an image overlay (type 25) showing `data` (the image file's bytes) stretched over `r`
    /// (relative to `parent`), at the top of `parent`'s z-order. It draws no characters.
    pub fn add_image(&mut self, parent: &str, data: &[u8], r: Rect, opacity: f64) -> String {
        let mut o = self.base_shape(25, 10);
        let name = self.next_name(10);
        o.insert("name".into(), json!(name));
        o.insert("img_data".into(), json!(crate::b64::encode(data)));
        o.insert("img_bounds".into(), json!(crate::geom::format_rect(r)));
        o.insert("img_opacity".into(), json!(opacity));
        let id = self.insert(o);
        self.attach_child(parent, &id, 0);
        id
    }

    /// The image file bytes of an image overlay.
    pub fn image_data(&self, id: &str) -> Option<Vec<u8>> {
        self.get(id).filter(|o| o.type_id() == 25).and_then(|o| o.str_("img_data")).and_then(crate::b64::decode)
    }

    /// Set (or clear) one cell of the tixmap referenced by `shape[key]`, creating the tixmap if needed.
    pub fn set_tixmap_cell(&mut self, shape: &str, key: &str, p: Point, c: Option<char>) {
        if self.get(shape).and_then(|o| o.str_(key)).is_none() {
            let t = self.add_object(18, Obj::new());
            self.set_value(shape, key, json!(t));
        }
        let Some(tid) = self.get(shape).and_then(|o| o.str_(key)).map(str::to_string) else { return };
        let mut cells: Vec<(Point, char)> = Vec::new();
        if let Some(t) = self.get(&tid).and_then(|o| o.val("tixels")).and_then(|v| v.as_object()) {
            let content = t.get("text_content").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let points = t.get("text_points").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            for (cv, pv) in content.iter().zip(points.iter()) {
                if let (Some(ch), Some(pt)) = (cv.as_str().and_then(|s| s.chars().next()), pv.as_str().and_then(crate::geom::parse_point)) {
                    if pt != p {
                        cells.push((pt, ch));
                    }
                }
            }
        }
        if let Some(ch) = c {
            cells.push((p, ch));
        }
        self.set_tixmap_cells(shape, key, &cells);
    }

    /// Delete a shape and everything it owns. Lines attached to its positions are re-pointed
    /// at static positions at their current location so the file stays consistent.
    pub fn delete_shape(&mut self, canvas: &str, id: &str) {
        let (owned, subtree) = self.collect_owned(id);
        // Re-anchor line ends that point into the deleted set.
        let scene_points: Vec<(String, String, Point, Point)> = {
            let scene = crate::scene::Scene::new(self, canvas, true);
            let mut v = Vec::new();
            for (eid, val) in &self.objects {
                let Some(o) = val.as_object() else { continue };
                if o.type_id() != 10 {
                    continue;
                }
                let Some(pr) = o.str_("position_ref") else { continue };
                if owned.contains(pr) && !owned.contains(eid) {
                    if let Some(r) = scene.resolve(eid) {
                        let line = self
                            .objects
                            .iter()
                            .find(|(_, v)| v.as_object().is_some_and(|l| l.refs("segment_end_refs").contains(&eid.as_str())))
                            .map(|(k, _)| k.clone());
                        if let Some(line) = line {
                            let t = scene.translation(&line);
                            v.push((eid.clone(), line, r.p, t));
                        }
                    }
                }
            }
            v
        };
        for (eid, line, p, t) in scene_points {
            let sp = self.add_static_position(&line, p.sub(t));
            self.set_value(&eid, "position_ref", json!(sp));
        }
        // Detach from parent and drop objects.
        let parents: Vec<String> = self
            .objects
            .iter()
            .filter(|(_, v)| v.as_object().is_some_and(|o| o.refs("subshape_refs").contains(&id)))
            .map(|(k, _)| k.clone())
            .collect();
        for p in parents {
            self.detach_child(&p, id);
        }
        let _ = subtree;
        for o in owned {
            self.objects.shift_remove(&o);
        }
    }

    /// Ids owned by a shape subtree: the shapes, their positions, texts, tixmaps, segments.
    fn collect_owned(&self, id: &str) -> (std::collections::HashSet<String>, Vec<String>) {
        let mut owned = std::collections::HashSet::new();
        let mut shapes = Vec::new();
        let mut stack = vec![id.to_string()];
        while let Some(s) = stack.pop() {
            let Some(o) = self.get(&s) else { continue };
            shapes.push(s.clone());
            owned.insert(s.clone());
            for key in ["position_refs", "segment_end_refs", "segment_refs"] {
                for r in o.refs(key) {
                    owned.insert(r.to_string());
                }
            }
            for key in ["model_text", "model_text_ref", "content_tixels", "border_tixels", "content_tixmap_ref"] {
                if let Some(r) = o.str_(key) {
                    owned.insert(r.to_string());
                }
            }
            // Table cells and their texts.
            if let Some(cells) = o.get("cells").and_then(|v| v.as_object()) {
                for c in cells.values().filter_map(|v| v.as_str()) {
                    owned.insert(c.to_string());
                    if let Some(t) = self.get(c).and_then(|c| c.str_("text_ref")) {
                        owned.insert(t.to_string());
                    }
                }
            }
            stack.extend(o.refs("subshape_refs").into_iter().map(str::to_string));
        }
        // Static positions owned by owned line ends are already in position_refs.
        (owned, shapes)
    }
}

impl crate::scene::Scene<'_> {
    /// Topmost visible shape whose geometry covers `p` (leaf shapes only).
    pub fn hit_test(&self, p: Point) -> Option<String> {
        for id in self.draw_order().into_iter().rev() {
            let Some(o) = self.obj(&id) else { continue };
            match o.type_id() {
                5 => {
                    if let Some(path) = self.line_path(&id) {
                        if path.cells.contains(&p) {
                            return Some(id);
                        }
                    }
                }
                20 | 28 | 21 | 7 | 26 => {
                    if self.frame(&id).is_some_and(|f| f.contains(p)) {
                        return Some(id);
                    }
                }
                // Image overlays take clicks unless they are click-through.
                25 => {
                    if !o.boolean("cthrough") && self.frame(&id).is_some_and(|f| f.contains(p)) {
                        return Some(id);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// The ancestor of `id` that is a direct child of the canvas.
    pub fn root_of(&self, id: &str) -> String {
        let mut cur = id.to_string();
        while let Some(p) = self.parent(&cur) {
            if p == self.canvas {
                break;
            }
            cur = p.to_string();
        }
        cur
    }

    /// What a click on leaf `hit` selects: its top-level shape, except that a group with
    /// "Allows sub-shape selection" (`subshape_selection`) passes the click on to its child,
    /// and so on down nested groups.
    pub fn click_target(&self, hit: &str) -> String {
        let mut chain = vec![hit.to_string()];
        while let Some(p) = self.parent(chain.last().unwrap()) {
            if p == self.canvas {
                break;
            }
            chain.push(p.to_string());
        }
        let mut pick = chain.pop().unwrap();
        while let Some(child) = chain.pop() {
            if !self.obj(&pick).is_some_and(|o| o.boolean("subshape_selection")) {
                break;
            }
            pick = child;
        }
        pick
    }
}

/// Cells 4-connected to `start` inside `bounds` that show the same character as `start`
/// (`char_at` gives the visible character, `None` = empty). Used by the bucket fill tool.
pub fn flood_region(start: Point, bounds: Rect, char_at: impl Fn(Point) -> Option<char>) -> Vec<Point> {
    if !bounds.contains(start) {
        return vec![];
    }
    let blank = |c: Option<char>| c.filter(|&c| c != ' ');
    let target = blank(char_at(start));
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![start];
    let mut out = Vec::new();
    while let Some(p) = stack.pop() {
        if !bounds.contains(p) || !seen.insert(p) || blank(char_at(p)) != target {
            continue;
        }
        out.push(p);
        stack.extend([p.offset(1, 0), p.offset(-1, 0), p.offset(0, 1), p.offset(0, -1)]);
    }
    out
}

// ---------------------------------------------------------------------------
// Line waypoints and labels

impl Document {
    /// Insert a waypoint at `p` (canvas coordinates) after end `after`, splitting that segment.
    /// A Diagram line (which only routes between its first and last ends) becomes Automatic so
    /// the waypoint is used. Returns the new end's index.
    pub fn insert_line_point(&mut self, line: &str, after: usize, p: Point, line_translation: Point) -> Option<usize> {
        let o = self.get(line)?;
        let ends: Vec<String> = o.refs("segment_end_refs").into_iter().map(str::to_string).collect();
        let segs: Vec<String> = o.refs("segment_refs").into_iter().map(str::to_string).collect();
        if after + 1 >= ends.len() {
            return None;
        }
        let direction = segs.get(after).and_then(|s| self.get(s)).map(|s| s.int_or("direction", 1)).unwrap_or(1);
        let pos = self.add_static_position(line, p.sub(line_translation));
        let mut e = Obj::new();
        e.insert("position_ref".into(), json!(pos));
        e.insert("head_type".into(), json!(0));
        let eid = self.add_object(10, e);
        let mut sg = Obj::new();
        sg.insert("direction".into(), json!(direction));
        let sid = self.add_object(9, sg);
        let o = self.get_mut(line)?;
        let insert = |o: &mut Obj, key: &str, at: usize, id: &str| {
            if let Some(a) = o.get_mut(key).and_then(|v| v.as_array_mut()) {
                a.insert(at.min(a.len()), json!(id));
            }
        };
        insert(o, "segment_end_refs", after + 1, &eid);
        insert(o, "segment_refs", after + 1, &sid);
        if let Some(a) = o.get_mut("position_refs").and_then(|v| v.as_array_mut()) {
            a.push(json!(eid));
        }
        if o.int_or("line_type", 3) == 3 {
            o.insert("line_type".into(), json!(2));
        }
        Some(after + 1)
    }

    /// Remove interior end `index` (a waypoint), joining its two segments.
    pub fn remove_line_point(&mut self, line: &str, index: usize) -> bool {
        let Some(o) = self.get(line) else { return false };
        let ends: Vec<String> = o.refs("segment_end_refs").into_iter().map(str::to_string).collect();
        let segs: Vec<String> = o.refs("segment_refs").into_iter().map(str::to_string).collect();
        if index == 0 || index + 1 >= ends.len() || segs.len() < 2 {
            return false;
        }
        let end = ends[index].clone();
        let seg = segs[index].clone();
        let pos = self.get(&end).and_then(|e| e.str_("position_ref")).map(str::to_string);
        if let Some(o) = self.get_mut(line) {
            for key in ["segment_end_refs", "position_refs"] {
                if let Some(a) = o.get_mut(key).and_then(|v| v.as_array_mut()) {
                    a.retain(|x| x.as_str() != Some(end.as_str()));
                }
            }
            if let Some(a) = o.get_mut("segment_refs").and_then(|v| v.as_array_mut()) {
                a.retain(|x| x.as_str() != Some(seg.as_str()));
            }
        }
        self.objects.shift_remove(&end);
        self.objects.shift_remove(&seg);
        if let Some(p) = pos {
            self.drop_position_if_unused(&p);
        }
        true
    }

    /// Add a text label centred on the middle of `line`, at the top of `parent`'s z-order. The
    /// label follows the line; moving it changes its offset from that point.
    pub fn add_line_label(&mut self, parent: &str, line: &str, text: &str) -> Option<String> {
        self.get(line)?;
        let w = text.lines().map(|l| l.chars().count()).max().unwrap_or(0) as i32 + 2;
        let h = text.lines().count().max(1) as i32;
        let label = self.add_rect_typed(parent, Rect::new(0, 0, w, h), text, 4);
        let old = self.get(&label).and_then(|o| o.str_("frame_anchor_ref")).map(str::to_string);
        let mut po = Obj::new();
        po.insert("line_pt_idx".into(), json!(-1));
        po.insert("line_pt_rel".into(), json!(0.5));
        let pos = self.add_object(24, po);
        if let Some(a) = self.get_mut(line).and_then(|o| o.get_mut("position_refs")).and_then(|v| v.as_array_mut()) {
            a.push(json!(pos));
        }
        self.set_value(&label, "frame_anchor_ref", json!(pos));
        self.set_value(&label, "frame_corner", json!(5));
        self.set_value(&label, "rectangular_border_enabled", json!(false));
        self.set_value(&label, "background_enabled", json!(true));
        self.set_value(&label, "background_type", json!(2));
        self.set_value(&label, "attachable", json!(false));
        self.set_value(&label, "name", json!(format!("Label {}", text.lines().next().unwrap_or_default())));
        if let Some(old) = old {
            self.drop_position_if_unused(&old);
        }
        Some(label)
    }

    /// Move a shape by `d` the way dragging does: shapes anchored to another shape's position
    /// (line labels) change their offset from it; everything else moves its origin.
    pub fn move_shape(&mut self, id: &str, d: Point) {
        let Some(o) = self.get(id) else { return };
        let own = o.refs("position_refs");
        let external = o.str_("frame_anchor_ref").filter(|a| !own.contains(a)).is_some();
        if external {
            let cur = o.point("frame_offset").unwrap_or_default();
            self.set_value(id, "frame_offset", json!(format_point(cur.add(d))));
        } else {
            self.translate_shape(id, d);
        }
    }
}
