//! FIGlet letter fonts for text in shapes (SPEC §7.1).
//!
//! Implements the standard FIGfont 2 layout (each font's own default smushing/kerning),
//! equivalent to a plain `figlet -w <width+1>` invocation with justification.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use crate::geom::{Point, Rect};
use crate::text::TextStyle;

const SM_EQUAL: i64 = 1;
const SM_LOWLINE: i64 = 2;
const SM_HIERARCHY: i64 = 4;
const SM_PAIR: i64 = 8;
const SM_BIGX: i64 = 16;
const SM_HARDBLANK: i64 = 32;
const SM_KERN: i64 = 64;
const SM_SMUSH: i64 = 128;

pub struct Font {
    pub hardblank: char,
    pub height: usize,
    pub smushmode: i64,
    pub right_to_left: bool,
    pub chars: HashMap<u32, Vec<Vec<char>>>,
}

impl Font {
    pub fn parse(data: &str) -> Option<Font> {
        let mut lines = data.lines();
        let header = lines.next()?;
        let sig = header.strip_prefix("flf2a")?;
        let mut sig_chars = sig.chars();
        let hardblank = sig_chars.next()?;
        let nums: Vec<i64> = sig_chars.as_str().split_whitespace().filter_map(|s| s.parse().ok()).collect();
        let height = *nums.first()? as usize;
        let old_layout = *nums.get(3)?;
        let comment_lines = *nums.get(4)? as usize;
        let right_to_left = nums.get(5).copied().unwrap_or(0) == 1;
        let smushmode = match nums.get(6) {
            Some(&full) => full,
            None => match old_layout {
                0 => SM_KERN,
                l if l < 0 => 0,
                l => (l & 31) | SM_SMUSH,
            },
        };
        for _ in 0..comment_lines {
            lines.next()?;
        }
        let mut chars = HashMap::new();
        let read_char = |lines: &mut std::str::Lines| -> Option<Vec<Vec<char>>> {
            let mut rows = Vec::with_capacity(height);
            for _ in 0..height {
                let l = lines.next()?.trim_end_matches(['\r', '\n']);
                let mut v: Vec<char> = l.chars().collect();
                // Strip the endmark (last char, repeated at the end of the final row).
                if let Some(&end) = v.last() {
                    while v.last() == Some(&end) {
                        v.pop();
                    }
                }
                rows.push(v);
            }
            Some(rows)
        };
        let required: Vec<u32> = (32..=126).chain([196, 214, 220, 228, 246, 252, 223]).collect();
        for code in required {
            match read_char(&mut lines) {
                Some(rows) => {
                    chars.insert(code, rows);
                }
                None => break,
            }
        }
        // Code-tagged characters.
        while let Some(tag) = lines.next() {
            let tok = tag.split_whitespace().next().unwrap_or("");
            let code = parse_code(tok);
            let Some(rows) = read_char(&mut lines) else { break };
            if let Some(c) = code {
                if c >= 0 {
                    chars.insert(c as u32, rows);
                }
            }
        }
        Some(Font { hardblank, height, smushmode, right_to_left, chars })
    }

