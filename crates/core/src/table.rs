//! Table shapes (types 26 / 29, SPEC §6.5).
//!
//! A table (type 26, `shape_type` 11) is a rect-positioned frame split into rows and columns by
//! integer ratio arrays (`rows`, `cols`). Cells are sparse: `cells` maps the decimal string of
//! `(row << 32) | col` to a table cell (type 29) holding `cell_index` (the same key) and
//! `text_ref` (a type-27 text). Merged cells are never persisted by Monodraw, so they are not
//! modelled here.

use serde_json::{json, Map, Value};

use crate::format::{Document, Obj, ObjExt};
use crate::geom::{format_point, Point, Rect};
use crate::scene::Scene;

pub const TABLE_TYPE: i64 = 26;
pub const CELL_TYPE: i64 = 29;
/// New tables: 9 × 9, every ratio 1 (Monodraw's creator defaults).
pub const DEFAULT_TRACKS: usize = 9;

pub fn cell_key(row: usize, col: usize) -> String {
    ((row as u64) << 32 | col as u64).to_string()
}

/// (row, col) of a cell key; `None` if it has any non-digit (Monodraw drops those on load).
pub fn split_key(key: &str) -> Option<(usize, usize)> {
    if key.is_empty() || !key.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let k: u64 = key.parse().ok()?;
    Some(((k >> 32) as usize, (k & 0xffff_ffff) as usize))
}

/// A ratio array (`rows` / `cols`). `None` when missing or not all non-negative integers.
pub fn ratios(o: &Obj, key: &str) -> Option<Vec<u64>> {
    o.val(key)?.as_array()?.iter().map(Value::as_u64).collect()
}

/// Split `space` units over tracks in proportion to `ratios`, one unit at a time: each unit
/// goes to the track whose increment leaves the summed absolute error |s_i/t − r_i/R| smallest,
/// ties to the lower index. All-zero ratios are treated as equal **[inferred]**.
pub fn distribute(ratios: &[u64], space: i32) -> Vec<i32> {
    let n = ratios.len();
    let mut sizes = vec![0i64; n];
    if n == 0 || space <= 0 {
        return vec![0; n];
    }
    let equal = vec![1u64; n];
    let r: &[u64] = if ratios.iter().all(|&x| x == 0) { &equal } else { ratios };
    let total: i64 = r.iter().map(|&x| x as i64).sum();
    for t in 1..=space as i64 {
        // Error scaled by R·t keeps everything in integers: Σ |s_i·R − r_i·t|.
        let base: Vec<i64> = (0..n).map(|i| sizes[i] * total - r[i] as i64 * t).collect();
        let sum: i64 = base.iter().map(|b| b.abs()).sum();
        let best = (0..n)
            .min_by_key(|&j| sum - base[j].abs() + (base[j] + total).abs())
            .unwrap_or(0);
        sizes[best] += 1;
    }
    sizes.into_iter().map(|s| s as i32).collect()
}

/// Laid-out table: per-axis cell origins and track sizes (interior, without borders).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableGrid {
    pub frame: Rect,
    pub col_x: Vec<i32>,
    pub col_w: Vec<i32>,
    pub row_y: Vec<i32>,
    pub row_h: Vec<i32>,
}

fn axis(origin: i32, size: i32, ratios: &[u64]) -> Option<(Vec<i32>, Vec<i32>)> {
    let n = ratios.len() as i32;
    let interior = size - (n + 1);
    if n == 0 || interior < 0 {
        return None;
    }
    let sizes = distribute(ratios, interior);
    let mut at = origin;
    let mut starts = Vec::with_capacity(sizes.len());
    for s in &sizes {
        starts.push(at);
        at += s + 1;
    }
    Some((starts, sizes))
}

impl TableGrid {
    /// Lay out `rows` × `cols` ratios in `frame` (the table's content rect, assumed = frame).
    pub fn new(frame: Rect, rows: &[u64], cols: &[u64]) -> Option<TableGrid> {
        let (col_x, col_w) = axis(frame.x, frame.w, cols)?;
        let (row_y, row_h) = axis(frame.y, frame.h, rows)?;
        Some(TableGrid { frame, col_x, col_w, row_y, row_h })
    }

    pub fn rows(&self) -> usize {
        self.row_h.len()
    }

    pub fn cols(&self) -> usize {
        self.col_w.len()
    }

    /// Cell rect including its border; neighbours share border lines.
    pub fn cell_rect(&self, row: usize, col: usize) -> Rect {
        Rect::new(self.col_x[col], self.row_y[row], self.col_w[col] + 2, self.row_h[row] + 2)
    }

