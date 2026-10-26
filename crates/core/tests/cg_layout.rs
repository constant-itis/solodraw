//! Conformance: rebuild the author's card grid (a real Monodraw 1.7 text export,
//! `ASCII_CG_Layout.txt`, a private fixture: see `common`) as Monodraw shapes and require a byte-identical render.
//!
//! Only the *structure* comes from the fixture (where boxes are, their size, the label
//! strings and the free-hand art). Everything the renderer produces — heavy/light borders,
//! border-character overrides, centred labels, blank padding rows, trailing-whitespace
//! trimming — must reproduce Monodraw's output exactly.

use solodraw_core::export::TextOptions;
use solodraw_core::geom::{Point, Rect};
use solodraw_core::{canvas_to_text, Document, ObjExt};
use serde_json::json;

mod common;

const FIXTURE: &str = "ASCII_CG_Layout.txt";

struct Grid(Vec<Vec<char>>);

impl Grid {
    fn at(&self, x: i32, y: i32) -> char {
        if x < 0 || y < 0 {
            return ' ';
        }
        self.0.get(y as usize).and_then(|r| r.get(x as usize)).copied().unwrap_or(' ')
    }
    fn find_right(&self, x: i32, y: i32, c: char) -> i32 {
        (x + 1..).find(|&i| self.at(i, y) == c || i > x + 400).unwrap()
    }
    fn find_down(&self, x: i32, y: i32, c: char) -> i32 {
        (y + 1..).find(|&j| self.at(x, j) == c || j > y + 400).unwrap()
    }
    fn row_text(&self, x0: i32, x1: i32, y: i32) -> String {
        (x0..=x1).map(|x| self.at(x, y)).collect::<String>().trim().to_string()
    }
}

/// Add a bordered rect at `r`; returns its id.
fn rect(doc: &mut Document, parent: &str, r: Rect, border: i64, text: &str) -> String {
    let id = doc.add_rect(parent, r, text);
    doc.set_value(&id, "rectangular_border", json!(border));
    id
}

fn build(grid: &Grid) -> Document {
    let (mut doc, canvas) = Document::new_empty();
    let rows = grid.0.len() as i32;
    let mut shapes: Vec<(String, i32)> = Vec::new(); // (id, y) — later added ends up on top
    for y in 0..rows {
        let width = grid.0[y as usize].len() as i32;
        for x in 0..width {
            match grid.at(x, y) {
                // Section header: a light box with centred text.
                // (Headers span the page; small boxes inside card art are free-hand art.)
                '┌' if grid.at(x + 1, y) == '─' && grid.find_right(x, y, '┐') - x > 60 => {
                    let x1 = grid.find_right(x, y, '┐');
                    let y1 = grid.find_down(x, y, '└');
                    let text = grid.row_text(x + 1, x1 - 1, y + 1);
                    let id = rect(&mut doc, &canvas, Rect::new(x, y, x1 - x + 1, y1 - y + 1), 4, &text);
                    shapes.push((id, y));
                }
                // Card: heavy outer frame, art box with custom corners, name box.
                '┏' => {
                    let x1 = grid.find_right(x, y, '┓');
                    let y1 = grid.find_down(x, y, '┗');
                    let group = doc.add_group(&canvas);
                    rect(&mut doc, &group, Rect::new(x, y, x1 - x + 1, y1 - y + 1), 5, "");

                    // Art box: from the row below the top edge down to its bottom corner.
                    let (ax, ay) = (x + 2, y + 1);
                    let ax1 = x1 - 2;
                    let ay1 = (ay + 1..y1).find(|&j| grid.at(ax, j) != '│').unwrap();
                    let art = rect(&mut doc, &group, Rect::new(ax, ay, ax1 - ax + 1, ay1 - ay + 1), 4, "");
                    for (key, (cx, cy), default) in [
                        ("border_tixel_tl", (ax, ay), '┌'),
                        ("border_tixel_tr", (ax1, ay), '┐'),
                        ("border_tixel_bl", (ax, ay1), '└'),
                        ("border_tixel_br", (ax1, ay1), '┘'),
                    ] {
                        let c = grid.at(cx, cy);
                        if c != default {
                            doc.set_value(&art, key, json!(c.to_string()));
                        }
                    }
                    // Free-hand art = content tixels relative to the frame (non-blank interior
                    // cells). Edge cells whose character differs from the plain border are
                    // per-cell border overrides (part, offset): content never covers the border.
                    let (mut cells, mut edges) = (Vec::new(), Vec::new());
                    for j in ay..=ay1 {
                        for i in ax..=ax1 {
                            let c = grid.at(i, j);
                            let on_border = j == ay || j == ay1 || i == ax || i == ax1;
                            let corner = (i == ax || i == ax1) && (j == ay || j == ay1);
                            let plain = if j == ay || j == ay1 { '─' } else { '│' };
                            if corner || (on_border && c == plain) || (!on_border && c == ' ') {
                                continue;
                            }
                            if !on_border {
                                cells.push((Point::new(i - ax, j - ay), c));
                            } else if j == ay {
                                edges.push((Point::new(0, i - ax), c));
                            } else if j == ay1 {
                                edges.push((Point::new(1, i - ax), c));
                            } else if i == ax {
                                edges.push((Point::new(2, j - ay), c));
                            } else {
                                edges.push((Point::new(3, j - ay), c));
                            }
                        }
                    }
                    doc.set_tixmap_cells(&art, "content_tixels", &cells);
                    doc.set_tixmap_cells(&art, "border_tixels", &edges);

                    // Name box: next '┌' below the art box in the same column.
                    let ny = grid.find_down(ax, ay1, '┌');
                    let ny1 = grid.find_down(ax, ny, '└');
                    let name = grid.row_text(ax + 1, ax1 - 1, ny + 1);
                    rect(&mut doc, &group, Rect::new(ax, ny, ax1 - ax + 1, ny1 - ny + 1), 4, &name);
                }
                _ => {}
            }
        }
    }
    doc
}

