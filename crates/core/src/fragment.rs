//! Shape fragments: detached copies of shapes for the clipboard, duplicate and snippets
//! (SPEC §8.3), plus the structural edits built on them (group, ungroup, flatten, style).
//!
//! A fragment is a persistence graph like a document's: `{"header": {"v": 5}, "object_list": [...]}`
//! whose list holds a shape list object (type 13, `shapes` = root shapes, top of the z-order first)
//! and everything the roots own. Root shapes store their `origin` in canvas coordinates and carry
//! `canvas_origin = true`. Snippet content files use the same graph (older header versions are
//! migrated on load).

use std::collections::{HashMap, HashSet};

use serde_json::{json, Value};

use crate::format::{new_id, rewrite, Document, Obj, ObjExt, Result};
use crate::geom::{format_point, Point, Rect};
use crate::scene::Scene;

/// Keys that point at objects outside the owning shape (re-anchored when the target is not copied).
const CROSS_REFS: [&str; 3] = ["position_ref", "frame_anchor_ref", "anchor_point"];

/// Shape keys that are structure or geometry, not style.
const NOT_STYLE: &[&str] = &[
    "object_id", "type_id", "doc_ref", "shape_type", "origin", "name", "name_edited", "subshape_refs",
    "position_refs", "segment_refs", "segment_end_refs", "model_text", "model_text_ref", "content_tixels",
    "border_tixels", "content_tixmap_ref", "frame_anchor_ref", "anchor_point", "frame_size", "frame_offset",
    "frame_corner", "content_offset", "radius", "anchor_pos", "locked", "hidden", "canvas_origin", "tag",
    "rows", "cols", "cells",
];

fn ref_strings(v: &Value) -> Vec<&str> {
    match v {
        Value::String(s) => vec![s.as_str()],
        Value::Array(a) => a.iter().filter_map(|x| x.as_str()).collect(),
        // Table `cells`: key -> cell ref.
        Value::Object(m) => m.values().filter_map(|x| x.as_str()).collect(),
        _ => vec![],
    }
}

impl Document {
    /// Ids of `roots` and everything they own, in document order. Cross references
    /// ([`CROSS_REFS`]) are not followed.
    fn owned_closure(&self, roots: &[String]) -> Vec<String> {
        let mut seen: HashSet<String> = HashSet::new();
        let mut stack: Vec<String> = roots.to_vec();
        while let Some(id) = stack.pop() {
            let Some(o) = self.get(&id) else { continue };
            if !seen.insert(id.clone()) {
                continue;
            }
            for (k, v) in o {
                if k == "doc_ref" || k == "object_id" || CROSS_REFS.contains(&k.as_str()) {
                    continue;
                }
                for r in ref_strings(v) {
                    if self.objects.contains_key(r) && !seen.contains(r) {
                        stack.push(r.to_string());
                    }
                }
            }
        }
        self.objects.keys().filter(|k| seen.contains(*k)).cloned().collect()
    }

    /// A detached fragment of `roots` (top of the z-order first). Line ends attached to shapes
    /// outside the fragment, and frames or diamonds anchored outside it, are re-pointed at static
    /// positions at their current place.
    pub fn extract_fragment(&self, canvas: &str, roots: &[String]) -> Value {
        let scene = Scene::new(self, canvas, self.unicode());
        // Roots top of the z-order first, whatever order they were selected in.
        let order = scene.draw_order();
        let mut sorted: Vec<String> = roots.to_vec();
        sorted.sort_by_key(|r| std::cmp::Reverse(order.iter().position(|x| x == r)));
        let roots = &sorted[..];
        let ids = self.owned_closure(roots);
        let inside: HashSet<&str> = ids.iter().map(String::as_str).collect();
        let mut objs: Vec<Obj> = ids.iter().filter_map(|id| self.get(id).cloned()).collect();

        // Owner of each line end (for re-anchoring to a static position owned by the line).
        let mut end_owner: HashMap<String, String> = HashMap::new();
        for o in &objs {
            if let Some(id) = o.str_("object_id") {
                for e in o.refs("segment_end_refs") {
                    end_owner.insert(e.to_string(), id.to_string());
                }
            }
        }
        let mut extra: Vec<Obj> = Vec::new();
        let mut new_owned: Vec<(String, String)> = Vec::new();
        for o in objs.iter_mut() {
            let Some(id) = o.str_("object_id").map(str::to_string) else { continue };
            for key in CROSS_REFS {
                let Some(target) = o.get(key).and_then(|v| v.as_str()).map(str::to_string) else { continue };
                if inside.contains(target.as_str()) {
                    continue;
                }
                // Resolve where it is now, relative to whoever will own the static position.
                let (owner, point) = if key == "position_ref" {
                    let Some(line) = end_owner.get(&id).cloned() else { continue };
                    let Some(r) = scene.resolve(&id) else { continue };
                    let t = scene.translation(&line);
                    (line, r.p.sub(t))
                } else {
                    let Some(r) = scene.resolve(&target) else { continue };
                    (id.clone(), r.p.sub(scene.translation(&id)))
                };
                let pid = new_id();
                let mut p = Obj::new();
                p.insert("object_id".into(), json!(pid));
                p.insert("type_id".into(), json!(11));
                p.insert("dirset".into(), json!(0));
                p.insert("point".into(), json!(format_point(point)));
                extra.push(p);
                o.insert(key.into(), json!(pid));
                new_owned.push((owner, pid));
            }
        }
        for (owner, pid) in new_owned {
            if let Some(o) = objs.iter_mut().find(|o| o.str_("object_id") == Some(owner.as_str())) {
                let list = o.entry("position_refs").or_insert_with(|| json!([]));
                if let Some(a) = list.as_array_mut() {
                    a.push(json!(pid));
                }
            }
        }

        let mut list: Vec<Value> = Vec::new();
        let mut shapes = Obj::new();
        shapes.insert("object_id".into(), json!(new_id()));
        shapes.insert("type_id".into(), json!(13));
        shapes.insert("shapes".into(), json!(roots));
        list.push(Value::Object(shapes));
        for mut o in objs.into_iter().chain(extra) {
            o.remove("doc_ref");
            let id = o.str_("object_id").unwrap_or_default().to_string();
            if roots.contains(&id) {
                o.insert("origin".into(), json!(format_point(scene.translation(&id))));
                o.insert("canvas_origin".into(), json!(true));
            }
            list.push(Value::Object(o));
        }
        json!({"header": {"v": crate::format::MODEL_VERSION}, "object_list": list})
    }

