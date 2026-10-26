//! Export window: Text | PNG | SVG panes with Monodraw's options, a preview, and
//! Copy / Cancel / Export (docs/UI_INVENTORY.md §5.2, SPEC §8.1–8.2).
//!
//! PNG is drawn with the same cell renderer as the canvas, through cairo. SVG is written
//! directly in Monodraw's structure: one filled path per glyph (font outlines from Pango, box
//! drawing as rectangle geometry shared with the canvas renderer).

use gtk::cairo;
use solodraw_core::export::{export_rect, Decoration, COMMENT_STYLES, FOOTER, WRAP_STYLES};
use solodraw_core::render::render_shapes;
use solodraw_core::tixel::Buffer;

use crate::form::{self, Changed, Form};
use crate::prefs::{self as p};
use crate::prelude::*;

const BLACK: u64 = 0xff00_0000;
const WHITE: u64 = 0xffff_ffff;
const GRID: u64 = 0xffe6_e6e6;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Text,
    Png,
    Svg,
}

impl Format {
    fn ext(self) -> &'static str {
        match self {
            Format::Text => "txt",
            Format::Png => "png",
            Format::Svg => "svg",
        }
    }
}

/// Text export with the Export preferences.
pub fn export_text(st: &App, selected: bool) -> Option<String> {
    let roots = st.selection_roots();
    let shapes = (selected && !roots.is_empty()).then_some(&roots[..]);
    let opts = TextOptions {
        anchor_origin: true,
        trim_padding: p::get_bool(p::EXPORT_TRIM_PADDING, true),
        trim_trailing: p::get_bool(p::EXPORT_TRIM_TRAILING, true),
        unicode: None,
    };
    let d = Decoration {
        comment: p::get_int(p::EXPORT_COMMENT, 0),
        padding: p::get_int(p::EXPORT_PADDING, 1).max(0) as usize,
        footer: p::get_bool(p::EXPORT_FOOTER, false),
        footer_right: false,
        wrap: p::get_bool(p::EXPORT_WRAP, false).then(|| p::get_int(p::EXPORT_WRAP_TAG, 0)),
    };
    solodraw_core::export::export_text(&st.doc, &st.canvas, shapes, &opts, &d)
}

/// Clipboard text for Copy: the selection (or canvas) with the Clipboard preferences.
pub fn clipboard_text(st: &App) -> String {
    let roots = st.selection_roots();
    let shapes = (!roots.is_empty()).then_some(&roots[..]);
    let opts = TextOptions { anchor_origin: true, trim_padding: true, trim_trailing: p::get_bool(p::CLIP_TRIM_TRAILING, true), unicode: None };
    let d = Decoration {
        comment: p::get_int(p::CLIP_COMMENT, 0),
        padding: p::get_int(p::CLIP_PADDING, 1).max(0) as usize,
        ..Default::default()
    };
    solodraw_core::export::export_text(&st.doc, &st.canvas, shapes, &opts, &d).unwrap_or_default()
}

/// Image options for one format.
struct Img {
    trim: bool,
    vgrid: bool,
    hgrid: bool,
    scale: f64,
    text: u64,
    background: Option<u64>,
    grid: u64,
    footer: bool,
}

fn img_opts(f: Format) -> Img {
    match f {
        Format::Svg => Img {
            trim: p::get_bool(p::SVG_TRIM, true),
            vgrid: p::get_bool(p::SVG_VGRID, false),
            hgrid: p::get_bool(p::SVG_HGRID, false),
            scale: 1.0,
            text: p::get_u64(p::SVG_TEXT, BLACK),
            background: None,
            grid: p::get_u64(p::SVG_GRID, GRID),
            footer: p::get_bool(p::SVG_FOOTER, false),
        },
        _ => Img {
            trim: p::get_bool(p::PNG_TRIM, false),
            vgrid: p::get_bool(p::PNG_VGRID, false),
            hgrid: p::get_bool(p::PNG_HGRID, false),
            scale: p::get_int(p::PNG_SCALE, 1).clamp(1, 3) as f64,
            text: p::get_u64(p::PNG_TEXT, BLACK),
            background: p::get_bool(p::PNG_BACKGROUND_ON, true).then(|| p::get_u64(p::PNG_BACKGROUND, WHITE)),
            grid: p::get_u64(p::PNG_GRID, GRID),
            footer: p::get_bool(p::PNG_FOOTER, false),
        },
    }
}