    /// Text area of a cell: the cell rect inset by 1.
    pub fn content_rect(&self, row: usize, col: usize) -> Rect {
        self.cell_rect(row, col).inset(1)
    }

    /// The cell whose interior holds `p`, or on a shared border the cell below / right of it.
    pub fn cell_at(&self, p: Point) -> Option<(usize, usize)> {
        if !self.frame.contains(p) {
            return None;
        }
        let pick = |starts: &[i32], v: i32| starts.iter().rposition(|&s| s <= v);
        let col = pick(&self.col_x, p.x.min(self.frame.x1() - 1))?;
        let row = pick(&self.row_y, p.y.min(self.frame.y1() - 1))?;
        Some((row, col))
    }
}

impl Scene<'_> {
    /// Layout of a table shape, `None` if it isn't one or doesn't fit its frame.
    pub fn table_grid(&self, id: &str) -> Option<TableGrid> {
        let o = self.obj(id).filter(|o| o.type_id() == TABLE_TYPE)?;
        TableGrid::new(self.frame(id)?, &ratios(o, "rows")?, &ratios(o, "cols")?)
    }
}

impl Document {
    /// Add a `rows` × `cols` table (all ratios 1) filling `frame` (relative to `parent`), at the
    /// top of `parent`'s z-order. The key set beyond `rows`/`cols`/`cells` follows the other
    /// rect-positioned primitives **[inferred]**; cells start empty (Monodraw seeds "awesome").
    pub fn add_table(&mut self, parent: &str, frame: Rect, rows: usize, cols: usize) -> String {
        let mut o = Obj::new();
        o.insert("type_id".into(), json!(TABLE_TYPE));
        o.insert("doc_ref".into(), json!(self.root().map(|(k, _)| k.to_string()).unwrap_or_default()));
        // Shape base + primitive background/shadow keys + frame keys, borrowed from the rectangle
        // defaults minus its border keys.
        if let Some(d) = crate::format::defaults(20) {
            for (k, v) in d {
                if !k.starts_with("border_") && !k.starts_with("rectangular_border") {
                    o.insert(k.clone(), v.clone());
                }
            }
        }
        o.insert("shape_type".into(), json!(11));
        let name = self.next_name(11);
        o.insert("name".into(), json!(name));
        o.insert("frame_size".into(), json!(format_point(Point::new(frame.w, frame.h))));
        o.insert("rows".into(), json!(vec![1; rows.max(1)]));
        o.insert("cols".into(), json!(vec![1; cols.max(1)]));
        o.insert("cells".into(), json!({}));
        o.insert("position_refs".into(), json!([]));
        let id = self.insert(o);
        let anchor = self.add_static_position(&id, Point::new(frame.x, frame.y));
        self.set_value(&id, "frame_anchor_ref", json!(anchor));
        self.attach_child(parent, &id, 0);
        id
    }

    fn table_cells(&self, table: &str) -> Map<String, Value> {
        self.get(table).and_then(|o| o.get("cells")).and_then(|v| v.as_object()).cloned().unwrap_or_default()
    }

    /// The type-29 cell object at (row, col), if the table has one.
    pub fn table_cell(&self, table: &str, row: usize, col: usize) -> Option<String> {
        let cells = self.table_cells(table);
        cells.get(&cell_key(row, col)).and_then(|v| v.as_str()).map(str::to_string)
    }

    /// Text of a table cell ("" when the cell is absent).
    pub fn table_cell_text(&self, table: &str, row: usize, col: usize) -> String {
        self.table_cell(table, row, col)
            .and_then(|c| self.get(&c)?.str_("text_ref").map(str::to_string))
            .and_then(|t| self.get(&t)?.str_("text").map(str::to_string))
            .unwrap_or_default()
    }

    /// Set a cell's text, creating the cell (and its text) if needed; empty text removes the
    /// cell so the map stays sparse.
    pub fn set_table_cell_text(&mut self, table: &str, row: usize, col: usize, text: &str) {
        let existing = self.table_cell(table, row, col);
        if text.is_empty() {
            if let Some(c) = existing {
                self.remove_table_cell(table, &cell_key(row, col), &c);
            }
            return;
        }
        let tid = existing.as_deref().and_then(|c| self.get(c)?.str_("text_ref").map(str::to_string));
        if let Some(tid) = tid {
            self.set_value(&tid, "text", json!(text));
            return;
        }
        let mut t = Obj::new();
        t.insert("text".into(), json!(text));
        let tid = self.add_object(27, t);
        let mut c = Obj::new();
        c.insert("cell_index".into(), json!(cell_key(row, col)));
        c.insert("text_ref".into(), json!(tid));
        let cid = self.add_object(CELL_TYPE, c);
        if let Some(Value::Object(m)) = self.get_mut(table).map(|o| o.entry("cells").or_insert_with(|| json!({}))) {
            m.insert(cell_key(row, col), json!(cid));
        }
    }

