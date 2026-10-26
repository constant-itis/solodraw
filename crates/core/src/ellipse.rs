//! Ellipse rasterization (SPEC §6.2). Clean-room: written from SPEC.md and the golden data only.

use crate::format::{Obj, ObjExt};
use crate::geom::{Point, Rect};
use crate::render::{background_char, color, draw_shadow, draw_text};
use crate::scene::Scene;
use crate::tixel::*;

/// Cell size in points used for the outline geometry. 13 is the Menlo 11 line height
/// (golden-gui 04). The width is fitted to golden-gui 02: widths of about 6.6241…6.6266 give
/// the same 323 of 324 outlines (Menlo 11's own advance, ≈6.6226, gives 322).
pub const CELL_W: f64 = 6.625;
pub const CELL_H: f64 = 13.0;
/// Samples taken around the ellipse outline.
const SAMPLES: usize = 5760;

/// Hit-cell grid of a w×h ellipse, in frame-local cells.
#[derive(Clone, Debug)]
pub struct Hits {
    pub w: i32,
    pub h: i32,
    cells: Vec<bool>,
}

impl Hits {
    fn new(w: i32, h: i32) -> Self {
        Hits { w, h, cells: vec![false; (w.max(0) * h.max(0)) as usize] }
    }

    /// Whether (x, y) is a hit cell; anything outside the frame is not.
    pub fn get(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h && self.cells[(y * self.w + x) as usize]
    }

    fn set(&mut self, x: i32, y: i32, v: bool) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.cells[(y * self.w + x) as usize] = v;
        }
    }

    /// Maximal horizontal runs of hit cells in row y, as (x0, x1) inclusive.
    pub fn runs(&self, y: i32) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        let mut x = 0;
        while x < self.w {
            if self.get(x, y) {
                let x0 = x;
                while self.get(x + 1, y) {
                    x += 1;
                }
                out.push((x0, x));
            }
            x += 1;
        }
        out
    }

    /// Hit cells in row-major order.
    pub fn points(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        (0..self.h).flat_map(move |y| (0..self.w).filter(move |&x| self.get(x, y)).map(move |x| (x, y)))
    }

    /// Cells strictly between the leftmost and rightmost hit cell of each row that are not hits.
    pub fn interior(&self) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        for y in 0..self.h {
            let row: Vec<i32> = (0..self.w).filter(|&x| self.get(x, y)).collect();
            if let (Some(&l), Some(&r)) = (row.first(), row.last()) {
                out.extend((l + 1..r).filter(|&x| !self.get(x, y)).map(|x| (x, y)));
            }
        }
        out
    }
}

/// The outline cells of a w×h ellipse (SPEC §6.2: sampling, then the two trim passes).
pub fn hit_cells(w: i32, h: i32) -> Hits {
    let mut g = Hits::new(w, h);
    if w <= 0 || h <= 0 {
        return g;
    }
    if h == 3 {
        g.set(0, 1, true);
        g.set(w - 1, 1, true);
        for x in 1..w - 1 {
            g.set(x, 0, true);
            g.set(x, 2, true);
        }
        return g;
    }
    if w == 3 {
        g.set(1, 0, true);
        g.set(1, h - 1, true);
        for y in 1..h - 1 {
            g.set(0, y, true);
            g.set(2, y, true);
        }
        return g;
    }
    if w < 3 || h < 3 {
        // Degenerate: every cell is outline.
        for y in 0..h {
            for x in 0..w {
                g.set(x, y, true);
            }
        }
        return g;
    }

    // Pixel space, y up: the frame spans x 0…W, y −H…0.
    let (wp, hp) = (w as f64 * CELL_W, h as f64 * CELL_H);
    let a = (wp - CELL_W) / 2.0;
    let b = (hp - CELL_H) / 2.0;
    let (cx, cy) = (wp / 2.0, -hp / 2.0);
    for i in 0..SAMPLES {
        let t = i as f64 * std::f64::consts::TAU / SAMPLES as f64;
        let (s, c) = t.sin_cos();
        let r = a * b / ((b * c).powi(2) + (a * s).powi(2)).sqrt();
        let x = cx + r * c;
        let y = cy + r * s;
        g.set((x / CELL_W).floor() as i32, (-y / CELL_H).floor() as i32, true);
    }

    // Pass 1: a run's end cells one row down are dropped when the cell beyond them is also hit.
    for y in 0..h {
        for (x0, x1) in g.runs(y) {
            if g.get(x0 - 1, y + 1) {
                g.set(x0, y + 1, false);
            }
            if g.get(x1 + 1, y + 1) {
                g.set(x1, y + 1, false);
            }
        }
    }
    // Pass 2: a run's end cell is dropped when its inner neighbour and a vertical neighbour are hit.
    for y in 0..h {
        for (x0, x1) in g.runs(y) {
            if g.get(x0 + 1, y) && (g.get(x0, y + 1) || g.get(x0, y - 1)) {
                g.set(x0, y, false);
            }
            if g.get(x1 - 1, y) && (g.get(x1, y + 1) || g.get(x1, y - 1)) {
                g.set(x1, y, false);
            }
        }
    }
    g
}