    /// Insert a fragment under `parent` (whose scene translation is `parent_translation`) with
    /// fresh ids, its roots at the top of the z-order and moved by `offset`. Returns the new roots,
    /// top first.
    pub fn paste_fragment(&mut self, frag: &Value, parent: &str, parent_translation: Point, offset: Point) -> Result<Vec<String>> {
        let src = Document::from_json(frag.clone())?;
        let list_obj = src.objects.values().filter_map(|v| v.as_object()).find(|o| o.type_id() == 13);
        let mut roots: Vec<String> = match list_obj {
            Some(l) => l.refs("shapes").into_iter().map(str::to_string).collect(),
            None => {
                // No shape list: every shape no other shape contains.
                let kids: HashSet<&str> =
                    src.objects.values().filter_map(|v| v.as_object()).flat_map(|o| o.refs("subshape_refs")).collect();
                src.objects
                    .iter()
                    .filter(|(k, v)| v.as_object().is_some_and(|o| o.get("shape_type").is_some()) && !kids.contains(k.as_str()))
                    .map(|(k, _)| k.clone())
                    .collect()
            }
        };
        roots.retain(|r| src.objects.contains_key(r));
        let idmap: HashMap<String, String> =
            src.objects.iter().filter(|(_, v)| v.as_object().is_some_and(|o| o.type_id() != 13)).map(|(k, _)| (k.clone(), new_id())).collect();
        let doc_root = self.root().map(|(k, _)| k.to_string()).unwrap_or_default();
        for (old, v) in &src.objects {
            let Some(new) = idmap.get(old) else { continue };
            let Value::Object(mut o) = rewrite(v.clone(), &idmap) else { continue };
            o.insert("object_id".into(), json!(new));
            o.insert("doc_ref".into(), json!(doc_root));
            if roots.contains(old) {
                let at = o.point("origin").unwrap_or_default();
                let local = if o.boolean("canvas_origin") { at.sub(parent_translation) } else { at };
                o.insert("origin".into(), json!(format_point(local.add(offset))));
                o.remove("canvas_origin");
            }
            self.objects.insert(new.clone(), Value::Object(o));
        }
        let new_roots: Vec<String> = roots.iter().filter_map(|r| idmap.get(r).cloned()).collect();
        for r in new_roots.iter().rev() {
            self.attach_child(parent, r, 0);
        }
        Ok(new_roots)
    }

    /// Wrap `ids` (which must share a parent) in a new group placed where the topmost of them was.
    /// Returns the group id.
    pub fn group_shapes(&mut self, canvas: &str, ids: &[String]) -> Option<String> {
        let parent = {
            let scene = Scene::new(self, canvas, true);
            scene.parent(ids.first()?)?.to_string()
        };
        let siblings: Vec<String> = self.get(&parent)?.refs("subshape_refs").into_iter().map(str::to_string).collect();
        let members: Vec<String> = siblings.iter().filter(|s| ids.contains(s)).cloned().collect();
        let top = siblings.iter().position(|s| members.contains(s))?;
        let group = self.add_group(&parent);
        self.detach_child(&parent, &group);
        for m in &members {
            self.detach_child(&parent, m);
        }
        self.attach_child(&parent, &group, top);
        self.set_value(&group, "subshape_refs", json!(members));
        Some(group)
    }