fn region(st: &App, selected: bool, trim: bool) -> Option<(Buffer, Rect)> {
    let roots = st.selection_roots();
    let selected = selected && !roots.is_empty();
    let buf = if selected { render_shapes(&st.scene(), &roots) } else { st.buffer.clone() };
    let rect = export_rect(&buf, &TextOptions { anchor_origin: !selected, trim_padding: trim, trim_trailing: false, unicode: None })?;
    Some((buf, rect))
}

/// The canvas font at its own point size, one point per pixel at 1x (Monodraw 1.6.1 exports with the
/// canvas font: Menlo 11 gives 7x13 cells, Menlo 14 gives 9x17; golden-gui 04-cell-metrics).
fn export_font() -> pango::FontDescription {
    let pref = pango::FontDescription::from_string(&p::get_str(p::FONT_NAME, p::DEFAULT_FONT));
    let size = if pref.size() > 0 { pref.size() as f64 / pango::SCALE as f64 } else { 11.0 };
    let mut d = pango::FontDescription::new();
    d.set_family(&p::font_family());
    d.set_absolute_size(size * pango::SCALE as f64);
    d
}

/// Cell size: the font's unhinted advance and line height, each rounded up to whole pixels.
fn cell_metrics() -> (f64, f64) {
    let ctx = pangocairo::FontMap::default().create_context();
    if let Ok(mut fo) = cairo::FontOptions::new() {
        fo.set_hint_metrics(cairo::HintMetrics::Off);
        fo.set_hint_style(cairo::HintStyle::None);
        pangocairo::functions::context_set_font_options(&ctx, Some(&fo));
    }
    ctx.set_round_glyph_positions(false);
    let m = ctx.metrics(Some(&export_font()), None);
    let scale = pango::SCALE as f64;
    let w = (m.approximate_char_width() as f64 / scale).ceil();
    let h = ((m.ascent() + m.descent()) as f64 / scale).ceil();
    (w.max(1.0), h.max(1.0))
}

/// Size in cells (with the footer rows) for a region.
fn cells(rect: Rect, o: &Img) -> (i32, i32) {
    if o.footer {
        (rect.w.max(FOOTER.chars().count() as i32), rect.h + 2)
    } else {
        (rect.w, rect.h)
    }
}

fn draw_image(cr: &cairo::Context, buf: &Buffer, rect: Rect, o: &Img, unicode: bool, (cw, ch): (f64, f64)) {
    let (wc, hc) = cells(rect, o);
    // Images are one pixel larger than the cells (cols*cw+1 by rows*ch+1) so the last grid line fits.
    let (w, h) = (wc as f64 * cw + 1.0, hc as f64 * ch + 1.0);
    if let Some(bg) = o.background {
        let c = form::rgb_of(bg);
        cr.set_source_rgb(c.red() as f64, c.green() as f64, c.blue() as f64);
        cr.rectangle(0.0, 0.0, w, h);
        let _ = cr.fill();
    }
    if o.vgrid || o.hgrid {
        let c = form::rgb_of(o.grid);
        cr.set_source_rgb(c.red() as f64, c.green() as f64, c.blue() as f64);
        cr.set_line_width(1.0);
        if o.vgrid {
            for x in 0..=wc {
                cr.move_to(x as f64 * cw, 0.0);
                cr.line_to(x as f64 * cw, h);
            }
        }
        if o.hgrid {
            for y in 0..=hc {
                cr.move_to(0.0, y as f64 * ch);
                cr.line_to(w, y as f64 * ch);
            }
        }
        let _ = cr.stroke();
    }
    let layout = pangocairo::functions::create_layout(cr);
    layout.set_font_description(Some(&export_font()));
    let t = form::rgb_of(o.text);
    let text_rgb = (t.red() as f64, t.green() as f64, t.blue() as f64);
    let mut b = [0u8; 4];
    for y in 0..rect.h {
        for x in 0..rect.w {
            if let Some(cell) = buf.get(Point::new(rect.x + x, rect.y + y)) {
                draw_cell(cr, &layout, (x as f64 * cw, y as f64 * ch, cw, ch), cell, unicode, text_rgb, &mut b);
            }
        }
    }
    if o.footer {
        cr.set_source_rgb(text_rgb.0, text_rgb.1, text_rgb.2);
        layout.set_text(FOOTER);
        cr.move_to((wc - FOOTER.chars().count() as i32) as f64 * cw, (hc - 1) as f64 * ch);
        pangocairo::functions::show_layout(cr, &layout);
    }
}