    fn remove_table_cell(&mut self, table: &str, key: &str, cell: &str) {
        if let Some(Value::Object(m)) = self.get_mut(table).and_then(|o| o.get_mut("cells")) {
            m.remove(key);
        }
        if let Some(t) = self.get(cell).and_then(|c| c.str_("text_ref")).map(str::to_string) {
            self.objects.shift_remove(&t);
        }
        self.objects.shift_remove(cell);
    }

    /// Insert a row (`rows` = true) or column before `index` (clamped to the end), copying the
    /// neighbouring track's ratio. The frame grows by that track's laid-out size plus its border
    /// so existing tracks keep their size. Cells at or after `index` move along.
    pub fn insert_table_track(&mut self, canvas: &str, table: &str, rows: bool, index: usize) {
        let key = if rows { "rows" } else { "cols" };
        let Some(mut r) = self.get(table).and_then(|o| ratios(o, key)) else { return };
        let grid = Scene::new(self, canvas, true).table_grid(table);
        let i = index.min(r.len());
        let like = i.min(r.len().saturating_sub(1));
        let grow = grid.map_or(2, |g| if rows { g.row_h[like] } else { g.col_w[like] } + 1);
        r.insert(i, r.get(like).copied().unwrap_or(1));
        self.shift_table_cells(table, rows, i, 1);
        self.set_value(table, key, json!(r));
        self.grow_table(table, rows, grow);
    }

    /// Delete a row or column (never the last one), its cells, and shrink the frame by its
    /// laid-out size plus border.
    pub fn delete_table_track(&mut self, canvas: &str, table: &str, rows: bool, index: usize) {
        let key = if rows { "rows" } else { "cols" };
        let Some(mut r) = self.get(table).and_then(|o| ratios(o, key)) else { return };
        if r.len() <= 1 || index >= r.len() {
            return;
        }
        let grid = Scene::new(self, canvas, true).table_grid(table);
        let shrink = grid.map_or(0, |g| if rows { g.row_h[index] } else { g.col_w[index] } + 1);
        for (k, v) in self.table_cells(table) {
            let Some((row, col)) = split_key(&k) else { continue };
            if (if rows { row } else { col }) == index {
                if let Some(c) = v.as_str() {
                    self.remove_table_cell(table, &k, c);
                }
            }
        }
        self.shift_table_cells(table, rows, index + 1, -1);
        r.remove(index);
        self.set_value(table, key, json!(r));
        self.grow_table(table, rows, -shrink);
    }

    /// Move every cell whose row (or column) is ≥ `from` by `by`, rewriting map keys and
    /// `cell_index`.
    fn shift_table_cells(&mut self, table: &str, rows: bool, from: usize, by: isize) {
        let mut out = Map::new();
        let mut moved = Vec::new();
        for (k, v) in self.table_cells(table) {
            let Some((row, col)) = split_key(&k) else { continue };
            let t = if rows { row } else { col };
            let nk = if t >= from {
                let n = (t as isize + by).max(0) as usize;
                if rows { cell_key(n, col) } else { cell_key(row, n) }
            } else {
                k
            };
            if let Some(c) = v.as_str() {
                moved.push((c.to_string(), nk.clone()));
            }
            out.insert(nk, v);
        }
        for (c, nk) in moved {
            self.set_value(&c, "cell_index", json!(nk));
        }
        self.set_value(table, "cells", Value::Object(out));
    }

    fn grow_table(&mut self, table: &str, rows: bool, by: i32) {
        let Some(size) = self.get(table).and_then(|o| o.point("frame_size")) else { return };
        let s = if rows { Point::new(size.x, size.y + by) } else { Point::new(size.x + by, size.y) };
        self.set_value(table, "frame_size", json!(format_point(Point::new(s.x.max(1), s.y.max(1)))));
    }

    /// (row ratios, column ratios) of a table.
    pub fn table_tracks(&self, table: &str) -> Option<(Vec<u64>, Vec<u64>)> {
        let o = self.get(table).filter(|o| o.type_id() == TABLE_TYPE)?;
        Some((ratios(o, "rows")?, ratios(o, "cols")?))
    }
}