    /// Move a group's children into its parent at the group's place and remove the group.
    /// Returns the children, top first.
    pub fn ungroup(&mut self, canvas: &str, group: &str) -> Vec<String> {
        let Some(g) = self.get(group) else { return vec![] };
        if g.type_id() != 3 {
            return vec![];
        }
        let kids: Vec<String> = g.refs("subshape_refs").into_iter().map(str::to_string).collect();
        let d = g.point("origin").unwrap_or_default();
        let parent = {
            let scene = Scene::new(self, canvas, true);
            match scene.parent(group) {
                Some(p) => p.to_string(),
                None => return vec![],
            }
        };
        let Some(at) = self.detach_child(&parent, group) else { return vec![] };
        self.attach_child(&parent, group, at);
        for (i, k) in kids.iter().enumerate() {
            self.translate_shape(k, d);
            self.attach_child(&parent, k, at + 1 + i);
        }
        self.set_value(group, "subshape_refs", json!([]));
        self.delete_shape(canvas, group);
        kids
    }

    /// Replace `ids` (sharing a parent) by one Surface holding their rendered characters.
    /// Returns the surface id.
    pub fn flatten(&mut self, canvas: &str, ids: &[String]) -> Option<String> {
        let (parent, pt) = {
            let scene = Scene::new(self, canvas, true);
            let p = scene.parent(ids.first()?)?.to_string();
            let t = scene.translation(&p);
            (p, t)
        };
        let siblings: Vec<String> = self.get(&parent)?.refs("subshape_refs").into_iter().map(str::to_string).collect();
        let members: Vec<String> = siblings.iter().filter(|s| ids.contains(s)).cloned().collect();
        let top = siblings.iter().position(|s| members.contains(s))?;

        // Render only the members, in a scratch document with the same alphabet.
        let frag = self.extract_fragment(canvas, &members);
        let (mut scratch, sc) = Document::new_empty();
        if let Some(r) = scratch.root().map(|(k, _)| k.to_string()) {
            scratch.set_value(&r, "alphabet", json!(if self.unicode() { 1 } else { 2 }));
        }
        scratch.paste_fragment(&frag, &sc, Point::default(), Point::default()).ok()?;
        let buf = crate::render::render_scene(&Scene::new(&scratch, &sc, scratch.unicode()));
        let r = buf.content_box();
        if r.is_empty() {
            return None;
        }
        // Tixmaps don't persist per-cell colours (SPEC §1.7) and a surface has one
        // `content_color`, so each colour gets its own surface; several are grouped.
        let mut by_color: std::collections::BTreeMap<u64, Vec<(Point, char)>> = Default::default();
        for (p, c) in &buf.cells {
            if let Some(ch) = c.t.ch.filter(|ch| *ch != ' ') {
                by_color.entry(c.color).or_default().push((Point::new(p.x - r.x, p.y - r.y), ch));
            }
        }

        for m in &members {
            self.delete_shape(canvas, m);
        }
        let frame = Rect::new(r.x - pt.x, r.y - pt.y, r.w, r.h);
        let mut surfaces = Vec::new();
        for (color, cells) in &by_color {
            let s = self.add_surface(&parent, frame, cells);
            if *color != crate::format::NO_COLOR {
                self.set_value(&s, "content_color", json!(color));
            }
            surfaces.push(s);
        }
        if surfaces.is_empty() {
            surfaces.push(self.add_surface(&parent, frame, &[]));
        }
        let result = if surfaces.len() == 1 {
            surfaces[0].clone()
        } else {
            self.group_shapes(canvas, &surfaces)?
        };
        self.detach_child(&parent, &result);
        self.attach_child(&parent, &result, top);
        Some(result)
    }