/// PNG bytes for the export, or `None` when there is nothing to export.
pub fn png_bytes(st: &App, selected: bool) -> Option<Vec<u8>> {
    let o = img_opts(Format::Png);
    let scale = o.scale;
    raster(st, selected, &o, scale)
}

fn raster(st: &App, selected: bool, o: &Img, scale: f64) -> Option<Vec<u8>> {
    let (buf, rect) = region(st, selected, o.trim)?;
    let (cw, ch) = cell_metrics();
    let (wc, hc) = cells(rect, o);
    let surf = cairo::ImageSurface::create(cairo::Format::ARgb32, ((wc as f64 * cw + 1.0) * scale).ceil() as i32, ((hc as f64 * ch + 1.0) * scale).ceil() as i32).ok()?;
    {
        let cr = cairo::Context::new(&surf).ok()?;
        cr.scale(scale, scale);
        draw_image(&cr, &buf, rect, o, st.doc.unicode(), (cw, ch));
    }
    let mut out = Vec::new();
    surf.write_to_png(&mut out).ok()?;
    Some(out)
}

/// SVG bytes for the export, or `None` when there is nothing to export.
///
/// Written directly in Monodraw 1.6.1's shape (golden-gui 04-cell-metrics, SPEC §8.2): a root
/// `<svg>` with integer `width`/`height` and no viewBox, then one filled `<path>` per non-blank
/// cell (font outlines for characters, rectangle geometry for box drawing), coordinates with six
/// decimals, `fill="rgb(r, g, b)" fill-opacity="…"`, no `<text>`, no font names, no background.
pub fn svg_bytes(st: &App, selected: bool) -> Option<Vec<u8>> {
    let o = img_opts(Format::Svg);
    let (buf, rect) = region(st, selected, o.trim)?;
    let (cw, ch) = cell_metrics();
    Some(svg_document(&buf, rect, &o, st.doc.unicode(), (cw, ch))?.into_bytes())
}

/// A number the way Monodraw writes it: fixed six decimals, never "-0.000000".
fn num(v: f64) -> String {
    let s = format!("{v:.6}");
    if s == "-0.000000" { "0.000000".into() } else { s }
}

/// `fill="rgb(r, g, b)" fill-opacity="a"` for a colour in 0..1 components.
fn svg_fill((r, g, b): (f64, f64, f64), alpha: f64) -> String {
    let c = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("fill=\"rgb({}, {}, {})\" fill-opacity=\"{}\"", c(r), c(g), c(b), num(alpha))
}

/// SVG path data ("M x y L x y C … Z ", each command followed by a space) for a cairo path,
/// offset by (dx, dy).
fn path_data(path: &cairo::Path, (dx, dy): (f64, f64)) -> String {
    let num = |v: f64, o: f64| num(v + o);
    let mut d = String::new();
    let mut segs = path.iter().peekable();
    while let Some(seg) = segs.next() {
        match seg {
            cairo::PathSegment::MoveTo((x, y)) => {
                // Skip a bare move (Pango leaves the pen after the last glyph).
                if matches!(segs.peek(), Some(cairo::PathSegment::LineTo(_) | cairo::PathSegment::CurveTo(..))) {
                    d += &format!("M {} {} ", num(x, dx), num(y, dy));
                }
            }
            cairo::PathSegment::LineTo((x, y)) => d += &format!("L {} {} ", num(x, dx), num(y, dy)),
            cairo::PathSegment::CurveTo((x1, y1), (x2, y2), (x3, y3)) => {
                d += &format!("C {} {} {} {} {} {} ", num(x1, dx), num(y1, dy), num(x2, dx), num(y2, dy), num(x3, dx), num(y3, dy))
            }
            cairo::PathSegment::ClosePath => d += "Z ",
        }
    }
    d
}