    fn smush(&self, l: char, r: char, prev_w: usize, cur_w: usize) -> Option<char> {
        if l == ' ' {
            return Some(r);
        }
        if r == ' ' {
            return Some(l);
        }
        if prev_w < 2 || cur_w < 2 {
            return None;
        }
        let m = self.smushmode;
        if m & SM_SMUSH == 0 {
            return None;
        }
        let hb = self.hardblank;
        if m & 63 == 0 {
            if l == hb {
                return Some(r);
            }
            if r == hb {
                return Some(l);
            }
            return Some(if self.right_to_left { l } else { r });
        }
        if m & SM_HARDBLANK != 0 && l == hb && r == hb {
            return Some(l);
        }
        if l == hb || r == hb {
            return None;
        }
        if m & SM_EQUAL != 0 && l == r {
            return Some(l);
        }
        if m & SM_LOWLINE != 0 {
            if l == '_' && "|/\\[]{}()<>".contains(r) {
                return Some(r);
            }
            if r == '_' && "|/\\[]{}()<>".contains(l) {
                return Some(l);
            }
        }
        if m & SM_HIERARCHY != 0 {
            let classes = ["|", "/\\", "[]", "{}", "()", "<>"];
            for (i, c) in classes.iter().enumerate() {
                let rest: String = classes[i + 1..].concat();
                if c.contains(l) && rest.contains(r) {
                    return Some(r);
                }
                if c.contains(r) && rest.contains(l) {
                    return Some(l);
                }
            }
        }
        if m & SM_PAIR != 0 {
            for (a, b) in [('[', ']'), ('{', '}'), ('(', ')')] {
                if (l == a && r == b) || (l == b && r == a) {
                    return Some('|');
                }
            }
        }
        if m & SM_BIGX != 0 {
            if l == '/' && r == '\\' {
                return Some('|');
            }
            if r == '/' && l == '\\' {
                return Some('Y');
            }
            if l == '>' && r == '<' {
                return Some('X');
            }
        }
        None
    }

    fn smush_amount(&self, line: &[Vec<char>], cur: &[Vec<char>], prev_w: usize) -> usize {
        if self.smushmode & (SM_SMUSH | SM_KERN) == 0 {
            return 0;
        }
        let cur_w = cur.first().map_or(0, |r| r.len());
        let mut max = cur_w as i64;
        for row in 0..self.height {
            let out = &line[row];
            let ch = &cur[row];
            // Last non-blank column of the output row (index len = end of string).
            let mut bd = out.len() as i64;
            while bd > 0 && out.get(bd as usize).is_none_or(|&c| c == ' ') {
                bd -= 1;
            }
            let l = out.get(bd as usize).copied();
            let mut cb = 0usize;
            while ch.get(cb) == Some(&' ') {
                cb += 1;
            }
            let r = ch.get(cb).copied();
            let mut amt = cb as i64 + out.len() as i64 - 1 - bd;
            match (l, r) {
                (None, _) | (Some(' '), _) => amt += 1,
                (Some(l), Some(r)) => {
                    if self.smush(l, r, prev_w, cur_w).is_some() {
                        amt += 1;
                    }
                }
                _ => {}
            }
            max = max.min(amt);
        }
        max.max(0) as usize
    }

    /// Lay out `text` into FIGlet rows with at most `limit` columns per line.
    pub fn render(&self, text: &str, limit: usize) -> Vec<Vec<String>> {
        let mut out_lines: Vec<Vec<String>> = Vec::new();
        for para in text.split('\n') {
            let mut line: Vec<Vec<char>> = vec![Vec::new(); self.height];
            let mut prev_w = 0usize;
            // Snapshot at the last space for word wrapping: (line, prev_w, chars after the space).
            let mut pending: Vec<char> = Vec::new();
            let mut at_space: Option<(Vec<Vec<char>>, usize)> = None;
            let chars: Vec<char> = para.chars().collect();
            let mut i = 0;
            while i < chars.len() {
                let c = chars[i];
                let Some(glyph) = self.chars.get(&(c as u32)) else {
                    i += 1;
                    continue;
                };
                let cur_w = glyph.first().map_or(0, |r| r.len());
                let amt = self.smush_amount(&line, glyph, prev_w);
                let new_len = line[0].len() + cur_w - amt.min(line[0].len() + cur_w);
                if new_len > limit && !line[0].is_empty() {
                    // Wrap: back up to the last space if there is one.
                    if let Some((snap, _)) = at_space.take() {
                        out_lines.push(finish(&snap, self.hardblank));
                        line = vec![Vec::new(); self.height];
                        let redo: Vec<char> = std::mem::take(&mut pending);
                        let rest: Vec<char> = redo.into_iter().chain(chars[i..].iter().copied()).collect();
                        let tail: String = rest.into_iter().collect();
                        let mut sub = self.render(&tail, limit);
                        out_lines.append(&mut sub);
                        line.clear();
                        break;
                    }
                    out_lines.push(finish(&line, self.hardblank));
                    line = vec![Vec::new(); self.height];
                    prev_w = 0;
                    continue;
                }
                self.append(&mut line, glyph, amt, prev_w);
                prev_w = cur_w;
                if c == ' ' {
                    at_space = Some((line.clone(), prev_w));
                    pending.clear();
                } else if at_space.is_some() {
                    pending.push(c);
                }
                i += 1;
            }
            if !line.is_empty() {
                out_lines.push(finish(&line, self.hardblank));
            }
        }
        out_lines
    }

