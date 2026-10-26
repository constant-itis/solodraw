//! Plain-text export (SPEC §4.2, §8.1).

use crate::format::Document;
use crate::geom::Rect;
use crate::render::{render_scene, render_shapes};
use crate::scene::Scene;
use crate::tixel::Buffer;

#[derive(Clone, Copy, Debug)]
pub struct TextOptions {
    /// Option bit 1: export rect starts at the canvas origin.
    pub anchor_origin: bool,
    /// Option bit 2: drop the canvas-origin anchoring (export just the shapes' extent).
    pub trim_padding: bool,
    /// Option bit 4: trim trailing whitespace per line.
    pub trim_trailing: bool,
    /// Override the document alphabet (true = Unicode).
    pub unicode: Option<bool>,
}

impl Default for TextOptions {
    fn default() -> Self {
        TextOptions { anchor_origin: true, trim_padding: false, trim_trailing: false, unicode: None }
    }
}

/// Export rect for a rendered buffer, or `None` when there is nothing to export.
///
/// The rect is the union of the shapes' content extents. Without trimming it also reaches
/// back to the canvas origin. Trimming padding drops that anchoring and nothing else: an
/// empty shape (say a bare 1x1 surface) still widens the export (golden round-trip,
/// WesASCIIcard / Emily1).
pub fn export_rect(buf: &Buffer, opts: &TextOptions) -> Option<Rect> {
    let rect = buf.extent;
    if rect.is_empty() {
        return None;
    }
    if opts.anchor_origin && !opts.trim_padding {
        // Keep the blank space between the canvas origin and the drawing. Content at
        // negative coordinates (real files have it) is kept rather than clipped:
        // SPEC §4.2's clipping was inferred, and Monodraw shows such content.
        let (x0, y0) = (rect.x.min(0), rect.y.min(0));
        return Some(Rect::new(x0, y0, rect.x + rect.w - x0, rect.y + rect.h - y0));
    }
    Some(rect)
}

/// Render a canvas (default: the first) to text.
pub fn canvas_to_text(doc: &Document, canvas: Option<&str>, opts: &TextOptions) -> Option<String> {
    let canvases = doc.canvases();
    let canvas = canvas.map(str::to_string).or_else(|| canvases.first().cloned())?;
    let scene = Scene::new(doc, &canvas, opts.unicode.unwrap_or_else(|| doc.unicode()));
    let buf = render_scene(&scene);
    let rect = export_rect(&buf, opts)?;
    Some(buf.string_for_rect(rect, opts.trim_trailing))
}

/// Render selected root shapes to text ("selected shapes only": no origin anchoring).
pub fn shapes_to_text(doc: &Document, canvas: &str, shapes: &[String], opts: &TextOptions) -> Option<String> {
    let scene = Scene::new(doc, canvas, opts.unicode.unwrap_or_else(|| doc.unicode()));
    let buf = render_shapes(&scene, shapes);
    let rect = export_rect(&buf, opts)?;
    Some(buf.string_for_rect(rect, opts.trim_trailing))
}

// ---------------------------------------------------------------------------
// Decorations (SPEC §8.1): comment styles, footer, output wrapping.

/// Comment styles, in the export popup's order: (id, name). Ids are Monodraw's.
pub const COMMENT_STYLES: [(i64, &str); 14] = [
    (0, "None"),
    (1, "//"),
    (5, "///"),
    (9, "#"),
    (2, "Header /*"),
    (3, "Header /**"),
    (4, "Box"),
    (13, "Box (Markdeep)"),
    (7, "Vertical /*"),
    (8, "Vertical /**"),
    (6, "XML / HTML"),
    (10, "Markdown (Tabs)"),
    (11, "Markdown (Spaces)"),
    (12, "Markdown (Backticks)"),
];

/// Output wrappers: (tag, name). Tags are Monodraw's.
pub const WRAP_STYLES: [(i64, &str); 5] = [(0, "<pre>"), (2, "<pre><code>"), (3, "Shell (UTF-8)"), (4, "Ruby 2"), (5, "Python 3")];

/// Footer line (Monodraw's says "Created with Monodraw").
pub const FOOTER: &str = "Created with solodraw";

#[derive(Clone, Debug, Default)]
pub struct Decoration {
    /// Comment style id (see [`COMMENT_STYLES`]).
    pub comment: i64,
    pub padding: usize,
    pub footer: bool,
    /// Right-align the footer (PNG/SVG); text export puts it on the left.
    pub footer_right: bool,
    /// Output wrapper tag (see [`WRAP_STYLES`]).
    pub wrap: Option<i64>,
}

