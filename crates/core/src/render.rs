//! Drawing shapes into a tixel buffer (SPEC §4.1, §5, §6, §7).

use std::collections::HashSet;

use crate::format::{Obj, ObjExt, NO_COLOR};
use crate::geom::*;
use crate::scene::Scene;
use crate::text::{layout, TextStyle};
use crate::tixel::*;

/// Render every visible shape of the scene's canvas into a fresh buffer.
/// Line tixels are converted to glyphs once at the end.
pub fn render_scene(scene: &Scene) -> Buffer {
    let mut buf = Buffer::new();
    for id in scene.draw_order() {
        draw_shape(scene, &mut buf, &id);
    }
    buf.convert_lines(scene.unicode);
    buf
}

/// Render a subset of root shapes (and their descendants).
pub fn render_shapes(scene: &Scene, roots: &[String]) -> Buffer {
    let mut wanted = HashSet::new();
    for r in roots {
        let mut stack = vec![r.clone()];
        while let Some(id) = stack.pop() {
            if let Some(o) = scene.obj(&id) {
                stack.extend(o.refs("subshape_refs").into_iter().map(str::to_string));
            }
            wanted.insert(id);
        }
    }
    let mut buf = Buffer::new();
    for id in scene.draw_order().into_iter().filter(|id| wanted.contains(id)) {
        draw_shape(scene, &mut buf, &id);
    }
    buf.convert_lines(scene.unicode);
    buf
}

pub fn draw_shape(scene: &Scene, buf: &mut Buffer, id: &str) {
    let Some(o) = scene.obj(id) else { return };
    match o.type_id() {
        20 => draw_rect(scene, buf, id, o),
        28 => draw_ellipse(scene, buf, id, o),
        21 => draw_diamond(scene, buf, id, o),
        5 => draw_line(scene, buf, id, o),
        7 => draw_triangle(scene, buf, id, o),
        26 => draw_table(scene, buf, id, o),
        _ => {}
    }
    draw_position_tixels(scene, buf, o);
}

pub(crate) fn color(o: &Obj, key: &str) -> u64 {
    o.uint(key).unwrap_or(NO_COLOR)
}

fn border_style(variant: i64) -> Option<Option<u8>> {
    // Some(None) = opaque spaces, Some(Some(style)) = segments, None = nothing drawn.
    match variant {
        3 => Some(None),
        4 => Some(Some(STYLE_LIGHT)),
        5 => Some(Some(STYLE_HEAVY)),
        6 => Some(Some(STYLE_DOUBLE)),
        _ => None,
    }
}

/// Dash rule shared by lines and borders (SPEC §5.5).
pub fn dash_painted(i: i64, dash: Option<(i64, i64, i64)>) -> bool {
    let Some((p, u, o)) = dash else { return true };
    if p <= 0 || u <= 0 {
        return true;
    }
    let period = p + u;
    let phase = if o <= 0 { o } else { o.rem_euclid(period) - period };
    (i - phase).rem_euclid(period) < p
}

fn dash_of(v: Option<&serde_json::Value>) -> Option<(i64, i64, i64)> {
    let d = v?.as_object()?;
    let g = |k: &str| d.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
    Some((g("p"), g("u"), g("o")))
}

fn shadow_char(o: &Obj, unicode: bool) -> char {
    if let Some(c) = o.tixel_char("shadow_tixel") {
        return c;
    }
    let off = o.point("shadow_offset").unwrap_or_default();
    match (o.int_or("shadow_intensity", 4), unicode) {
        (1, true) => '░',
        (1, false) => '.',
        (2, true) => '▒',
        (2, false) => ':',
        (3, true) => '▓',
        (3, false) => {
            if off.x == 0 {
                '|'
            } else if off.y == 0 {
                '-'
            } else if off.x.signum() == off.y.signum() {
                '\\'
            } else {
                '/'
            }
        }
        (_, true) => '█',
        (_, false) => '*',
    }
}

pub(crate) fn draw_shadow(buf: &mut Buffer, o: &Obj, cells: &[Point], unicode: bool, mode: Blend) {
    if !o.boolean("shadow_enabled") {
        return;
    }
    let off = o.point("shadow_offset").unwrap_or_default();
    let ch = shadow_char(o, unicode);
    let col = color(o, "shadow_color");
    for p in cells {
        buf.set(p.add(off), Tixel::ch(ch), col, mode);
    }
}