    fn append(&self, line: &mut [Vec<char>], glyph: &[Vec<char>], amt: usize, prev_w: usize) {
        let cur_w = glyph.first().map_or(0, |r| r.len());
        for row in 0..self.height {
            let out = &mut line[row];
            let g = &glyph[row];
            for k in 0..amt.min(g.len()) {
                let col = out.len() as i64 - amt as i64 + k as i64;
                let col = col.max(0) as usize;
                if col < out.len() {
                    out[col] = self.smush(out[col], g[k], prev_w, cur_w).unwrap_or(g[k]);
                }
            }
            out.extend_from_slice(&g[amt.min(g.len())..]);
        }
    }
}

fn finish(line: &[Vec<char>], hb: char) -> Vec<String> {
    line.iter().map(|r| r.iter().map(|&c| if c == hb { ' ' } else { c }).collect()).collect()
}

fn parse_code(tok: &str) -> Option<i64> {
    let (neg, t) = match tok.strip_prefix('-') {
        Some(t) => (true, t),
        None => (false, tok),
    };
    let v = if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).ok()?
    } else if t.len() > 1 && t.starts_with('0') {
        i64::from_str_radix(&t[1..], 8).ok()?
    } else {
        t.parse().ok()?
    };
    Some(if neg { -v } else { v })
}

// ---------------------------------------------------------------------------
// Font lookup

/// Directories searched for `.flf` files.
pub fn font_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(s) = std::env::var("SOLODRAW_FIGLET_DIR") {
        v.extend(std::env::split_paths(&s));
    }
    if let Some(home) = std::env::var_os("HOME") {
        let h = PathBuf::from(home);
        v.push(h.join(".local/share/solodraw/figlet-fonts"));
        v.push(h.join(".local/share/solodraw/figlet-fonts-contrib"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            v.push(dir.join("figlet-fonts"));
            v.push(dir.join("../share/solodraw/figlet-fonts"));
            v.push(dir.join("../share/solodraw/figlet-fonts-contrib"));
        }
    }
    v.push("/usr/share/solodraw/figlet-fonts".into());
    v.push("/usr/share/solodraw/figlet-fonts-contrib".into());
    v.push("/usr/share/figlet".into());
    v.push("/usr/local/share/figlet".into());
    v
}

/// Map a Monodraw font id to the `.flf` base name.
pub fn font_base(id: &str) -> &str {
    id.rsplit_once("figlet-font.contrib.")
        .or_else(|| id.rsplit_once("figlet-font."))
        .map(|(_, b)| b)
        .unwrap_or(id)
}

pub fn load_font(id: &str) -> Option<Arc<Font>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Arc<Font>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(f) = cache.lock().ok()?.get(id) {
        return f.clone();
    }
    let base = font_base(id);
    let font = font_dirs().into_iter().find_map(|d| {
        let bytes = std::fs::read(d.join(format!("{base}.flf"))).ok()?;
        Font::parse(&String::from_utf8_lossy(&bytes)).map(Arc::new)
    });
    cache.lock().ok()?.insert(id.to_string(), font.clone());
    font
}