/// Add the footer and comment decoration around a rendered block. Rows are padded to a common
/// width first; trailing whitespace is trimmed afterwards when `trim_trailing`.
pub fn decorate(text: &str, d: &Decoration, trim_trailing: bool) -> String {
    let mut rows: Vec<Vec<char>> = text.split('\n').map(|l| l.chars().collect()).collect();
    if d.footer {
        let f: Vec<char> = FOOTER.chars().collect();
        let w = rows.iter().map(Vec::len).max().unwrap_or(0);
        rows.push(vec![]);
        if d.footer_right {
            let w = w.max(f.len());
            let mut r = vec![' '; w - f.len()];
            r.extend(f);
            rows.push(r);
        } else {
            rows.push(f);
        }
    }
    let w = rows.iter().map(Vec::len).max().unwrap_or(0);
    for r in rows.iter_mut() {
        r.resize(w, ' ');
    }
    let p = d.padding;
    let line = |s: &str, w: usize| {
        let mut v: Vec<char> = s.chars().collect();
        v.resize(v.len().max(w), ' ');
        v
    };
    let prefix = |pre: &str, rows: Vec<Vec<char>>| -> Vec<Vec<char>> {
        rows.into_iter().map(|r| pre.chars().chain(std::iter::repeat(' ').take(p)).chain(r).collect()).collect()
    };
    let indent = |rows: Vec<Vec<char>>, n: usize| -> Vec<Vec<char>> { rows.into_iter().map(|r| std::iter::repeat(' ').take(n).chain(r).collect()).collect() };
    let out: Vec<Vec<char>> = match d.comment {
        1 => prefix("//", rows),
        5 => prefix("///", rows),
        9 => prefix("#", rows),
        2 | 3 | 6 => {
            let (top, bottom) = match d.comment {
                2 => ("/*", "*/"),
                3 => ("/**", "*/"),
                _ => ("<!--", "-->"),
            };
            let body = indent(rows, p);
            let w = body.iter().map(Vec::len).max().unwrap_or(0);
            let mut v = vec![line(top, w)];
            v.extend(body);
            v.push(line(bottom, w));
            v
        }
        7 | 8 => {
            let (first, mid, last) = if d.comment == 7 { ("/*", " *", " */") } else { ("/*", "**", "*/") };
            let mut v: Vec<Vec<char>> = rows
                .into_iter()
                .enumerate()
                .map(|(i, r)| (if i == 0 { first } else { mid }).chars().chain(std::iter::repeat(' ').take(p)).chain(r).collect())
                .collect();
            let w = v.iter().map(Vec::len).max().unwrap_or(0);
            v.push(line(last, w));
            v
        }
        4 | 13 => {
            let (tl, tr, bl, br) = if d.comment == 4 { ('/', '\\', '\\', '/') } else { ('*', '*', '*', '*') };
            let inner = w + 2 * p;
            let mut v = Vec::new();
            let edge = |l: char, r: char| -> Vec<char> { std::iter::once(l).chain(std::iter::repeat('*').take(inner)).chain(std::iter::once(r)).collect() };
            v.push(edge(tl, tr));
            let blank: Vec<char> = std::iter::once('*').chain(std::iter::repeat(' ').take(inner)).chain(std::iter::once('*')).collect();
            for _ in 0..p {
                v.push(blank.clone());
            }
            for r in rows {
                v.push(std::iter::once('*').chain(std::iter::repeat(' ').take(p)).chain(r).chain(std::iter::repeat(' ').take(p)).chain(std::iter::once('*')).collect());
            }
            for _ in 0..p {
                v.push(blank.clone());
            }
            v.push(edge(bl, br));
            v
        }
        10 => rows.into_iter().map(|r| std::iter::once('\t').chain(r).collect()).collect(),
        11 => indent(rows, 4),
        12 => {
            let w = w.max(3);
            let mut v = vec![line("```", w)];
            v.extend(rows.into_iter().map(|mut r| {
                r.resize(w, ' ');
                r
            }));
            v.push(line("```", w));
            v
        }
        _ => rows,
    };
    let mut s = String::new();
    for (i, r) in out.iter().enumerate() {
        if i > 0 {
            s.push('\n');
        }
        let row: String = r.iter().collect();
        s.push_str(if trim_trailing { row.trim_end_matches([' ', '\t']) } else { &row });
    }
    s
}

/// Wrap exported text for HTML or a shell/Ruby/Python program that prints it.
pub fn wrap_output(text: &str, tag: i64) -> String {
    let html = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    match tag {
        0 => format!("<pre>\n{}\n</pre>", html(text)),
        2 => format!("<pre><code>\n{}\n</code></pre>", html(text)),
        3 => text
            .split('\n')
            .map(|l| {
                let mut e = String::new();
                for b in l.bytes() {
                    if (0x20..0x7f).contains(&b) && !matches!(b, b'"' | b'$' | b'\\' | b'`') {
                        e.push(b as char);
                    } else {
                        e.push_str(&format!("\\x{b:02x}"));
                    }
                }
                format!("echo \"{e}\";\n")
            })
            .collect(),
        4 => text
            .split('\n')
            .map(|l| {
                let mut e = String::new();
                for c in l.chars() {
                    match c {
                        '"' | '\\' => {
                            e.push('\\');
                            e.push(c);
                        }
                        c if c.is_control() => e.push_str(&format!("\\u{{{:X}}}", c as u32)),
                        c => e.push(c),
                    }
                }
                format!("puts \"{e}\";\n")
            })
            .collect(),
        5 => text
            .split('\n')
            .map(|l| {
                let mut e = String::new();
                for c in l.chars() {
                    match c {
                        '"' | '\\' => {
                            e.push('\\');
                            e.push(c);
                        }
                        c if (c as u32) < 0x20 || (c as u32) >= 0x7f => {
                            if (c as u32) < 0x10000 {
                                e.push_str(&format!("\\u{:04x}", c as u32));
                            } else {
                                e.push_str(&format!("\\U{:08x}", c as u32));
                            }
                        }
                        c => e.push(c),
                    }
                }
                format!("print(\"{e}\");\n")
            })
            .collect(),
        _ => text.to_string(),
    }
}

/// Full text export: render (whole canvas, or `shapes` only), decorate, wrap.
pub fn export_text(doc: &Document, canvas: &str, shapes: Option<&[String]>, opts: &TextOptions, d: &Decoration) -> Option<String> {
    let raw = TextOptions { trim_trailing: false, ..*opts };
    let text = match shapes {
        Some(s) => shapes_to_text(doc, canvas, s, &TextOptions { anchor_origin: false, ..raw })?,
        None => canvas_to_text(doc, Some(canvas), &raw)?,
    };
    let text = decorate(&text, d, opts.trim_trailing);
    Some(match d.wrap {
        Some(tag) => wrap_output(&text, tag),
        None => text,
    })
}