pub(crate) fn background_char(o: &Obj, unicode: bool) -> Option<char> {
    if !o.boolean("background_enabled") {
        return None;
    }
    match o.int_or("background_type", 2) {
        2 => Some(' '),
        3 => o.tixel_char("background_tixel"),
        4 => Some(if unicode { '█' } else { '@' }),
        _ => None,
    }
}

fn rect_cells(r: Rect) -> impl Iterator<Item = Point> {
    (r.y..r.y + r.h).flat_map(move |y| (r.x..r.x + r.w).map(move |x| Point::new(x, y)))
}

// ---------------------------------------------------------------------------
// Rectangle family (SPEC §6.1)

fn draw_rect(scene: &Scene, buf: &mut Buffer, _id: &str, o: &Obj) {
    draw_rect_with(scene, buf, _id, o, true);
}

fn draw_rect_with(scene: &Scene, buf: &mut Buffer, _id: &str, o: &Obj, with_shadow: bool) {
    let Some(f) = scene.frame(_id) else { return };
    if f.is_empty() {
        return;
    }
    let unicode = scene.unicode;
    let border = o.boolean("rectangular_border_enabled");
    let mode = if o.boolean("content_blended") && border { Blend::Merge } else { Blend::Replace };
    buf.add_extent(f);

    if with_shadow && o.boolean("shadow_enabled") {
        // The shape is composed over its own shadow before it lands on the canvas: where the
        // shape writes a tixel the shadow never reaches the canvas, so a border still merges
        // with what was under it (golden craft `░┤ Plant!`); dash gaps show the shadow.
        let off = o.point("shadow_offset").unwrap_or_default();
        let mut own = Buffer::new();
        draw_rect_with(scene, &mut own, _id, o, false);
        let free = |q: Point| own.cells.get(&q).is_none_or(|c| c.t.is_null());
        let cells: Vec<Point> = rect_cells(f).filter(|p| free(p.add(off))).collect();
        draw_shadow(buf, o, &cells, scene.doc.unicode(), mode);
        buf.add_extent(f.translate(off));
    }

    let bg = background_char(o, unicode);
    let inner = if border { f.inset(1) } else { f };
    if let Some(c) = bg {
        let col = color(o, "mps_bckg_color");
        for p in rect_cells(inner) {
            buf.set(p, Tixel::ch(c), col, mode);
        }
    }

    if border && f.w >= 2 && f.h >= 2 {
        draw_rect_border(buf, o, f, bg.is_some(), mode);
    }
    draw_border_tixels(scene, buf, o, f, scene.content_origin(_id).unwrap_or(Point::new(f.x, f.y)), mode);

    if let Some(t) = o.str_("model_text").and_then(|t| scene.obj(t)) {
        let mut tr = if border && t.boolean("inside_border") { f.inset(1) } else { f };
        tr.x += t.int_or("inset_left", 0) as i32;
        tr.y += t.int_or("inset_top", 0) as i32;
        tr.w -= (t.int_or("inset_left", 0) + t.int_or("inset_right", 0)) as i32;
        tr.h -= (t.int_or("inset_top", 0) + t.int_or("inset_bottom", 0)) as i32;
        draw_text(buf, t, tr, mode);
    }

    // Freeform content is clipped to the interior: a border always wins over content under it.
    let content_origin = scene.content_origin(_id).unwrap_or(Point::new(f.x, f.y));
    if let Some(cells) = tixmap_cells(scene, o.str_("content_tixels")) {
        let col = color(o, "content_color");
        for (p, c) in cells {
            let p = content_origin.add(p);
            if !border || inner.contains(p) {
                buf.set(p, Tixel::ch(c), col, mode);
            }
        }
    }
}