fn render(doc: &Document) -> String {
    let opts = TextOptions { anchor_origin: true, trim_padding: false, trim_trailing: true, unicode: None };
    canvas_to_text(doc, None, &opts).unwrap()
}

fn diff(expected: &str, got: &str) -> String {
    let mut out = String::new();
    for (i, (e, g)) in expected.lines().zip(got.lines()).enumerate() {
        if e != g {
            out.push_str(&format!("line {}:\n  want |{e}|\n  got  |{g}|\n", i + 1));
        }
    }
    let (ne, ng) = (expected.lines().count(), got.lines().count());
    if ne != ng {
        out.push_str(&format!("line count: want {ne}, got {ng}\n"));
    }
    out
}

#[test]
fn cg_layout_matches_monodraw_export() {
    let Some(fixture) = common::private_text(FIXTURE) else { return };
    let grid = Grid(fixture.lines().map(|l| l.chars().collect()).collect());
    let doc = build(&grid);
    let got = render(&doc);
    let d = diff(&fixture, &got);
    assert!(d.is_empty(), "render differs from Monodraw export:\n{d}");
    assert_eq!(got, fixture);
}

#[test]
fn cg_layout_survives_monopic_round_trip() {
    let Some(fixture) = common::private_text(FIXTURE) else { return };
    let grid = Grid(fixture.lines().map(|l| l.chars().collect()).collect());
    let doc = build(&grid);
    for minify in [false, true] {
        let bytes = doc.to_monopic(minify);
        let back = Document::from_bytes(&bytes).unwrap();
        assert_eq!(back.objects.len(), doc.objects.len());
        assert_eq!(render(&back), fixture, "minify={minify}");
    }
    // Canvas tree is intact.
    let back = Document::from_bytes(&doc.to_monopic(true)).unwrap();
    let canvas = back.canvases()[0].clone();
    assert!(!back.get(&canvas).unwrap().refs("subshape_refs").is_empty());
}

/// `SOLODRAW_WRITE_FIXTURE=1 cargo test` writes the reconstructed document next to the fixture.
#[test]
fn write_reconstructed_monopic() {
    if std::env::var_os("SOLODRAW_WRITE_FIXTURE").is_none() {
        return;
    }
    let Some(path) = common::private(FIXTURE) else { return };
    let fixture = std::fs::read_to_string(&path).unwrap();
    let grid = Grid(fixture.lines().map(|l| l.chars().collect()).collect());
    build(&grid).save(path.with_extension("monopic")).unwrap();
}