/// Path data for closed polygons.
fn polygons_data(polys: &[[(f64, f64); 4]]) -> String {
    let mut d = String::new();
    for q in polys {
        d += &format!("M {} {} ", num(q[0].0), num(q[0].1));
        for p in &q[1..] {
            d += &format!("L {} {} ", num(p.0), num(p.1));
        }
        d += "Z ";
    }
    d
}

/// Glyph outlines through Pango on a scratch cairo context, unhinted so the outlines are the
/// font's own curves at the export size.
struct Outliner {
    cr: cairo::Context,
    layout: pango::Layout,
}

impl Outliner {
    fn new() -> Option<Self> {
        // Unbounded, so no glyph is ever outside the surface.
        let surf = cairo::RecordingSurface::create(cairo::Content::Alpha, None).ok()?;
        let cr = cairo::Context::new(&surf).ok()?;
        let layout = pangocairo::functions::create_layout(&cr);
        let ctx = layout.context();
        if let Ok(mut fo) = cairo::FontOptions::new() {
            fo.set_hint_metrics(cairo::HintMetrics::Off);
            fo.set_hint_style(cairo::HintStyle::None);
            pangocairo::functions::context_set_font_options(&ctx, Some(&fo));
        }
        ctx.set_round_glyph_positions(false);
        layout.context_changed();
        layout.set_font_description(Some(&export_font()));
        Some(Outliner { cr, layout })
    }

    /// Outline of `text` with its layout's top-left at (x, y); `None` when it has no ink or
    /// no font covers it. The text is laid out at the scratch surface's origin and offset while
    /// serialising: Pango skips glyphs that fall outside the target's clip.
    fn outline(&self, text: &str, x: f64, y: f64) -> Option<String> {
        self.layout.set_text(text);
        if self.layout.unknown_glyphs_count() > 0 {
            return None;
        }
        self.cr.new_path();
        self.cr.move_to(0.0, 0.0);
        pangocairo::functions::layout_path(&self.cr, &self.layout);
        let path = self.cr.copy_path().ok()?;
        let d = path_data(&path, (x, y));
        self.cr.new_path();
        d.contains(['L', 'C']).then_some(d)
    }
}