/// What an outline cell shows before alphabet conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Char(char),
    /// `─`, with a hidden segment pointing toward the inside (down in the top half).
    Horizontal { inward_down: bool },
    /// `│`, with a hidden segment pointing toward the inside (right on the left side).
    Vertical { inward_right: bool },
    /// `╱` / `/`.
    Rising,
    /// `╲` / `\`.
    Falling,
}

/// Glyph for every outline cell (SPEC §6.2 glyph rules), in frame-local cells.
pub fn outline_glyphs(g: &Hits) -> Vec<((i32, i32), Glyph)> {
    let mut out: Vec<((i32, i32), Glyph)> = Vec::new();
    let (w, h) = (g.w, g.h);
    for y in 0..h {
        let above = |x: i32| g.get(x, y - 1);
        let below = |x: i32| g.get(x, y + 1);
        let horiz = Glyph::Horizontal { inward_down: y <= h / 2 };
        for (x0, x1) in g.runs(y) {
            if x0 == x1 {
                let x = x0;
                let single = if above(x + 1) && below(x - 1) {
                    Some(Glyph::Rising)
                } else if above(x - 1) && below(x + 1) {
                    Some(Glyph::Falling)
                } else if above(x) && below(x) {
                    Some(Glyph::Vertical { inward_right: 2 * x < w - 1 })
                } else if (above(x) && below(x - 1)) || (above(x + 1) && below(x)) {
                    Some(Glyph::Char(';'))
                } else if (above(x) && below(x + 1)) || (above(x - 1) && below(x)) {
                    Some(Glyph::Char(':'))
                } else if above(x + 1) && below(x + 1) {
                    Some(Glyph::Char('('))
                } else if above(x - 1) && below(x - 1) {
                    Some(Glyph::Char(')'))
                } else {
                    None
                };
                if let Some(s) = single {
                    out.push(((x, y), s));
                    continue;
                }
            }
            let n = (x1 - x0 + 1) as usize;
            let rising = above(x1 + 1) && below(x0 - 1);
            let falling = above(x0 - 1) && below(x1 + 1);
            let ch = Glyph::Char;
            // Written first cell, middle, last cell: later writes win (a lone cap shows its last char).
            let ends = |first: char, last: char| {
                let mut r = vec![horiz; n];
                r[0] = ch(first);
                r[n - 1] = ch(last);
                r
            };
            let row: Vec<Glyph> = if rising && n <= 3 {
                ends(',', '\'')
            } else if falling && n == 3 {
                ends('\'', '.')
            } else if falling && n <= 2 {
                ends('`', '.')
            } else if rising {
                let mut r = ends('_', '\'');
                r[1] = ch('.');
                r
            } else if falling {
                ends('`', '.')
            } else if below(x0 - 1) && below(x1 + 1) {
                ends('.', '.')
            } else if above(x0 - 1) && above(x1 + 1) {
                ends('`', '\'')
            } else {
                vec![ch('.'); n]
            };
            for (i, gl) in row.into_iter().enumerate() {
                out.push(((x0 + i as i32, y), gl));
            }
        }
    }
    out
}