fn draw_rect_border(buf: &mut Buffer, o: &Obj, f: Rect, background: bool, mode: Blend) {
    let Some(kind) = border_style(o.int_or("rectangular_border", 4)) else { return };
    // On disk the names are swapped: `border_dash` = enabled flag, `border_dash_enabled` = pattern.
    let dash = if o.boolean("border_dash") { dash_of(o.val("border_dash_enabled")) } else { None };
    let col = color(o, "mps_border_color");
    let (w, h) = (f.w as i64, f.h as i64);
    let mut put = |p: Point, idx: i64, mask: u8, inward: u8, key: &str| {
        if !dash_painted(idx, dash) {
            return;
        }
        let t = if let Some(c) = o.tixel_char(key) {
            Tixel::ch(c)
        } else {
            match kind {
                None => Tixel::ch(' '),
                Some(style) => {
                    let mut t = Tixel::segments(mask, style);
                    if background && inward != 0 {
                        t.add_segments(inward, STYLE_HIDDEN);
                    }
                    t
                }
            }
        };
        buf.set(p, t, col, mode);
    };
    let (x0, y0, x1, y1) = (f.x, f.y, f.x1(), f.y1());
    put(Point::new(x0, y0), 0, SEG_D | SEG_R, 0, "border_tixel_tl");
    put(Point::new(x1, y0), w - 1, SEG_D | SEG_L, 0, "border_tixel_tr");
    put(Point::new(x1, y1), w + h - 2, SEG_U | SEG_L, 0, "border_tixel_br");
    put(Point::new(x0, y1), 2 * w + h - 3, SEG_U | SEG_R, 0, "border_tixel_bl");
    for x in x0 + 1..x1 {
        let k = (x - x0) as i64;
        put(Point::new(x, y0), k, SEG_L | SEG_R, SEG_D, "border_tixel_ts");
        put(Point::new(x, y1), w + h - 2 + (w - 1 - k), SEG_L | SEG_R, SEG_U, "border_tixel_bs");
    }
    for y in y0 + 1..y1 {
        let k = (y - y0) as i64;
        put(Point::new(x1, y), w - 1 + k, SEG_U | SEG_D, SEG_L, "border_tixel_rs");
        put(Point::new(x0, y), 2 * w + h - 3 + (h - 1 - k), SEG_U | SEG_D, SEG_R, "border_tixel_ls");
    }
}

/// Per-cell border overrides (`border_tixels`, SPEC §1.7): point = (part, offset).
fn draw_border_tixels(scene: &Scene, buf: &mut Buffer, o: &Obj, f: Rect, origin: Point, mode: Blend) {
    let Some(cells) = tixmap_cells(scene, o.str_("border_tixels")) else { return };
    // Side offsets are measured from the content origin (anchor + content_offset).
    let co = Point::new(origin.x - f.x, origin.y - f.y);
    for (pt, ch) in cells {
        let (part, off) = (pt.x, pt.y);
        let r = if part >= 8 { f.inset(-1) } else { f };
        let p = match part % 8 {
            0 => Point::new(r.x + off + co.x, r.y),
            1 => Point::new(r.x + off + co.x, r.y1()),
            2 => Point::new(r.x, r.y + off + co.y),
            3 => Point::new(r.x1(), r.y + off + co.y),
            4 => Point::new(r.x, r.y),
            5 => Point::new(r.x1(), r.y),
            6 => Point::new(r.x, r.y1()),
            _ => Point::new(r.x1(), r.y1()),
        };
        buf.set(p, Tixel::ch(ch), color(o, "mps_border_color"), mode);
    }
}

pub(crate) fn draw_text(buf: &mut Buffer, t: &Obj, rect: Rect, mode: Blend) {
    if !t.boolean("enabled") || rect.is_empty() {
        return;
    }
    let text = t.str_("text").unwrap_or("");
    let style = TextStyle {
        sweep: t.int_or("sweep", 0),
        movement: t.int_or("movement", 0),
        alignment: t.int_or("alignment", 1),
        position: t.int_or("position", 1),
    };
    let col = color(t, "color");
    if let Some(font) = t.str_("letter_font_id") {
        if let Some(cells) = crate::figlet::render_in_rect(font, text, rect, style) {
            for (p, c) in cells {
                buf.set(p, Tixel::ch(c), col, mode);
            }
        }
        return;
    }
    for (p, c) in layout(text, rect, style) {
        buf.set(p, Tixel::ch(c), col, mode);
    }
}