fn svg_document(buf: &Buffer, rect: Rect, o: &Img, unicode: bool, (cw, ch): (f64, f64)) -> Option<String> {
    let (wc, hc) = cells(rect, o);
    let (w, h) = (wc as f64 * cw + 1.0, hc as f64 * ch + 1.0);
    let mut out = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{w}\" height=\"{h}\">\n");
    let path = |out: &mut String, d: &str, rgb: (f64, f64, f64), alpha: f64| {
        *out += &format!("<path d=\"{d}\" {} />\n", svg_fill(rgb, alpha));
    };
    if o.vgrid || o.hgrid {
        // Grid lines: one-pixel rectangles on the cell boundaries, under the drawing.
        let fill = svg_fill(color_or(o.grid, (0.9, 0.9, 0.9)), 1.0);
        out += "<g id=\"Grid\">\n";
        if o.vgrid {
            for x in 0..=wc {
                out += &format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" {fill} />\n", num(x as f64 * cw), num(0.0), num(1.0), num(h));
            }
        }
        if o.hgrid {
            for y in 0..=hc {
                out += &format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" {fill} />\n", num(0.0), num(y as f64 * ch), num(w), num(1.0));
            }
        }
        out += "</g>\n";
    }
    let outliner = Outliner::new()?;
    let text_rgb = color_or(o.text, (0.0, 0.0, 0.0));
    let mut b = [0u8; 4];
    for y in 0..rect.h {
        for x in 0..rect.w {
            let Some(cell) = buf.get(Point::new(rect.x + x, rect.y + y)) else { continue };
            let Some(c) = cell.t.ch else { continue };
            if c == ' ' {
                continue;
            }
            let (lx, ly) = (x as f64 * cw, y as f64 * ch);
            let rgb = color_or(cell.color, text_rgb);
            if let Some((mask, style)) = cell_segments(cell, c, unicode) {
                path(&mut out, &polygons_data(&segment_quads(lx, ly, cw, ch, mask, style)), rgb, 1.0);
            } else if let Some(alpha) = shade_alpha(c) {
                path(&mut out, &polygons_data(&[[(lx, ly + ch), (lx, ly), (lx + cw, ly), (lx + cw, ly + ch)]]), rgb, alpha);
            } else if let Some(d) = outliner.outline(c.encode_utf8(&mut b), lx, ly) {
                path(&mut out, &d, rgb, 1.0);
            } else if outliner.layout.unknown_glyphs_count() > 0 {
                // No font has this character: Monodraw's marker, "?" on red.
                path(&mut out, &polygons_data(&[[(lx, ly + ch), (lx, ly), (lx + cw, ly), (lx + cw, ly + ch)]]), (0.85, 0.15, 0.15), 1.0);
                if let Some(d) = outliner.outline("?", lx, ly) {
                    path(&mut out, &d, (1.0, 1.0, 1.0), 1.0);
                }
            }
        }
    }
    if o.footer {
        let x0 = wc - FOOTER.chars().count() as i32;
        for (i, c) in FOOTER.chars().enumerate() {
            if c == ' ' {
                continue;
            }
            if let Some(d) = outliner.outline(c.encode_utf8(&mut b), (x0 + i as i32) as f64 * cw, (hc - 1) as f64 * ch) {
                path(&mut out, &d, text_rgb, 1.0);
            }
        }
    }
    out += "</svg>";
    Some(out)
}

fn bytes_for(st: &App, f: Format, selected: bool) -> Option<Vec<u8>> {
    match f {
        Format::Text => export_text(st, selected).map(String::into_bytes),
        Format::Png => png_bytes(st, selected),
        Format::Svg => svg_bytes(st, selected),
    }
}