fn glyph_tixel(g: Glyph) -> Tixel {
    match g {
        Glyph::Char(c) => Tixel::ch(c),
        Glyph::Horizontal { inward_down } => {
            let mut t = Tixel::segments(SEG_L | SEG_R, STYLE_LIGHT);
            t.add_segments(if inward_down { SEG_D } else { SEG_U }, STYLE_HIDDEN);
            t
        }
        Glyph::Vertical { inward_right } => {
            let mut t = Tixel::segments(SEG_U | SEG_D, STYLE_LIGHT);
            t.add_segments(if inward_right { SEG_R } else { SEG_L }, STYLE_HIDDEN);
            t
        }
        Glyph::Rising => Tixel::segments(SEG_UR | SEG_DL, STYLE_LIGHT),
        Glyph::Falling => Tixel::segments(SEG_UL | SEG_DR, STYLE_LIGHT),
    }
}

/// Text rect of an ellipse (SPEC §6.2), in frame-local cells; `None` when nothing fits.
pub fn text_rect(g: &Hits, control: f64) -> Option<Rect> {
    let (w, h) = (g.w, g.h);
    if w < 3 || h < 3 {
        return None;
    }
    let c = if control.is_finite() { control.clamp(0.0, 1.0) } else { 0.25 };
    let theta = c * std::f64::consts::FRAC_PI_2;
    let (wp, hp) = (w as f64 * CELL_W, h as f64 * CELL_H);
    let a = (wp - CELL_W) / 2.0;
    let b = (hp - CELL_H) / 2.0;
    // At the slider ends the rect would collapse; it always keeps the centre row / column.
    let (hw, hh) = ((a * theta.cos()).max(CELL_W / 2.0), (b * theta.sin()).max(CELL_H / 2.0));
    let inside = |k: i32, cell: f64, centre: f64, half: f64| ((k as f64 + 0.5) * cell - centre).abs() <= half + 1e-9;
    let cols: Vec<i32> = (0..w).filter(|&x| inside(x, CELL_W, wp / 2.0, hw)).collect();
    let rows: Vec<i32> = (0..h).filter(|&y| inside(y, CELL_H, hp / 2.0, hh)).collect();
    let (cx0, cx1) = (*cols.first()?, *cols.last()?);
    let (mut y0, mut y1) = (*rows.first()?, *rows.last()?);
    // Room inside the outline on row y: the gap between hits around the centre column(s),
    // bounded by a hit on both sides.
    let span = |y: i32| -> Option<(i32, i32)> {
        let (ml, mr) = ((w - 1) / 2, w / 2);
        if (ml..=mr).any(|x| g.get(x, y)) {
            return None;
        }
        let left = (0..ml).rev().find(|&x| g.get(x, y))?;
        let right = (mr + 1..w).find(|&x| g.get(x, y))?;
        Some((left + 1, right - 1))
    };
    // Keep to the interior of every row used, dropping end rows (symmetrically) that leave no room.
    while y0 <= y1 {
        let (mut x0, mut x1) = (cx0, cx1);
        let mut fits = true;
        for y in y0..=y1 {
            match span(y) {
                Some((l, r)) => {
                    x0 = x0.max(l);
                    x1 = x1.min(r);
                }
                None => fits = false,
            }
        }
        if fits && x1 >= x0 {
            return Some(Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1));
        }
        y0 += 1;
        y1 -= 1;
    }
    None
}