/// Cells of a tixmap object (SPEC §1.7), in its own grid.
pub fn tixmap_cells(scene: &Scene, id: Option<&str>) -> Option<Vec<(Point, char)>> {
    let t = scene.obj(id?)?;
    let tix = t.val("tixels")?.as_object()?;
    let content = tix.get("text_content")?.as_array()?;
    let points = tix.get("text_points")?.as_array()?;
    if content.len() != points.len() {
        return None;
    }
    Some(
        content
            .iter()
            .zip(points)
            .filter_map(|(c, p)| {
                let s = c.as_str()?;
                let units = s.encode_utf16().count();
                if !(1..=2).contains(&units) {
                    return None;
                }
                Some((parse_point(p.as_str()?)?, s.chars().next()?))
            })
            .collect(),
    )
}

fn draw_tixmap(scene: &Scene, buf: &mut Buffer, id: Option<&str>, origin: Point, col: u64, mode: Blend) {
    if let Some(cells) = tixmap_cells(scene, id) {
        for (p, c) in cells {
            buf.set(origin.add(p), Tixel::ch(c), col, mode);
        }
    }
}

fn draw_position_tixels(scene: &Scene, buf: &mut Buffer, o: &Obj) {
    for pid in o.refs("position_refs") {
        let Some(po) = scene.obj(pid) else { continue };
        if let Some(c) = po.tixel_char("tixel") {
            if let Some(r) = scene.resolve(pid) {
                buf.set(r.p, Tixel::ch(c), color(o, "content_color"), Blend::Replace);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Lines (SPEC §5)

fn head_glyph(head: i64, d: usize, unicode: bool) -> Option<char> {
    // d: 0 right, 1 left, 2 down, 3 up.
    let pick = |u: [char; 4], a: [char; 4]| Some(if unicode { u[d] } else { a[d] });
    let arrows = ['>', '<', 'v', '^'];
    match head {
        1 => pick(['▶', '◀', '▼', '▲'], arrows),
        2 => pick(['▷', '◁', '▽', '△'], arrows),
        3 => Some(if unicode { '■' } else { '#' }),
        4 => Some(if unicode { '□' } else { '#' }),
        5 => Some(if unicode { '▣' } else { '#' }),
        6 => Some(if unicode { '◆' } else { '*' }),
        7 => Some(if unicode { '◇' } else { '*' }),
        8 => Some(if unicode { '◈' } else { '*' }),
        9 => Some(if unicode { '●' } else { '.' }),
        10 => Some(if unicode { '○' } else { '.' }),
        11 => Some(if unicode { '◌' } else { '.' }),
        12 => Some(if unicode { '◎' } else { '.' }),
        13 => Some(if unicode { '◍' } else { '.' }),
        20 => Some(if unicode { '▲' } else { '^' }),
        21 => Some(if unicode { '▼' } else { 'v' }),
        22 => Some(if unicode { '◀' } else { '<' }),
        23 => Some(if unicode { '▶' } else { '>' }),
        24 => Some(if unicode { '△' } else { '^' }),
        25 => Some(if unicode { '▽' } else { 'v' }),
        26 => Some(if unicode { '◁' } else { '<' }),
        27 => Some(if unicode { '▷' } else { '>' }),
        _ => None,
    }
}

/// Head direction index from `prev` to `end` (0 right, 1 left, 2 down, 3 up).
fn head_dir(prev: Point, end: Point) -> usize {
    let (dx, dy) = (end.x - prev.x, end.y - prev.y);
    if dx > 0 && dy == 0 {
        0
    } else if dx < 0 && dy == 0 {
        1
    } else if dy < 0 && dx == 0 {
        3
    } else if dy > 0 && dx == 0 {
        2
    } else if dx.abs() > dy.abs() {
        if dx > 0 { 0 } else { 1 }
    } else if dy < 0 {
        3
    } else {
        2
    }
}

fn draw_line(scene: &Scene, buf: &mut Buffer, id: &str, o: &Obj) {
    let Some(path) = scene.line_path(id) else { return };
    let unicode = scene.unicode;
    let mode = if o.boolean("content_blended") { Blend::Merge } else { Blend::Replace };
    let col = color(o, "content_color");
    let dash = if o.boolean("line_dash_enabled") { dash_of(o.val("line_dash")) } else { None };
    let style = o.int_or("line_style", 3);
    for (i, (&p, &m)) in path.cells.iter().zip(path.masks.iter()).enumerate() {
        buf.add_extent(Rect::new(p.x, p.y, 1, 1));
        if !dash_painted(i as i64, dash) {
            continue;
        }
        let t = match style {
            1 => Tixel::ch(' '),
            3 => Tixel::segments(m, STYLE_LIGHT),
            4 => Tixel::segments(m, STYLE_HEAVY),
            5 => Tixel::segments(m, STYLE_DOUBLE),
            _ => continue,
        };
        buf.set(p, t, col, mode);
    }

    // Heads on the first and last segment ends.
    let ends = o.refs("segment_end_refs");
    let n = path.cells.len();
    if n >= 2 && ends.len() >= 2 {
        let heads = [
            (ends[0], path.cells[0], path.cells[1], path.first_seg_len, "line_head_start_tixel"),
            (ends[ends.len() - 1], path.cells[n - 1], path.cells[n - 2], path.last_seg_len, "line_head_end_tixel"),
        ];
        for (eid, end, prev, seg_len, custom_key) in heads {
            let head = scene.obj(eid).map(|e| e.int_or("head_type", 0)).unwrap_or(0);
            let glyph = o.tixel_char(custom_key).or_else(|| {
                if seg_len < 2 {
                    return None;
                }
                // Arrow and marker heads follow the document's alphabet, not a forced
                // ASCII export: real Monodraw keeps ▶▼ under the CLI's ASCII output.
                head_glyph(head, head_dir(prev, end), scene.doc.unicode())
            });
            if let Some(g) = glyph {
                buf.set(end, Tixel::ch(g), col, mode);
            } else if (14..=19).contains(&head) && seg_len >= 2 {
                draw_er_head(buf, head, end, prev, o, unicode, col, mode);
            }
        }
    }

    if let Some(cells) = tixmap_cells(scene, o.str_("content_tixmap_ref")) {
        for (p, c) in cells {
            if let Some(&cell) = path.cells.get(p.x.max(0) as usize).filter(|_| p.x >= 0) {
                buf.set(cell, Tixel::ch(c), col, mode);
            }
        }
    }
}

/// Entity-relationship heads (SPEC §5.7). The per-direction layout is only partly
/// known; this draws the bar/circle marks along the line and a simple crow's foot.
#[allow(clippy::too_many_arguments)]
fn draw_er_head(buf: &mut Buffer, head: i64, end: Point, prev: Point, o: &Obj, unicode: bool, col: u64, mode: Blend) {
    let d = dir_between(prev, end);
    if d == 0 {
        return;
    }
    let back = dir_reverse(d);
    let horizontal = dir_is_horizontal(d);
    let heavy = o.int_or("line_style", 3) == 4;
    let bar = if unicode {
        Tixel::segments(SEG_U | SEG_D | SEG_L | SEG_R, if heavy { STYLE_HEAVY } else { STYLE_LIGHT })
    } else {
        Tixel::ch(if horizontal { '|' } else { '-' })
    };
    let circle = Tixel::ch(if unicode { '○' } else { 'o' });
    // Notation only changes the "many" fork (Monodraw 1.6.1 GUI captures,
    // fixtures/golden-gui 03-er-crowsfoot): One / One Only / Zero or One look the same in all three.
    let many = matches!(head, 15 | 18 | 19);
    let notation = o.int_or("head_notation", 2);
    if many {
        // Fork arms sit beside the tip: (a, b) are the cells above/below (horizontal) or left/right.
        let (a, b) = if horizontal { (DIR_UP, DIR_DOWN) } else { (DIR_LEFT, DIR_RIGHT) };
        match notation {
            // Hat: a filled triangle at the tip pointing back along the line.
            3 => {
                let g = head_glyph(1, head_dir(end, prev), unicode).unwrap_or('<');
                buf.set(end, Tixel::ch(g), col, Blend::Replace);
            }
            // Orthogonal: a cross at the tip with corner arms opening away from the line.
            1 => {
                let (ma, mb) = match d {
                    DIR_RIGHT => (SEG_R | SEG_D, SEG_R | SEG_U),
                    DIR_LEFT => (SEG_L | SEG_D, SEG_L | SEG_U),
                    DIR_DOWN => (SEG_R | SEG_D, SEG_L | SEG_D),
                    _ => (SEG_R | SEG_U, SEG_L | SEG_U),
                };
                let style = if heavy { STYLE_HEAVY } else { STYLE_LIGHT };
                buf.set(end, Tixel::segments(SEG_U | SEG_D | SEG_L | SEG_R, style), col, Blend::Replace);
                buf.set(end.step(a, 1), Tixel::segments(ma, style), col, Blend::Replace);
                buf.set(end.step(b, 1), Tixel::segments(mb, style), col, Blend::Replace);
            }
            // Diagonal (default): the arms spread toward the tip, e.g. ╱ above and ╲ below a rightward end.
            _ => {
                let (ca, cb) = match (unicode, d) {
                    (true, DIR_RIGHT | DIR_DOWN) => ('╱', '╲'),
                    (true, _) => ('╲', '╱'),
                    (false, DIR_RIGHT | DIR_DOWN) => ('/', '\\'),
                    (false, _) => ('\\', '/'),
                };
                buf.set(end.step(a, 1), Tixel::ch(ca), col, Blend::Replace);
                buf.set(end.step(b, 1), Tixel::ch(cb), col, Blend::Replace);
            }
        }
    }
    // Marks as (cells back from the tip, glyph).
    let marks: &[(i32, Tixel)] = match (head, notation) {
        (14, _) => &[(0, bar)],
        (16, _) => &[(0, bar), (1, bar)],
        (17, _) => &[(0, bar), (1, circle)],
        // Diagonal keeps a free cell between bar and fork on horizontal ends only.
        (18, 2) if horizontal => &[(2, bar)],
        (18, _) => &[(1, bar)],
        (19, _) => &[(1, circle)],
        _ => &[],
    };
    for &(k, t) in marks {
        buf.set(end.step(back, k), t, col, mode);
    }
}

// ---------------------------------------------------------------------------
// Ellipse (SPEC §6.2): crate::ellipse

fn draw_ellipse(scene: &Scene, buf: &mut Buffer, id: &str, o: &Obj) {
    crate::ellipse::draw_ellipse(scene, buf, id, o)
}

// ---------------------------------------------------------------------------
// Diamond (SPEC §6.3)

fn draw_diamond(scene: &Scene, buf: &mut Buffer, id: &str, o: &Obj) {
    let Some((c, r)) = scene.diamond_geometry(id) else { return };
    let unicode = scene.unicode;
    let mode = Blend::Replace;
    let rect = Rect::new(c.x - r, c.y - r, 2 * r + 1, 2 * r + 1);
    buf.add_extent(rect);
    let mut interior = Vec::new();
    let mut outline = Vec::new();
    for dy in -r..=r {
        let half = r - dy.abs();
        for dx in -half..=half {
            let p = c.offset(dx, dy);
            if dx.abs() == half {
                outline.push(p);
            } else {
                interior.push(p);
            }
        }
    }
    let mut all = interior.clone();
    all.extend(&outline);
    draw_shadow(buf, o, &all, scene.doc.unicode(), mode);
    if let Some(ch) = background_char(o, unicode) {
        for p in &interior {
            buf.set(*p, Tixel::ch(ch), color(o, "mps_bckg_color"), mode);
        }
    }
    if o.boolean("rectangular_border_enabled") {
        let col = color(o, "mps_border_color");
        let g = |key: &str, u: char, a: char| o.tixel_char(key).unwrap_or(if unicode { u } else { a });
        for k in 1..r {
            buf.set(c.offset(-k, -r + k), Tixel::ch(g("btd_ul", '╱', '/')), col, mode);
            buf.set(c.offset(k, -r + k), Tixel::ch(g("btd_ur", '╲', '\\')), col, mode);
            buf.set(c.offset(-k, r - k), Tixel::ch(g("btd_ll", '╲', '\\')), col, mode);
            buf.set(c.offset(k, r - k), Tixel::ch(g("btd_lr", '╱', '/')), col, mode);
        }
        buf.set(c.offset(0, -r), Tixel::ch(g("bt_tp", 'Λ', 'X')), col, mode);
        buf.set(c.offset(0, r), Tixel::ch(g("bt_bp", 'V', 'V')), col, mode);
        buf.set(c.offset(-r, 0), Tixel::ch(g("bt_lp", '▕', '|')), col, mode);
        buf.set(c.offset(r, 0), Tixel::ch(g("bt_rp", '▏', '|')), col, mode);
    }
    draw_tixmap(scene, buf, o.str_("content_tixels"), Point::new(rect.x, rect.y), color(o, "content_color"), mode);
}

// ---------------------------------------------------------------------------
// Triangle (SPEC §6.4)

pub fn triangle_cells(scene: &Scene, id: &str, o: &Obj) -> Vec<(Point, char)> {
    let Some(t) = o.val("triangle").and_then(|v| v.as_object()) else { return vec![] };
    let g = |k: &str| t.get(k).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let kind = g("type");
    let len = g("length");
    let len = len.signum() * (len.abs() / 2 * 2);
    let origin = Point::new(g("origin_x"), g("origin_y")).add(scene.translation(id));
    let n = len.abs();
    let s = len.signum().max(1) * if len < 0 { -1 } else { 1 };
    let mut out = Vec::new();
    for i in 0..=n {
        let h = i.min(n - i);
        let along = s * i;
        let base_char = if i == 0 || i == n { '*' } else if kind <= 1 { '-' } else { '|' };
        let (bp, dp, dc) = match kind {
            0 => (origin.offset(along, 0), origin.offset(along, -h), if i * 2 < n { '/' } else { '\\' }),
            1 => (origin.offset(along, 0), origin.offset(along, h), if i * 2 < n { '\\' } else { '/' }),
            2 => (origin.offset(0, along), origin.offset(-h, along), if i * 2 < n { '/' } else { '\\' }),
            _ => (origin.offset(0, along), origin.offset(h, along), if i * 2 < n { '\\' } else { '/' }),
        };
        out.push((bp, base_char));
        if h > 0 {
            out.push((dp, dc));
        }
    }
    out
}

fn draw_triangle(scene: &Scene, buf: &mut Buffer, id: &str, o: &Obj) {
    for (p, c) in triangle_cells(scene, id, o) {
        buf.set(p, Tixel::ch(c), color(o, "content_color"), Blend::Replace);
    }
}

// ---------------------------------------------------------------------------
// Tables (SPEC §6.5)

/// Shadow, background, then per cell its Single Light border and its text. Content blend mode
/// is 0, so shared borders merge into junctions. Merged cells are never persisted, so every cell
/// is drawn on its own.
fn draw_table(scene: &Scene, buf: &mut Buffer, id: &str, o: &Obj) {
    let Some(f) = scene.frame(id) else { return };
    if f.is_empty() {
        return;
    }
    let unicode = scene.unicode;
    let mode = Blend::Merge;
    buf.add_extent(f);
    let all: Vec<Point> = rect_cells(f).collect();
    draw_shadow(buf, o, &all, scene.doc.unicode(), mode);
    if o.boolean("shadow_enabled") {
        buf.add_extent(f.translate(o.point("shadow_offset").unwrap_or_default()));
    }
    // Background inside the outer border, so the outer border still merges with what's below.
    if let Some(c) = background_char(o, unicode) {
        let col = color(o, "mps_bckg_color");
        for p in rect_cells(f.inset(1)) {
            buf.set(p, Tixel::ch(c), col, mode);
        }
    }
    let Some(grid) = scene.table_grid(id) else { return };
    let col = color(o, "mps_border_color");
    for r in 0..grid.rows() {
        for c in 0..grid.cols() {
            let cr = grid.cell_rect(r, c);
            let (x0, y0, x1, y1) = (cr.x, cr.y, cr.x1(), cr.y1());
            let mut put = |p: Point, mask: u8| buf.set(p, Tixel::segments(mask, STYLE_LIGHT), col, mode);
            put(Point::new(x0, y0), SEG_D | SEG_R);
            put(Point::new(x1, y0), SEG_D | SEG_L);
            put(Point::new(x0, y1), SEG_U | SEG_R);
            put(Point::new(x1, y1), SEG_U | SEG_L);
            for x in x0 + 1..x1 {
                put(Point::new(x, y0), SEG_L | SEG_R);
                put(Point::new(x, y1), SEG_L | SEG_R);
            }
            for y in y0 + 1..y1 {
                put(Point::new(x0, y), SEG_U | SEG_D);
                put(Point::new(x1, y), SEG_U | SEG_D);
            }
            let text = scene
                .doc
                .table_cell(id, r, c)
                .and_then(|cell| scene.obj(&cell)?.str_("text_ref").map(str::to_string))
                .and_then(|t| scene.obj(&t));
            if let Some(t) = text {
                draw_text(buf, t, grid.content_rect(r, c), mode);
            }
        }
    }
}
