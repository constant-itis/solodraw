//! Resolves the object graph into geometry: shape translations, frames, attachment
//! positions and line paths (SPEC §1.4, §4, §5, §6).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::format::{Document, Obj, ObjExt};
use crate::geom::*;
use crate::route::{self, End};

/// A resolved position.
#[derive(Clone, Copy, Debug)]
pub struct Resolved {
    pub p: Point,
    /// Allowed exit directions (0 = unconstrained).
    pub dirs: u8,
    /// Rect of the shape the position belongs to (for routing).
    pub rect: Option<Rect>,
}

/// A rasterized line.
#[derive(Clone, Debug, Default)]
pub struct LinePath {
    pub cells: Vec<Point>,
    /// Segment mask per cell.
    pub masks: Vec<u8>,
    /// Number of cells in the first and last segment (for head placement).
    pub first_seg_len: usize,
    pub last_seg_len: usize,
    /// Corner points of the routed path (orthogonal lines).
    pub corners: Vec<Point>,
}

pub struct Scene<'a> {
    pub doc: &'a Document,
    pub canvas: String,
    pub unicode: bool,
    parent: HashMap<String, String>,
    translation: HashMap<String, Point>,
    owner: HashMap<String, String>,
    frames: RefCell<HashMap<String, Option<Rect>>>,
    paths: RefCell<HashMap<String, Option<Rc<LinePath>>>>,
    busy: RefCell<HashSet<String>>,
}

impl<'a> Scene<'a> {
    pub fn new(doc: &'a Document, canvas: &str, unicode: bool) -> Self {
        let mut s = Scene {
            doc,
            canvas: canvas.to_string(),
            unicode,
            parent: HashMap::new(),
            translation: HashMap::new(),
            owner: HashMap::new(),
            frames: RefCell::new(HashMap::new()),
            paths: RefCell::new(HashMap::new()),
            busy: RefCell::new(HashSet::new()),
        };
        s.walk(canvas, Point::default(), 0);
        for (id, v) in &doc.objects {
            if let Some(o) = v.as_object() {
                for p in o.refs("position_refs") {
                    s.owner.entry(p.to_string()).or_insert_with(|| id.clone());
                }
            }
        }
        s
    }

    fn walk(&mut self, id: &str, base: Point, depth: usize) {
        if depth > 256 {
            return;
        }
        let Some(o) = self.doc.get(id) else { return };
        let t = base.add(o.point("origin").unwrap_or_default());
        self.translation.insert(id.to_string(), t);
        let kids: Vec<String> = o.refs("subshape_refs").into_iter().map(str::to_string).collect();
        for k in kids {
            self.parent.insert(k.clone(), id.to_string());
            self.walk(&k, t, depth + 1);
        }
    }