pub fn draw_ellipse(scene: &Scene, buf: &mut Buffer, id: &str, o: &Obj) {
    let Some(f) = scene.frame(id) else { return };
    if f.is_empty() {
        return;
    }
    let unicode = scene.unicode;
    let border = o.boolean("rectangular_border_enabled");
    let mode = if o.boolean("content_blended") && border { Blend::Merge } else { Blend::Replace };
    buf.add_extent(f);

    let g = hit_cells(f.w, f.h);
    let at = |(x, y): (i32, i32)| Point::new(f.x + x, f.y + y);
    let interior: Vec<Point> = g.interior().into_iter().map(at).collect();

    // Shadow: outline and interior, shifted.
    let mut all: Vec<Point> = g.points().map(at).collect();
    all.extend(&interior);
    // The shadow glyph follows the document's alphabet, not the export alphabet.
    draw_shadow(buf, o, &all, scene.doc.unicode(), mode);
    if o.boolean("shadow_enabled") {
        buf.add_extent(f.translate(o.point("shadow_offset").unwrap_or_default()));
    }

    if let Some(c) = background_char(o, unicode) {
        let col = color(o, "mps_bckg_color");
        for &p in &interior {
            buf.set(p, Tixel::ch(c), col, mode);
        }
    }

    if border {
        let col = color(o, "mps_border_color");
        let custom = o.tixel_char("border_tixel");
        for (p, gl) in outline_glyphs(&g) {
            let t = match custom {
                Some(c) => Tixel::ch(c),
                None => glyph_tixel(gl),
            };
            buf.set(at(p), t, col, mode);
        }
    }

    let text = o.str_("model_text_ref").or_else(|| o.str_("model_text")).and_then(|t| scene.obj(t));
    if let Some(t) = text {
        if let Some(r) = text_rect(&g, o.float("text_container_control").unwrap_or(0.25)) {
            let mut tr = Rect::new(f.x + r.x, f.y + r.y, r.w, r.h);
            tr.x += t.int_or("inset_left", 0) as i32;
            tr.y += t.int_or("inset_top", 0) as i32;
            tr.w -= (t.int_or("inset_left", 0) + t.int_or("inset_right", 0)) as i32;
            tr.h -= (t.int_or("inset_top", 0) + t.int_or("inset_bottom", 0)) as i32;
            draw_text(buf, t, tr, mode);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::TextOptions;
    use crate::format::Document;
    use serde_json::json;

    fn render(w: i32, h: i32, text: &str, control: f64) -> String {
        let (mut doc, canvas) = Document::new_empty();
        let id = doc.add_ellipse(&canvas, Rect::new(0, 0, w, h));
        let tid = doc.text_of(&id).unwrap();
        doc.set_value(&tid, "text", json!(text));
        doc.set_value(&id, "text_container_control", json!(control));
        let o = TextOptions { trim_padding: true, trim_trailing: true, unicode: Some(true), ..Default::default() };
        crate::canvas_to_text(&doc, None, &o).unwrap_or_default()
    }

    #[test]
    fn text_never_covers_the_outline() {
        for (w, h) in [(20, 9), (9, 5), (5, 4), (4, 6), (12, 3), (3, 7), (16, 13)] {
            let empty = render(w, h, "", 0.25);
            for c in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let s = render(w, h, "Hello wide world of ellipses", c);
                for (a, b) in empty.lines().zip(s.lines()) {
                    for (x, y) in a.chars().zip(b.chars()) {
                        assert!(x == ' ' || x == y, "{w}x{h} control {c}:\n{s}");
                    }
                }
            }
        }
    }

    #[test]
    fn default_text_container() {
        let s = render(20, 9, "Hello wide world of ellipses", 0.25);
        assert_eq!(
            s,
            "     .────────.\n  ,─'          '─.\n ╱                ╲\n; Hello wide world :\n\
             :   of ellipses    ;\n ╲                ╱\n  ╲              ╱\n   '─.        ,─'\n      `──────'"
        );
    }
}