    /// Style of a shape: `{type_id, shape: {...}, text: {...}, heads: [start, end]}`.
    pub fn style_of(&self, id: &str) -> Option<Value> {
        let o = self.get(id)?;
        let shape: Obj = o.iter().filter(|(k, _)| !NOT_STYLE.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect();
        let text: Obj = self
            .text_of(id)
            .and_then(|t| self.get(&t))
            .map(|t| t.iter().filter(|(k, _)| !["object_id", "type_id", "doc_ref", "text"].contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        let heads: Vec<Value> = o.refs("segment_end_refs").iter().map(|e| json!(self.get(e).map(|e| e.int_or("head_type", 0)).unwrap_or(0))).collect();
        Some(json!({"type_id": o.type_id(), "shape": shape, "text": text, "heads": heads}))
    }

    /// Apply a style from [`Document::style_of`]. Keys the target's type does not have are skipped.
    pub fn apply_style(&mut self, id: &str, style: &Value) {
        let Some(o) = self.get(id) else { return };
        let has = |o: &Obj, k: &str| o.contains_key(k) || crate::format::defaults(o.type_id()).is_some_and(|d| d.contains_key(k));
        let mut sets: Vec<(String, String, Value)> = Vec::new();
        if let Some(m) = style["shape"].as_object() {
            for (k, v) in m {
                if has(o, k) {
                    sets.push((id.to_string(), k.clone(), v.clone()));
                }
            }
        }
        if let (Some(m), Some(tid)) = (style["text"].as_object(), self.text_of(id)) {
            if let Some(t) = self.get(&tid) {
                for (k, v) in m {
                    if has(t, k) {
                        sets.push((tid.clone(), k.clone(), v.clone()));
                    }
                }
            }
        }
        if let Some(h) = style["heads"].as_array().filter(|h| !h.is_empty()) {
            for (i, e) in o.refs("segment_end_refs").iter().enumerate() {
                if let Some(v) = h.get(i) {
                    sets.push((e.to_string(), "head_type".into(), v.clone()));
                }
            }
        }
        for (target, k, v) in sets {
            self.set_value(&target, &k, v);
        }
    }

    /// "Apply selection shape style on create": give the new shape `new` the shape style of
    /// `src` (the primary selection). Text style is not copied. A line takes styles only from a
    /// line, a diamond only from a diamond, a rect-positioned shape (rectangle, text, surface,
    /// ellipse, table, image) from any rect-positioned shape, keeping the keys it has
    ///. Returns whether anything was applied.
    pub fn apply_create_style(&mut self, src: &str, new: &str) -> bool {
        let (Some(a), Some(b)) = (self.get(src).map(|o| o.type_id()), self.get(new).map(|o| o.type_id())) else { return false };
        let rect_positioned = |t: i64| matches!(t, 20 | 25 | 26 | 28);
        if !((a == 5 && b == 5) || (a == 21 && b == 21) || (rect_positioned(a) && rect_positioned(b))) {
            return false;
        }
        let Some(mut style) = self.style_of(src) else { return false };
        style["text"] = json!({});
        self.apply_style(new, &style);
        true
    }

    /// Clear a shape's custom content (pencil cells).
    pub fn reset_content(&mut self, id: &str) {
        for key in ["content_tixels", "content_tixmap_ref"] {
            if self.get(id).and_then(|o| o.str_(key)).is_some() {
                self.set_tixmap_cells(id, key, &[]);
            }
        }
    }

    /// Add a Surface holding `text` (one cell per character, spaces left empty) with its
    /// top-left at `at` in `parent` coordinates. Returns the surface id.
    pub fn add_text_surface(&mut self, parent: &str, at: Point, text: &str) -> Option<String> {
        let mut cells = Vec::new();
        let (mut w, mut h) = (0, 0);
        for (y, line) in text.lines().enumerate() {
            let mut x = 0;
            for ch in line.chars() {
                if ch == '\t' {
                    x += 4 - x % 4;
                    continue;
                }
                if ch != ' ' && !ch.is_control() {
                    cells.push((Point::new(x, y as i32), ch));
                }
                x += 1;
            }
            w = w.max(x);
            h = y as i32 + 1;
        }
        if cells.is_empty() {
            return None;
        }
        Some(self.add_surface(parent, Rect::new(at.x, at.y, w.max(1), h.max(1)), &cells))
    }

    /// A borderless, backgroundless Surface (like the Surface tool's) holding `cells`
    /// (relative to the frame's top-left), at the top of `parent`'s z-order.
    pub fn add_surface(&mut self, parent: &str, frame: Rect, cells: &[(Point, char)]) -> String {
        let s = self.add_rect_typed(parent, frame, "", 5);
        self.set_value(&s, "rectangular_border_enabled", json!(false));
        self.set_value(&s, "background_enabled", json!(false));
        self.set_tixmap_cells(&s, "content_tixels", cells);
        s
    }
}

/// Name of a duplicated shape: "Name" → "Name Copy", "Name Copy" → "Name Copy 2",
/// "Name Copy N" → "Name Copy N+1" (never below 3), "" → "Copy".
pub fn copy_name(name: &str) -> String {
    if name.is_empty() {
        return "Copy".into();
    }
    if let Some(base) = name.strip_suffix(" Copy") {
        return format!("{base} Copy 2");
    }
    if let Some((base, n)) = name.rsplit_once(" Copy ") {
        if let Ok(n) = n.parse::<u64>() {
            return format!("{base} Copy {}", n.max(2) + 1);
        }
    }
    format!("{name} Copy")
}