    pub fn obj(&self, id: &str) -> Option<&'a Obj> {
        self.doc.get(id)
    }

    pub fn translation(&self, id: &str) -> Point {
        self.translation.get(id).copied().unwrap_or_default()
    }

    pub fn parent(&self, id: &str) -> Option<&str> {
        self.parent.get(id).map(String::as_str)
    }

    /// Shapes in draw order (bottom first): pre-order, children reversed (SPEC §4.1).
    pub fn draw_order(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.collect(&self.canvas, &mut out, 0);
        out
    }

    fn collect(&self, id: &str, out: &mut Vec<String>, depth: usize) {
        let Some(o) = self.doc.get(id) else { return };
        if depth > 256 || o.boolean("hidden") {
            return;
        }
        out.push(id.to_string());
        for k in o.refs("subshape_refs").into_iter().rev() {
            self.collect(k, out, depth + 1);
        }
    }

    fn guard<T>(&self, key: &str, f: impl FnOnce() -> Option<T>) -> Option<T> {
        if !self.busy.borrow_mut().insert(key.to_string()) {
            return None; // cycle
        }
        let r = f();
        self.busy.borrow_mut().remove(key);
        r
    }

    // -----------------------------------------------------------------------
    // Positions

    pub fn resolve(&self, pid: &str) -> Option<Resolved> {
        self.guard(&format!("pos:{pid}"), || self.resolve_inner(pid))
    }

    fn resolve_inner(&self, pid: &str) -> Option<Resolved> {
        let o = self.obj(pid)?;
        let owner = self.owner.get(pid).map(String::as_str);
        let dirset = o.int_or("dirset", 0) as u8 & DIR_ALL;
        match o.type_id() {
            10 => self.resolve(o.str_("position_ref")?),
            11 => {
                let t = owner.map(|w| self.translation(w)).unwrap_or_default();
                Some(Resolved { p: o.point("point")?.add(t), dirs: dirset, rect: None })
            }
            12 => {
                let f = self.frame(owner?)?;
                let (p, fixed) = f.keypad(o.int("rect_position")?)?;
                Some(Resolved { p, dirs: if fixed != 0 { fixed } else { dirset }, rect: Some(f) })
            }
            17 => {
                let f = self.frame(owner?)?;
                let cp = o.point("content_point").unwrap_or_default();
                let origin = self.content_origin(owner?)?;
                if o.boolean("on_border") {
                    // On the border, content_point is (border part, offset): the same encoding as
                    // a rect's border_tixels (SPEC §1.7, §5.6), not a content-grid cell.
                    let (p, side) = border_part_point(f, origin, cp.x, cp.y);
                    return Some(Resolved { p, dirs: if dirset != 0 { dirset } else { side }, rect: Some(f) });
                }
                Some(Resolved { p: origin.add(cp), dirs: dirset, rect: Some(f) })
            }
            22 => {
                let (c, r) = self.diamond_geometry(owner?)?;
                let (p, d) = diamond_point(c, r, o.int_or("dpos", 1));
                let rect = Rect::new(c.x - r, c.y - r, 2 * r + 1, 2 * r + 1);
                Some(Resolved { p, dirs: if d != 0 { d } else { dirset }, rect: Some(rect) })
            }
            23 => {
                let (c, r) = self.diamond_geometry(owner?)?;
                let cp = o.point("dcpos_point").unwrap_or_default();
                let rect = Rect::new(c.x - r, c.y - r, 2 * r + 1, 2 * r + 1);
                Some(Resolved { p: Point::new(rect.x + cp.x, rect.y + cp.y), dirs: dirset, rect: Some(rect) })
            }
            24 => {
                let path = self.line_path(owner?)?;
                let n = path.cells.len();
                if n == 0 {
                    return None;
                }
                let idx = o.int_or("line_pt_idx", -1);
                let i = if idx >= 0 && (idx as usize) < n {
                    idx as usize
                } else {
                    let rel = o.float("line_pt_rel")?;
                    if !(0.0..=1.0).contains(&rel) {
                        return None;
                    }
                    (rel * (n - 1) as f64).round() as usize
                };
                Some(Resolved { p: path.cells[i], dirs: dirset, rect: None })
            }
            16 => {
                let line = o.str_("line_seg_pos_shape_ref").or(owner)?;
                let path = self.line_path(line)?;
                let p = match (o.int_or("line_seg_pos_type", 4), path.corners.len()) {
                    (1, n) if n >= 2 => mid(path.corners[0], path.corners[1]),
                    (2, n) if n >= 3 => path.corners[1],
                    (3, n) if n >= 3 => mid(path.corners[1], path.corners[2]),
                    _ => *path.cells.get(path.cells.len() / 2)?,
                };
                Some(Resolved { p, dirs: dirset, rect: None })
            }
            19 => {
                let path = self.line_path(owner?)?;
                let c = &path.corners;
                if c.len() < 2 {
                    return None;
                }
                let p = match (o.int_or("line_dia_seg", 0), o.int_or("line_dia_pos", 1)) {
                    (0, _) => *path.cells.get(path.cells.len() / 2)?,
                    (1, 4) => *c.get(1)?,
                    (1, _) => mid(c[0], *c.get(1)?),
                    (_, 4) => *c.get(c.len().saturating_sub(2))?,
                    _ => mid(*c.get(c.len().saturating_sub(2))?, *c.last()?),
                };
                Some(Resolved { p, dirs: dirset, rect: None })
            }
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Frames

    /// Absolute frame of a shape (rect family, ellipse, diamond, line bbox, group union).
    pub fn frame(&self, id: &str) -> Option<Rect> {
        if let Some(f) = self.frames.borrow().get(id) {
            return *f;
        }
        let f = self.guard(&format!("frame:{id}"), || self.frame_inner(id));
        self.frames.borrow_mut().insert(id.to_string(), f);
        f
    }

    fn frame_inner(&self, id: &str) -> Option<Rect> {
        let o = self.obj(id)?;
        match o.type_id() {
            25 => {
                let b = crate::geom::parse_rect(o.str_("img_bounds")?)?;
                Some(b.translate(self.translation(id)))
            }
            20 | 28 | 26 => {
                let size = o.point("frame_size")?;
                let anchor = match o.str_("frame_anchor_ref") {
                    Some(a) => self.resolve(a)?.p,
                    None => self.translation(id),
                };
                let anchor = anchor.add(o.point("frame_offset").unwrap_or_default());
                let off = Rect::keypad_offset(o.int_or("frame_corner", 1), size.x, size.y);
                Some(Rect::new(anchor.x - off.x, anchor.y - off.y, size.x, size.y))
            }
            21 => {
                let (c, r) = self.diamond_geometry(id)?;
                Some(Rect::new(c.x - r, c.y - r, 2 * r + 1, 2 * r + 1))
            }
            5 => {
                let p = self.line_path(id)?;
                p.cells.iter().map(|c| Rect::new(c.x, c.y, 1, 1)).reduce(|a, b| a.union(&b))
            }
            7 => {
                let cells = crate::render::triangle_cells(self, id, o);
                cells.iter().map(|(c, _)| Rect::new(c.x, c.y, 1, 1)).reduce(|a, b| a.union(&b))
            }
            _ => {
                let mut r: Option<Rect> = None;
                for k in o.refs("subshape_refs") {
                    if let Some(f) = self.frame(k) {
                        r = Some(r.map_or(f, |r| r.union(&f)));
                    }
                }
                r
            }
        }
    }

    /// Origin of a rect's content grid (tixmaps, content positions): the frame's anchor
    /// point (keypad `frame_corner`) plus `content_offset` (SPEC §1.7).
    pub fn content_origin(&self, id: &str) -> Option<Point> {
        let o = self.obj(id)?;
        let f = self.frame(id)?;
        let corner = o.int_or("frame_corner", 1);
        let anchor = if (1..=9).contains(&corner) { f.keypad(corner)?.0 } else { Point::new(f.x, f.y) };
        Some(anchor.add(o.point("content_offset").unwrap_or_default()))
    }

    /// Points a line end can attach to on `id` as (position value, cell): keypad 10–25 on a
    /// rect/ellipse frame (`rect_position`), the four outside points of a diamond (`dpos`).
    pub fn attach_points(&self, id: &str) -> Vec<(i64, Point)> {
        self.attach_points_from(id, &RECT_ATTACH_POINTS, &DIAMOND_ATTACH_POINTS)
    }

    /// The four outside mid-side / tip points used when a line end lands on a shape but not on
    /// one of its points: the end attaches to the one facing the line's other end.
    pub fn attach_side_points(&self, id: &str) -> Vec<(i64, Point)> {
        self.attach_points_from(id, &[12, 15, 16, 19], &DIAMOND_ATTACH_POINTS)
    }

    fn attach_points_from(&self, id: &str, rect: &[i64], diamond: &[i64]) -> Vec<(i64, Point)> {
        if self.obj(id).is_some_and(|o| o.type_id() == 21) {
            let Some((c, r)) = self.diamond_geometry(id) else { return vec![] };
            return diamond.iter().map(|&k| (k, diamond_point(c, r, k).0)).collect();
        }
        let Some(f) = self.frame(id) else { return vec![] };
        rect.iter().filter_map(|&k| f.keypad(k).map(|(q, _)| (k, q))).collect()
    }

    /// Whether a line end at `p` lands on `id`: inside its frame (inside the outline for a
    /// diamond) or on one of its attachment points.
    pub fn attach_hit(&self, id: &str, p: Point) -> bool {
        let inside = if self.obj(id).is_some_and(|o| o.type_id() == 21) {
            self.diamond_geometry(id).is_some_and(|(c, r)| (p.x - c.x).abs() + (p.y - c.y).abs() <= r)
        } else {
            self.frame(id).is_some_and(|f| f.contains(p))
        };
        inside || self.attach_points(id).iter().any(|&(_, q)| q == p)
    }

    /// Diamond centre and radius.
    pub fn diamond_geometry(&self, id: &str) -> Option<(Point, i32)> {
        let o = self.obj(id)?;
        let r = o.int_or("radius", 1).max(1) as i32;
        let anchor = match o.str_("anchor_point") {
            Some(a) => self.resolve(a)?.p,
            None => self.translation(id),
        };
        let c = match o.int_or("anchor_pos", 1) {
            2 => anchor.offset(0, r),
            3 => anchor.offset(0, r + 1),
            4 => anchor.offset(0, -r),
            5 => anchor.offset(0, -r - 1),
            6 => anchor.offset(r, 0),
            7 => anchor.offset(r + 1, 0),
            8 => anchor.offset(-r, 0),
            9 => anchor.offset(-r - 1, 0),
            _ => anchor,
        };
        Some((c, r))
    }

    // -----------------------------------------------------------------------
    // Lines

    pub fn line_path(&self, id: &str) -> Option<Rc<LinePath>> {
        if let Some(p) = self.paths.borrow().get(id) {
            return p.clone();
        }
        let p = self.guard(&format!("line:{id}"), || self.line_path_inner(id)).map(Rc::new);
        self.paths.borrow_mut().insert(id.to_string(), p.clone());
        p
    }

    fn line_path_inner(&self, id: &str) -> Option<LinePath> {
        let o = self.obj(id)?;
        let ends: Vec<End> = o
            .refs("segment_end_refs")
            .into_iter()
            .map(|e| {
                let r = self.resolve(e)?;
                let own = self.obj(e).map(|eo| eo.int_or("dirset", 0) as u8 & DIR_ALL).unwrap_or(0);
                Some(End { p: r.p, dirs: if r.dirs != 0 { r.dirs } else { own }, rect: r.rect })
            })
            .collect::<Option<Vec<_>>>()?;
        if ends.is_empty() {
            return None;
        }
        let line_type = o.int_or("line_type", 3);
        let mut path = LinePath::default();
        match line_type {
            1 | 5 => {
                let kind = if line_type == 1 { 1 } else { 2 };
                let n = ends.len();
                for i in 0..n.saturating_sub(1) {
                    let mut seg = route::free_cells(ends[i].p, ends[i + 1].p, kind);
                    if i == 0 {
                        path.first_seg_len = seg.len();
                    }
                    if i + 2 == n {
                        path.last_seg_len = seg.len();
                    } else {
                        seg.pop();
                    }
                    path.cells.extend(seg);
                }
                if n == 1 {
                    path.cells.push(ends[0].p);
                }
                path.corners = ends.iter().map(|e| e.p).collect();
            }
            2 | 4 => {
                let dirs: Vec<i64> = if line_type == 4 {
                    o.refs("segment_refs").iter().map(|s| self.obj(s).map(|so| so.int_or("direction", 1)).unwrap_or(1)).collect()
                } else {
                    route::automatic_directions(&ends)
                };
                let mut pts = vec![ends[0].p];
                for i in 0..ends.len().saturating_sub(1) {
                    if let Some(c) = route::l_corner(ends[i].p, ends[i + 1].p, *dirs.get(i).unwrap_or(&1)) {
                        pts.push(c);
                    }
                    pts.push(ends[i + 1].p);
                }
                path.corners = route::simplify(&pts);
                path.cells = route::ortho_cells(&path.corners);
                self.set_terminal_lengths(&mut path);
            }
            _ => {
                let corners = route::diagram_route(ends[0], *ends.last().unwrap());
                path.cells = route::ortho_cells(&corners);
                path.corners = corners;
                self.set_terminal_lengths(&mut path);
            }
        }
        // Drop consecutive duplicates (zero-length segments).
        path.cells.dedup();
        path.masks = route::path_masks(&path.cells);
        Some(path)
    }

    fn set_terminal_lengths(&self, path: &mut LinePath) {
        let c = &path.corners;
        let len = |a: Point, b: Point| ((a.x - b.x).abs() + (a.y - b.y).abs() + 1) as usize;
        if c.len() >= 2 {
            path.first_seg_len = len(c[0], c[1]);
            path.last_seg_len = len(c[c.len() - 2], c[c.len() - 1]);
        } else {
            path.first_seg_len = 1;
            path.last_seg_len = 1;
        }
    }
}

/// Cell of border part `part` at `off` on frame `f` (SPEC §1.7): 0 top, 1 bottom, 2 left, 3 right
/// side, offsets measured from the content origin `origin`; 4–7 the TL/TR/BL/BR corner; 8–15 the
/// same parts on the ring one cell outside the frame. Also returns the side's outward direction.
pub fn border_part_point(f: Rect, origin: Point, part: i32, off: i32) -> (Point, u8) {
    let co = Point::new(origin.x - f.x, origin.y - f.y);
    let r = if part >= 8 { f.inset(-1) } else { f };
    match part.rem_euclid(8) {
        0 => (Point::new(r.x + off + co.x, r.y), DIR_UP),
        1 => (Point::new(r.x + off + co.x, r.y1()), DIR_DOWN),
        2 => (Point::new(r.x, r.y + off + co.y), DIR_LEFT),
        3 => (Point::new(r.x1(), r.y + off + co.y), DIR_RIGHT),
        4 => (Point::new(r.x, r.y), DIR_UP | DIR_LEFT),
        5 => (Point::new(r.x1(), r.y), DIR_UP | DIR_RIGHT),
        6 => (Point::new(r.x, r.y1()), DIR_DOWN | DIR_LEFT),
        _ => (Point::new(r.x1(), r.y1()), DIR_DOWN | DIR_RIGHT),
    }
}

fn mid(a: Point, b: Point) -> Point {
    Point::new(a.x + (b.x - a.x) / 2, a.y + (b.y - a.y) / 2)
}

/// Outside attachment points of a rect/ellipse frame (keypad 10–25, SPEC §4).
pub const RECT_ATTACH_POINTS: [i64; 16] = [10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25];

/// Outside attachment points of a diamond (`dpos` one cell beyond each tip, SPEC §6.3).
pub const DIAMOND_ATTACH_POINTS: [i64; 4] = [3, 5, 7, 9];

/// Cell and fixed exit direction of diamond position `dpos` (SPEC §6.3): 1 centre, 2/4/6/8 the
/// top/bottom/left/right tip, 3/5/7/9 one cell outside it.
pub fn diamond_point(c: Point, r: i32, dpos: i64) -> (Point, u8) {
    match dpos {
        2 => (c.offset(0, -r), 0),
        3 => (c.offset(0, -r - 1), DIR_UP),
        4 => (c.offset(0, r), 0),
        5 => (c.offset(0, r + 1), DIR_DOWN),
        6 => (c.offset(-r, 0), 0),
        7 => (c.offset(-r - 1, 0), DIR_LEFT),
        8 => (c.offset(r, 0), 0),
        9 => (c.offset(r + 1, 0), DIR_RIGHT),
        _ => (c, 0),
    }
}
