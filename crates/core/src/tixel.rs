//! The character cell ("tixel") model and the compositing buffer (SPEC §2, §3).

use std::collections::HashMap;

use crate::geom::{Point, Rect};
use crate::junction_table::JUNCTIONS;

/// Segment indices: half-segments from the cell centre.
pub const SEG_U: u8 = 1 << 0;
pub const SEG_D: u8 = 1 << 1;
pub const SEG_L: u8 = 1 << 2;
pub const SEG_R: u8 = 1 << 3;
pub const SEG_UL: u8 = 1 << 4;
pub const SEG_UR: u8 = 1 << 5;
pub const SEG_DL: u8 = 1 << 6;
pub const SEG_DR: u8 = 1 << 7;

/// Segment styles.
pub const STYLE_HIDDEN: u8 = 1;
pub const STYLE_LIGHT: u8 = 2;
pub const STYLE_HEAVY: u8 = 3;
pub const STYLE_DOUBLE: u8 = 4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tixel {
    /// Segment presence mask (bits 0–7).
    pub segs: u8,
    /// 3-bit style per segment, segment s at bits 3s..3s+2.
    pub styles: u32,
    /// Visible character (derived for line tixels at conversion time).
    pub ch: Option<char>,
}

impl Tixel {
    pub const NULL: Tixel = Tixel { segs: 0, styles: 0, ch: None };

    pub fn ch(c: char) -> Self {
        Tixel { ch: Some(c), ..Tixel::NULL }
    }

    /// All segments in `mask` with one style.
    pub fn segments(mask: u8, style: u8) -> Self {
        let mut t = Tixel::NULL;
        t.add_segments(mask, style);
        t
    }

    pub fn add_segments(&mut self, mask: u8, style: u8) {
        for s in 0..8 {
            if mask & (1 << s) != 0 {
                self.segs |= 1 << s;
                self.styles = (self.styles & !(7 << (3 * s))) | ((style as u32 & 7) << (3 * s));
            }
        }
    }

    pub fn seg_style(&self, s: u8) -> u8 {
        ((self.styles >> (3 * s as u32)) & 7) as u8
    }

    pub fn is_null(&self) -> bool {
        self.segs == 0 && self.ch.is_none()
    }

    pub fn is_line(&self) -> bool {
        self.segs != 0
    }

    /// Visible segments (style 1 dropped) and effective (max) style.
    pub fn visible(&self) -> (u8, u8) {
        let mut mask = 0u8;
        let mut style = 0u8;
        for s in 0..8u8 {
            if self.segs & (1 << s) != 0 {
                let st = self.seg_style(s);
                if st != STYLE_HIDDEN {
                    mask |= 1 << s;
                    style = style.max(st);
                }
            }
        }
        (mask, style)
    }

    /// Glyph for a line tixel (SPEC §3), `None` if nothing visible.
    pub fn line_glyph(&self, unicode: bool) -> Option<char> {
        let (mask, style) = self.visible();
        if mask == 0 {
            return None;
        }
        let row = &JUNCTIONS[mask as usize];
        if !unicode {
            return row[0];
        }
        let orthogonal = mask & 0xf0 == 0;
        if orthogonal {
            match style {
                STYLE_LIGHT => row[1],
                STYLE_HEAVY => row[2],
                STYLE_DOUBLE => row[3],
                _ => None,
            }
        } else {
            row[1]
        }
    }
}

/// Blend modes for writing a tixel (SPEC §2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blend {
    Merge = 0,
    Replace = 1,
    Erase = 2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub t: Tixel,
    pub color: u64,
}

/// Sparse, unbounded tixel buffer in canvas coordinates.
#[derive(Clone, Debug, Default)]
pub struct Buffer {
    pub cells: HashMap<Point, Cell>,
    /// Union of the content extents of everything drawn (SPEC §4.2).
    pub extent: Rect,
}