/// Render `text` with font `id` into `rect` (justification = alignment, vertical
/// placement = position). Returns `None` when the font is unavailable.
pub fn render_in_rect(id: &str, text: &str, rect: Rect, style: TextStyle) -> Option<Vec<(Point, char)>> {
    let font = load_font(id)?;
    if rect.is_empty() {
        return Some(vec![]);
    }
    Some(place(font.render(text, rect.w as usize), rect, style))
}

/// Place rendered FIGlet lines (`blocks`, one per output line) in `rect` (SPEC §7.1).
///
/// Each FIGlet line is aligned by its inked extent: its trailing blank columns don't
/// count, its leading ones do, and centring rounds the pad *up*. Blank rows above and
/// below the whole block (e.g. a glyph's empty descender row, or a trailing `\n`'s
/// empty line) are dropped before vertical placement, which rounds *down*.
fn place(blocks: Vec<Vec<String>>, rect: Rect, style: TextStyle) -> Vec<(Point, char)> {
    let blank = |r: &String| r.chars().all(|c| c == ' ');
    let width = rect.w.max(0) as usize;
    let mut rows: Vec<String> = Vec::new();
    for b in blocks {
        let end = b.iter().map(|r| r.trim_end_matches(' ').chars().count()).max().unwrap_or(0);
        let room = width.saturating_sub(end);
        let pad = match style.alignment {
            1 => room.div_ceil(2),
            2 => room,
            _ => 0,
        };
        rows.extend(b.into_iter().map(|r| format!("{}{}", " ".repeat(pad), r)));
    }
    while rows.last().is_some_and(blank) {
        rows.pop();
    }
    let lead = rows.iter().take_while(|r| blank(r)).count();
    rows.drain(..lead);
    let n = rows.len() as i32;
    let top = if n < rect.h {
        match style.position {
            0 => 0,
            2 => rect.h - n,
            _ => (rect.h - n) / 2,
        }
    } else {
        0
    };
    let mut out = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let y = top + i as i32;
        if y >= rect.h {
            break;
        }
        for (x, c) in row.chars().enumerate() {
            if c != ' ' && (x as i32) < rect.w {
                out.push((Point::new(rect.x + x as i32, rect.y + y), c));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(blocks: &[&[&str]], w: i32, h: i32) -> Vec<String> {
        let blocks = blocks.iter().map(|b| b.iter().map(|s| s.to_string()).collect()).collect();
        let mut g = vec![vec![' '; w as usize]; h as usize];
        for (p, c) in place(blocks, Rect::new(0, 0, w, h), TextStyle::default()) {
            g[p.y as usize][p.x as usize] = c;
        }
        g.into_iter().map(|r| r.into_iter().collect()).collect()
    }

    // smslant "+": 7 columns of ink, blank bottom row (golden: wes_deckNEW, 3cardBackup).
    const PLUS: &[&str] = &["    __ ", " __/ /_", "/_  __/", " /_/   ", "       "];

    #[test]
    fn centre_rounds_pad_up() {
        assert_eq!(grid(&[PLUS], 10, 7)[2], "   __/ /_ ");
        assert_eq!(grid(&[PLUS], 9, 6)[2], "  __/ /_ ");
    }

    #[test]
    fn blank_rows_do_not_count_vertically() {
        // "+\n": the empty second line and the blank glyph row are dropped: 4 rows in 6.
        let g = grid(&[PLUS, &["", "", "", "", ""]], 9, 6);
        assert_eq!(g[0].trim(), "");
        assert_eq!(g[1], "     __  ");
        // smslant "=": blank first row; 3 inked rows in 6 start at row 1.
        let eq: &[&str] = &["      ", "  ____", " /___/", "/___/ ", "      "];
        let g = grid(&[eq], 8, 6);
        assert_eq!(g[1], "   ____ ");
    }

    #[test]
    fn trailing_blank_columns_do_not_count() {
        // Ink ends at 3 of 6: centred on 3 columns in 7 (pad 2).
        assert_eq!(grid(&[&["abc   "]], 7, 1)[0], "  abc  ");
    }
}