/// The Export window.
pub fn show(ui: &U, st: &St) {
    let has_sel = !st.borrow().selection.is_empty();
    let win = gtk::Window::builder().title("Export").transient_for(&ui.window).modal(true).default_width(760).default_height(520).build();
    let stack = gtk::Stack::new();
    let switcher = gtk::StackSwitcher::new();
    switcher.set_stack(Some(&stack));
    switcher.set_halign(gtk::Align::Center);
    switcher.set_margin_top(8);

    let selected = Rc::new(std::cell::Cell::new(has_sel));
    let preview_text = gtk::TextView::new();
    preview_text.set_editable(false);
    preview_text.set_monospace(true);
    preview_text.set_cursor_visible(false);
    let preview_pic = gtk::Picture::new();
    preview_pic.set_can_shrink(true);
    let preview = gtk::Stack::new();
    let ts = gtk::ScrolledWindow::new();
    ts.set_child(Some(&preview_text));
    preview.add_named(&ts, Some("text"));
    let ps = gtk::ScrolledWindow::new();
    ps.set_child(Some(&preview_pic));
    preview.add_named(&ps, Some("image"));
    preview.set_hexpand(true);
    preview.set_vexpand(true);
    preview.add_css_class("frame");

    let format = {
        let stack = stack.clone();
        move || match stack.visible_child_name().as_deref() {
            Some("png") => Format::Png,
            Some("svg") => Format::Svg,
            _ => Format::Text,
        }
    };
    let refresh: Changed = {
        let (st, sel, pt, pp, pv, format) = (st.clone(), selected.clone(), preview_text.clone(), preview_pic.clone(), preview.clone(), format.clone());
        Rc::new(move || {
            let s = st.borrow();
            match format() {
                Format::Text => {
                    let t = export_text(&s, sel.get()).unwrap_or_default();
                    let t: String = t.lines().take(400).collect::<Vec<_>>().join("\n");
                    pt.buffer().set_text(&t);
                    pv.set_visible_child_name("text");
                }
                f => {
                    // Preview at 1x through the same renderer; SVG has no background, so
                    // preview it on white.
                    let mut o = img_opts(f);
                    if f == Format::Svg {
                        o.background = Some(WHITE);
                    }
                    let bytes = raster(&s, sel.get(), &o, 1.0);
                    let tex = bytes.and_then(|b| gdk::Texture::from_bytes(&glib::Bytes::from_owned(b)).ok());
                    pp.set_paintable(tex.as_ref());
                    pv.set_visible_child_name("image");
                }
            }
        })
    };

    let sel_check = |r: &Changed| {
        let c = gtk::CheckButton::with_label("Include selected shapes only");
        c.set_active(selected.get());
        c.set_sensitive(has_sel);
        let (sel, r) = (selected.clone(), r.clone());
        c.connect_toggled(move |c| {
            sel.set(c.is_active());
            r();
        });
        c
    };

    // Text.
    let f = Form::new();
    f.wide(&sel_check(&refresh));
    f.wide(&form::check("Trim whitespace padding", p::EXPORT_TRIM_PADDING, true, &refresh));
    f.wide(&form::check("Trim trailing whitespace", p::EXPORT_TRIM_TRAILING, true, &refresh));
    let wrap_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    wrap_row.append(&form::check("Output style:", p::EXPORT_WRAP, false, &refresh));
    wrap_row.append(&form::choice(&WRAP_STYLES, p::EXPORT_WRAP_TAG, 0, &refresh));
    f.wide(&wrap_row);
    f.row("Comment style:", &form::choice(&COMMENT_STYLES, p::EXPORT_COMMENT, 0, &refresh));
    f.row("Comment padding:", &form::spin(0.0, 20.0, p::EXPORT_PADDING, 1, &refresh));
    f.wide(&form::check(&format!("“{FOOTER}” footer"), p::EXPORT_FOOTER, false, &refresh));
    stack.add_titled(&f.grid, Some("text"), "Text");

    // PNG.
    let f = Form::new();
    f.wide(&sel_check(&refresh));
    f.wide(&form::check("Trim whitespace padding", p::PNG_TRIM, false, &refresh));
    f.wide(&form::check("Vertical grid lines", p::PNG_VGRID, false, &refresh));
    f.wide(&form::check("Horizontal grid lines", p::PNG_HGRID, false, &refresh));
    f.row("Resolution:", &form::choice(&[(1, "Normal (1x)"), (2, "Retina (2x)"), (3, "Retina HD (3x)")], p::PNG_SCALE, 1, &refresh));
    let text = form::color(p::PNG_TEXT, BLACK, &refresh);
    f.row("Text color:", &text);
    let bg_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    bg_row.append(&form::check("", p::PNG_BACKGROUND_ON, true, &refresh));
    let bg = form::color(p::PNG_BACKGROUND, WHITE, &refresh);
    bg_row.append(&bg);
    f.row("Background:", &bg_row);
    let grid = form::color(p::PNG_GRID, GRID, &refresh);
    f.row("Grid color:", &grid);
    f.row("", &form::reset_colors(vec![(text, p::PNG_TEXT, BLACK), (bg, p::PNG_BACKGROUND, WHITE), (grid, p::PNG_GRID, GRID)], &refresh));
    f.wide(&form::check(&format!("“{FOOTER}” footer"), p::PNG_FOOTER, false, &refresh));
    stack.add_titled(&f.grid, Some("png"), "PNG");

    // SVG.
    let f = Form::new();
    f.wide(&sel_check(&refresh));
    f.wide(&form::check("Trim whitespace padding", p::SVG_TRIM, true, &refresh));
    f.wide(&form::check("Vertical grid lines", p::SVG_VGRID, false, &refresh));
    f.wide(&form::check("Horizontal grid lines", p::SVG_HGRID, false, &refresh));
    let text = form::color(p::SVG_TEXT, BLACK, &refresh);
    f.row("Text color:", &text);
    let grid = form::color(p::SVG_GRID, GRID, &refresh);
    f.row("Grid color:", &grid);
    f.row("", &form::reset_colors(vec![(text, p::SVG_TEXT, BLACK), (grid, p::SVG_GRID, GRID)], &refresh));
    f.wide(&form::check(&format!("“{FOOTER}” footer"), p::SVG_FOOTER, false, &refresh));
    stack.add_titled(&f.grid, Some("svg"), "SVG");

    stack.set_visible_child_name(match p::get_int(p::EXPORT_PANE, 0) {
        1 => "png",
        2 => "svg",
        _ => "text",
    });
    {
        let r = refresh.clone();
        stack.connect_visible_child_name_notify(move |s| {
            let i = match s.visible_child_name().as_deref() {
                Some("png") => 1,
                Some("svg") => 2,
                _ => 0,
            };
            p::set(p::EXPORT_PANE, json!(i));
            r();
        });
    }

    let body = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    stack.set_vexpand(false);
    stack.set_valign(gtk::Align::Start);
    body.append(&stack);
    preview.set_margin_top(12);
    preview.set_margin_end(12);
    body.append(&preview);

    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    buttons.set_margin_end(12);
    buttons.set_margin_bottom(12);
    buttons.set_margin_top(8);
    let copy = gtk::Button::with_label("Copy");
    let cancel = gtk::Button::with_label("Cancel");
    let export = gtk::Button::with_label("Export…");
    export.add_css_class("suggested-action");
    buttons.append(&copy);
    buttons.append(&cancel);
    buttons.append(&export);

    let vb = gtk::Box::new(gtk::Orientation::Vertical, 0);
    vb.append(&switcher);
    body.set_vexpand(true);
    vb.append(&body);
    vb.append(&buttons);
    win.set_child(Some(&vb));
    win.set_default_widget(Some(&export));
    {
        let w = win.clone();
        cancel.connect_clicked(move |_| w.close());
    }
    {
        let (w, st, ui, sel, format) = (win.clone(), st.clone(), ui.clone(), selected.clone(), format.clone());
        copy.connect_clicked(move |_| {
            let f = format();
            let Some(bytes) = bytes_for(&st.borrow(), f, sel.get()) else {
                ui.status.set_text("Nothing to export");
                return;
            };
            let clip = ui.window.clipboard();
            match f {
                Format::Png => {
                    let provider = gdk::ContentProvider::for_bytes("image/png", &glib::Bytes::from_owned(bytes));
                    let _ = clip.set_content(Some(&provider));
                }
                _ => clip.set_text(&String::from_utf8_lossy(&bytes)),
            }
            ui.status.set_text(&format!("Copied as {}", f.ext().to_uppercase()));
            w.close();
        });
    }
    {
        let (w, st, ui, sel, format) = (win.clone(), st.clone(), ui.clone(), selected.clone(), format.clone());
        export.connect_clicked(move |_| {
            let f = format();
            save_dialog(&ui, &st, f, sel.get());
            w.close();
        });
    }
    {
        let w = win.clone();
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(move |_, k, _, _| {
            if k == gdk::Key::Escape {
                w.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        win.add_controller(keys);
    }
    refresh();
    win.present();
}

fn save_dialog(ui: &U, st: &St, f: Format, selected: bool) {
    let d = gtk::FileChooserNative::new(Some("Export"), Some(&ui.window), gtk::FileChooserAction::Save, Some("_Export"), Some("_Cancel"));
    let stem = st.borrow().path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Untitled".into());
    d.set_current_name(&format!("{stem}.{}", f.ext()));
    let (ui2, st2) = (ui.clone(), st.clone());
    d.connect_response(move |d, resp| {
        if resp == gtk::ResponseType::Accept {
            if let Some(mut path) = d.file().and_then(|f| f.path()) {
                if path.extension().is_none() {
                    path.set_extension(f.ext());
                }
                match bytes_for(&st2.borrow(), f, selected) {
                    Some(b) => {
                        if let Err(e) = std::fs::write(&path, b) {
                            error(&ui2, &format!("Could not export:\n{e}"));
                        } else {
                            ui2.status.set_text(&format!("Exported {}", path.display()));
                        }
                    }
                    None => error(&ui2, "Nothing to export."),
                }
            }
        }
        ui2.dialog.replace(None);
    });
    d.show();
    ui.dialog.replace(Some(d));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_size(png: &[u8]) -> (u32, u32) {
        let be = |i: usize| u32::from_be_bytes(png[i..i + 4].try_into().unwrap());
        (be(16), be(20))
    }

    /// Monodraw 1.6.1 image sizes for the step-04 capture (a 10x5 rect at column 10, row 5):
    /// PNG keeps the canvas origin (19x9 cells), SVG trims (10x5), both cols*cw+1 by rows*ch+1.
    #[test]
    fn image_size_and_trim_defaults() {
        // Default preferences, not the developer's.
        let dir = std::env::temp_dir().join(format!("solodraw-export-test-{}", std::process::id()));
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/gui/04-cell-metrics/rect10x5-Ag.monopic");
        let st = App::new(solodraw_core::Document::open(path).unwrap(), None);
        let (cw, ch) = cell_metrics();
        assert_eq!(png_size(&png_bytes(&st, false).unwrap()), ((19.0 * cw) as u32 + 1, (9.0 * ch) as u32 + 1));
        let svg = String::from_utf8(svg_bytes(&st, false).unwrap()).unwrap();
        let w = format!("width=\"{}", 10.0 * cw + 1.0);
        assert!(svg.contains(&w), "{w} not in {}", &svg[..svg.len().min(300)]);
    }

    /// The SVG has Monodraw's structure (golden-gui 04-cell-metrics/*.svg): width/height and no
    /// viewBox, no text or fonts, one filled path per non-blank cell, box drawing as rectangles.
    #[test]
    fn svg_structure_matches_monodraw() {
        let dir = std::env::temp_dir().join(format!("solodraw-export-test-svg-{}", std::process::id()));
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/gui/04-cell-metrics/rect10x5-Ag.monopic");
        let st = App::new(solodraw_core::Document::open(path).unwrap(), None);
        let (cw, ch) = cell_metrics();
        let svg = String::from_utf8(svg_bytes(&st, false).unwrap()).unwrap();
        if let Ok(out) = std::env::var("SOLODRAW_SVG_OUT") {
            std::fs::write(out, &svg).unwrap();
        }
        let head = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{}\" height=\"{}\">\n", 10.0 * cw + 1.0, 5.0 * ch + 1.0);
        assert!(svg.starts_with(&head), "{}", &svg[..svg.len().min(200)]);
        assert!(svg.ends_with("</svg>"));
        for bad in ["viewBox", "<text", "font-family", "<use", "<symbol", "<defs", "<rect", "<g"] {
            assert!(!svg.contains(bad), "{bad} in SVG");
        }
        let lines: Vec<&str> = svg.lines().collect();
        let paths = &lines[1..lines.len() - 1];
        // 10x5 border = 26 box cells, plus "A" and "g".
        assert_eq!(paths.len(), 28);
        let (mut boxes, mut glyphs) = (0, 0);
        for l in paths {
            assert!(l.starts_with("<path d=\"M "), "{l}");
            assert!(l.ends_with("Z \" fill=\"rgb(0, 0, 0)\" fill-opacity=\"1.000000\" />"), "{l}");
            let d = &l[9..l.find("\" fill").unwrap()];
            for n in d.split(' ').filter(|t| t.len() > 1) {
                assert_eq!(n.split('.').nth(1).map(str::len), Some(6), "{n}");
            }
            // Box drawing: closed axis-aligned rectangles only.
            let is_rect = |q: &str| {
                let n: Vec<f64> = q.split(' ').filter_map(|t| t.parse().ok()).collect();
                let xs: std::collections::BTreeSet<String> = n.iter().step_by(2).map(|v| format!("{v}")).collect();
                let ys: std::collections::BTreeSet<String> = n.iter().skip(1).step_by(2).map(|v| format!("{v}")).collect();
                n.len() == 8 && q.matches('L').count() == 3 && xs.len() == 2 && ys.len() == 2
            };
            if d.split("Z ").filter(|s| !s.is_empty()).all(is_rect) {
                boxes += 1;
            } else {
                glyphs += 1;
            }
        }
        assert_eq!((boxes, glyphs), (26, 2));
    }
}