impl Buffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, p: Point) -> Option<&Cell> {
        self.cells.get(&p)
    }

    pub fn add_extent(&mut self, r: Rect) {
        self.extent = self.extent.union(&r);
    }

    pub fn set(&mut self, p: Point, t: Tixel, color: u64, mode: Blend) {
        self.add_extent(Rect::new(p.x, p.y, 1, 1));
        match mode {
            Blend::Replace => {
                self.cells.insert(p, Cell { t, color });
            }
            Blend::Merge => {
                if t.is_null() {
                    return;
                }
                let cell = self.cells.entry(p).or_default();
                if cell.t.is_line() && t.is_line() {
                    for s in 0..8u8 {
                        if t.segs & (1 << s) == 0 {
                            continue;
                        }
                        let st = t.seg_style(s);
                        if st == STYLE_HIDDEN {
                            // An incoming hidden segment removes that segment.
                            cell.t.segs &= !(1 << s);
                            cell.t.styles &= !(7 << (3 * s));
                        } else {
                            cell.t.add_segments(1 << s, st);
                        }
                    }
                    cell.t.ch = None;
                } else {
                    cell.t = t;
                }
                cell.color = color;
            }
            Blend::Erase => {
                if let Some(cell) = self.cells.get_mut(&p) {
                    if cell.t.is_line() && t.is_line() {
                        cell.t.segs &= !t.segs;
                        if cell.t.segs == 0 {
                            cell.t = Tixel::NULL;
                        }
                    } else if !cell.t.is_line() && !t.is_line() && cell.t.ch == t.ch {
                        cell.t = Tixel::NULL;
                    }
                }
            }
        }
    }

    /// Convert every line tixel to its glyph (SPEC §3), done once after compositing.
    pub fn convert_lines(&mut self, unicode: bool) {
        for cell in self.cells.values_mut() {
            if cell.t.is_line() {
                cell.t.ch = cell.t.line_glyph(unicode);
            }
        }
    }

    /// Bounding box of non-null cells.
    pub fn content_box(&self) -> Rect {
        let mut r = Rect::default();
        for (p, c) in &self.cells {
            if !c.t.is_null() {
                r = r.union(&Rect::new(p.x, p.y, 1, 1));
            }
        }
        r
    }

    /// Text of `rect` (SPEC §4.2): rows joined by `\n`, no trailing newline.
    pub fn string_for_rect(&self, rect: Rect, trim_trailing: bool) -> String {
        let mut out = String::new();
        for y in rect.y..rect.y + rect.h {
            if y > rect.y {
                out.push('\n');
            }
            let mut row = String::new();
            for x in rect.x..rect.x + rect.w {
                match self.cells.get(&Point::new(x, y)).map(|c| c.t) {
                    Some(Tixel { ch: Some(c), .. }) => row.push(c),
                    // Null cells and segment-only cells with nothing visible print as a space.
                    _ => row.push(' '),
                }
            }
            if trim_trailing {
                let t = row.trim_end_matches([' ', '\t']);
                row.truncate(t.len());
            }
            out.push_str(&row);
        }
        out
    }
}

/// Reverse junction lookup: the (segment mask, style) a box-drawing character stands for,
/// so typed/override box characters can be drawn like line tixels.
pub fn glyph_segments(c: char) -> Option<(u8, u8)> {
    use std::collections::HashMap;
    use std::sync::OnceLock;
    static MAP: OnceLock<HashMap<char, (u8, u8)>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut m = HashMap::new();
        for mask in (1..16u8).rev() {
            for (col, style) in [(1, STYLE_LIGHT), (2, STYLE_HEAVY), (3, STYLE_DOUBLE)] {
                if let Some(g) = JUNCTIONS[mask as usize][col] {
                    // Prefer the two-sided form (─ = L+R rather than L).
                    m.entry(g).and_modify(|e: &mut (u8, u8)| {
                        if mask.count_ones() > e.0.count_ones() {
                            *e = (mask, style)
                        }
                    }).or_insert((mask, style));
                }
            }
        }
        m.insert('╱', (SEG_UR | SEG_DL, STYLE_LIGHT));
        m.insert('╲', (SEG_UL | SEG_DR, STYLE_LIGHT));
        m.insert('╳', (SEG_UR | SEG_DL | SEG_UL | SEG_DR, STYLE_LIGHT));
        m
    })
    .get(&c)
    .copied()
}
