//! Text typesetting inside shapes (SPEC §7).

use crate::geom::{Point, Rect};

#[derive(Clone, Copy, Debug)]
pub struct TextStyle {
    /// 0 horizontal, 1 vertical (each line is a column).
    pub sweep: i64,
    /// 0 down/right, 1 up/left (lines reversed).
    pub movement: i64,
    /// 0 left/top, 1 centre, 2 right/bottom.
    pub alignment: i64,
    /// Block placement across lines: 0 top/left, 1 centre, 2 bottom/right.
    pub position: i64,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle { sweep: 0, movement: 0, alignment: 1, position: 1 }
    }
}

/// One typeset line: (column, char) pairs, spaces omitted.
type Line = Vec<(i32, char)>;

/// Break `text` into lines of at most `len` cells.
pub fn break_lines(text: &str, len: i32) -> Vec<Line> {
    let mut lines = Vec::new();
    if len <= 0 {
        return lines;
    }
    for para in text.split('\n') {
        let para = para.strip_suffix('\r').unwrap_or(para);
        let mut line: Line = Vec::new();
        let mut col = 0i32;
        let chars: Vec<char> = para.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == ' ' {
                // Spaces between words are drawn (they hide what is underneath, as in
                // Monodraw); leading ones only advance, trailing ones are dropped below.
                if !line.is_empty() && col < len {
                    line.push((col, ' '));
                }
                col = (col + 1).min(len);
                i += 1;
            } else if c == '\t' {
                col += (len - col).min(4 - col % 4);
                i += 1;
            } else {
                let start = i;
                while i < chars.len() && chars[i] != ' ' && chars[i] != '\t' {
                    i += 1;
                }
                let word = &chars[start..i];
                let n = word.len() as i32;
                if col > 0 && (col >= len || n > len - col) {
                    trim_spaces(&mut line);
                    lines.push(std::mem::take(&mut line));
                    col = 0;
                }
                for &wc in word {
                    if col >= len {
                        lines.push(std::mem::take(&mut line));
                        col = 0;
                    }
                    line.push((col, wc));
                    col += 1;
                }
            }
        }
        trim_spaces(&mut line);
        lines.push(line);
    }
    lines
}

fn trim_spaces(line: &mut Line) {
    while line.last().is_some_and(|&(_, c)| c == ' ') {
        line.pop();
    }
}

/// Lay out `text` in `rect`; returns the cells to draw.
pub fn layout(text: &str, rect: Rect, style: TextStyle) -> Vec<(Point, char)> {
    if rect.is_empty() || text.is_empty() {
        return vec![];
    }
    let vertical = style.sweep == 1;
    let (len, extent) = if vertical { (rect.h, rect.w) } else { (rect.w, rect.h) };
    let mut lines = break_lines(text, len);
    if style.movement == 1 {
        lines.reverse();
    }
    let count = lines.len() as i32;
    let offset = if count < extent {
        match style.position {
            0 => 0,
            2 => extent - count,
            _ => (extent - count) / 2,
        }
    } else {
        0
    };
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let across = offset + i as i32;
        if across >= extent {
            break;
        }
        let end = line.iter().map(|(c, _)| c + 1).max().unwrap_or(0);
        let shift = match style.alignment {
            1 => (len - end) / 2,
            2 => len - end,
            _ => 0,
        };
        for &(col, ch) in line {
            let along = col + shift;
            if along < 0 || along >= len {
                continue;
            }
            let p = if vertical {
                Point::new(rect.x + across, rect.y + along)
            } else {
                Point::new(rect.x + along, rect.y + across)
            };
            out.push((p, ch));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(text: &str, w: i32, h: i32, style: TextStyle) -> String {
        let cells = layout(text, Rect::new(0, 0, w, h), style);
        let mut g = vec![vec![' '; w as usize]; h as usize];
        for (p, c) in cells {
            g[p.y as usize][p.x as usize] = c;
        }
        g.into_iter().map(|r| r.into_iter().collect::<String>()).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn centred_label() {
        assert_eq!(render("Uncle Wessy", 14, 1, TextStyle::default()), " Uncle Wessy  ");
        assert_eq!(render("Kitchen Witch", 14, 1, TextStyle::default()), "Kitchen Witch ");
    }

    #[test]
    fn wraps_words() {
        let s = TextStyle { alignment: 0, position: 0, ..Default::default() };
        assert_eq!(render("hello big world", 9, 2, s), "hello big\nworld    ");
        assert_eq!(render("abcdefghijk", 5, 3, s), "abcde\nfghij\nk    ");
    }
}
