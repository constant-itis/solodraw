//! Canvas rendering (cells, box-drawing geometry, selection and previews).

use crate::prelude::*;

pub fn color_rgba(c: u64) -> (f64, f64, f64) {
    color_or(c, (0.1, 0.1, 0.12))
}

/// A cell colour, or `default` for "no colour".
pub fn color_or(c: u64, default: (f64, f64, f64)) -> (f64, f64, f64) {
    if c == NO_COLOR || c == 0 {
        return default;
    }
    let r = ((c >> 16) & 0xff) as f64 / 255.0;
    let g = ((c >> 8) & 0xff) as f64 / 255.0;
    let b = (c & 0xff) as f64 / 255.0;
    (r, g, b)
}

/// Canvas colours for the current GTK theme (docs/UI_INVENTORY.md §1.3, §6).
pub struct CanvasTheme {
    pub background: (f64, f64, f64),
    pub text: (f64, f64, f64),
    pub grid: (f64, f64, f64, f64),
}

/// Dark when the theme's foreground colour is light.
pub fn is_dark(w: &impl IsA<gtk::Widget>) -> bool {
    let c = w.style_context().color();
    0.299 * c.red() + 0.587 * c.green() + 0.114 * c.blue() > 0.5
}

pub fn canvas_theme(w: &impl IsA<gtk::Widget>) -> CanvasTheme {
    if is_dark(w) {
        CanvasTheme { background: (0.12, 0.12, 0.13), text: (0.86, 0.86, 0.85), grid: (1.0, 1.0, 1.0, 0.08) }
    } else {
        CanvasTheme { background: (1.0, 1.0, 1.0), text: (0.1, 0.1, 0.12), grid: (0.0, 0.0, 0.0, 0.07) }
    }
}

pub fn draw(area: &gtk::DrawingArea, cr: &gtk::cairo::Context, st: &App) {
    let (cw, ch) = cell_size(area, st);
    let off = st.view_offset;
    let theme = canvas_theme(area);
    cr.set_source_rgb(theme.background.0, theme.background.1, theme.background.2);
    let _ = cr.paint();
    let (x1, y1, x2, y2) = cr.clip_extents().unwrap_or((0.0, 0.0, 4000.0, 4000.0));
    let c0 = (x1 / cw).floor() as i32 - off.x - 1;
    let r0 = (y1 / ch).floor() as i32 - off.y - 1;
    let c1 = (x2 / cw).ceil() as i32 - off.x + 1;
    let r1 = (y2 / ch).ceil() as i32 - off.y + 1;
    let px = |p: Point| ((p.x + off.x) as f64 * cw, (p.y + off.y) as f64 * ch);

    if st.show_vgrid || st.show_hgrid {
        cr.set_source_rgba(theme.grid.0, theme.grid.1, theme.grid.2, theme.grid.3);
        cr.set_line_width(1.0);
        if st.show_vgrid {
            for c in c0..=c1 {
                let x = (c + off.x) as f64 * cw + 0.5;
                cr.move_to(x, y1);
                cr.line_to(x, y2);
            }
        }
        if st.show_hgrid {
            for r in r0..=r1 {
                let y = (r + off.y) as f64 * ch + 0.5;
                cr.move_to(x1, y);
                cr.line_to(x2, y);
            }
        }
        let _ = cr.stroke();
    }
    // Canvas origin marker.
    let (ox, oy) = px(Point::new(0, 0));
    cr.set_source_rgba(0.2, 0.4, 0.9, 0.25);
    cr.rectangle(ox - 3.0, oy - 3.0, 6.0, 6.0);
    let _ = cr.fill();

    let layout = area.create_pango_layout(None);
    layout.set_font_description(Some(&font_desc(st)));
    let mut buf = [0u8; 4];
    let unicode = st.doc.unicode();
    for y in r0..=r1 {
        for x in c0..=c1 {
            let Some(cell) = st.buffer.get(Point::new(x, y)) else { continue };
            let (lx, ly) = px(Point::new(x, y));
            draw_cell(cr, &layout, (lx, ly, cw, ch), cell, unicode, theme.text, &mut buf);
        }
    }

    // Image overlays, stretched over their bounds at their opacity, above the characters
    //. They are never part of an export.
    let scene = st.scene();
    for id in scene.draw_order() {
        let Some(o) = scene.obj(&id).filter(|o| o.type_id() == 25) else { continue };
        let Some(f) = scene.frame(&id) else { continue };
        let (x, y) = px(Point::new(f.x, f.y));
        let (w, h) = (f.w as f64 * cw, f.h as f64 * ch);
        match image_pixbuf(st, &id) {
            Some(pb) if pb.width() > 0 && pb.height() > 0 => {
                cr.save().ok();
                cr.rectangle(x, y, w, h);
                cr.clip();
                cr.translate(x, y);
                cr.scale(w / pb.width() as f64, h / pb.height() as f64);
                cr.set_source_pixbuf(&pb, 0.0, 0.0);
                let _ = cr.paint_with_alpha(o.float("img_opacity").unwrap_or(0.0).clamp(0.0, 1.0));
                cr.restore().ok();
            }
            _ => {
                // Unreadable image data: a crossed box.
                cr.set_source_rgba(0.5, 0.5, 0.5, 0.6);
                cr.set_line_width(1.0);
                cr.rectangle(x + 0.5, y + 0.5, w - 1.0, h - 1.0);
                cr.move_to(x, y);
                cr.line_to(x + w, y + h);
                cr.move_to(x + w, y);
                cr.line_to(x, y + h);
                let _ = cr.stroke();
            }
        }
    }
    drop(scene);

    // Page guide.
    if crate::prefs::get_bool(crate::prefs::PAGE_GUIDE, false) {
        let col = crate::prefs::get_int(crate::prefs::PAGE_GUIDE_COLUMN, 80) as i32;
        let x = (col + off.x) as f64 * cw + 0.5;
        cr.set_source_rgba(0.9, 0.25, 0.2, 0.7);
        cr.set_line_width(1.0);
        cr.move_to(x, y1);
        cr.line_to(x, y2);
        let _ = cr.stroke();
    }

    // Selection: an outline per shape, handles on a single selection.
    {
        let scene = st.scene();
        cr.set_source_rgba(0.15, 0.45, 1.0, 0.9);
        cr.set_line_width(1.5);
        for id in &st.selection {
            let Some(f) = scene.frame(id) else { continue };
            let (sx, sy) = px(Point::new(f.x, f.y));
            cr.rectangle(sx - 1.0, sy - 1.0, f.w as f64 * cw + 2.0, f.h as f64 * ch + 2.0);
        }
        let _ = cr.stroke();
    }
    for (hx, hy) in handle_points(area, st) {
        cr.rectangle(hx - 4.0, hy - 4.0, 8.0, 8.0);
        cr.set_source_rgb(1.0, 1.0, 1.0);
        let _ = cr.fill_preserve();
        cr.set_source_rgba(0.15, 0.45, 1.0, 1.0);
        cr.set_line_width(1.5);
        let _ = cr.stroke();
    }

    // Sidebar hover overlay.
    if let Some(f) = st.hover.as_ref().and_then(|h| st.scene().frame(h)) {
        let (sx, sy) = px(Point::new(f.x, f.y));
        cr.set_source_rgba(0.15, 0.45, 1.0, 0.12);
        cr.rectangle(sx, sy, f.w as f64 * cw, f.h as f64 * ch);
        let _ = cr.fill();
    }

    // Drag previews.
    cr.set_source_rgba(0.15, 0.45, 1.0, 0.6);
    cr.set_line_width(1.0);
    match &st.drag {
        Some(Drag::Create { start, cur }) => {
            let r = Rect::from_points(*start, *cur);
            let (sx, sy) = px(Point::new(r.x, r.y));
            let (w, h) = (r.w as f64 * cw, r.h as f64 * ch);
            if st.tool == Tool::Ellipse {
                cr.save().ok();
                cr.translate(sx + w / 2.0, sy + h / 2.0);
                cr.scale(w.max(1.0) / 2.0, h.max(1.0) / 2.0);
                cr.arc(0.0, 0.0, 1.0, 0.0, std::f64::consts::TAU);
                cr.restore().ok();
            } else {
                cr.rectangle(sx, sy, w, h);
            }
            let _ = cr.stroke();
        }
        Some(Drag::Diamond { centre, cur }) => {
            let r = (cur.x - centre.x).abs().max((cur.y - centre.y).abs()).max(1);
            let (cx, cy) = px(*centre);
            let (cx, cy) = (cx + cw / 2.0, cy + ch / 2.0);
            let (rx, ry) = (r as f64 * cw, r as f64 * ch);
            cr.move_to(cx, cy - ry);
            cr.line_to(cx + rx, cy);
            cr.line_to(cx, cy + ry);
            cr.line_to(cx - rx, cy);
            cr.close_path();
            let _ = cr.stroke();
        }
        Some(Drag::Marquee { start, cur, .. }) => {
            let r = Rect::from_points(*start, *cur);
            let (sx, sy) = px(Point::new(r.x, r.y));
            cr.rectangle(sx, sy, r.w as f64 * cw, r.h as f64 * ch);
            cr.set_source_rgba(0.15, 0.45, 1.0, 0.08);
            let _ = cr.fill_preserve();
            cr.set_source_rgba(0.15, 0.45, 1.0, 0.6);
            let _ = cr.stroke();
        }
        Some(Drag::Line { start, cur }) => {
            let (ax, ay) = px(*start);
            let (bx, by) = px(*cur);
            cr.move_to(ax + cw / 2.0, ay + ch / 2.0);
            cr.line_to(bx + cw / 2.0, ay + ch / 2.0);
            cr.line_to(bx + cw / 2.0, by + ch / 2.0);
            let _ = cr.stroke();
        }
        _ => {}
    }

    // Alignment guides while moving.
    if !st.guides.is_empty() {
        cr.set_source_rgba(0.9, 0.2, 0.7, 0.8);
        cr.set_line_width(1.0);
        cr.set_dash(&[4.0, 3.0], 0.0);
        for &(vertical, t) in &st.guides {
            if vertical {
                let x = ((t + off.x) as f64 * cw + cw / 2.0).floor() + 0.5;
                cr.move_to(x, y1);
                cr.line_to(x, y2);
            } else {
                let y = ((t + off.y) as f64 * ch + ch / 2.0).floor() + 0.5;
                cr.move_to(x1, y);
                cr.line_to(x2, y);
            }
        }
        let _ = cr.stroke();
        cr.set_dash(&[], 0.0);
    }

    // Shape being edited (Shape ▸ Edit).
    if let Some(f) = st.edit_shape.as_ref().and_then(|e| st.scene().frame(e)) {
        let (sx, sy) = px(Point::new(f.x, f.y));
        cr.set_source_rgba(1.0, 0.55, 0.0, 0.9);
        cr.set_line_width(2.0);
        cr.set_dash(&[5.0, 3.0], 0.0);
        cr.rectangle(sx - 2.0, sy - 2.0, f.w as f64 * cw + 4.0, f.h as f64 * ch + 4.0);
        let _ = cr.stroke();
        cr.set_dash(&[], 0.0);
    }

    // Attachment points of the shape a dragged line end would attach to.
    let target = match &st.drag {
        Some(Drag::Line { start, cur }) => attach_choice(st, *cur, *start, ""),
        Some(Drag::LineEnd { line, index, cur }) if is_outer_end(st, line, *index) => attach_choice(st, *cur, line_other_end(st, line, *index, *cur), line),
        _ => None,
    };
    if let Some((_, points, chosen)) = target {
        for (k, q) in points {
            let (qx, qy) = px(q);
            let r = if k == chosen { 4.0 } else { 2.5 };
            cr.arc(qx + cw / 2.0, qy + ch / 2.0, r, 0.0, std::f64::consts::TAU);
            if k == chosen {
                cr.set_source_rgba(0.15, 0.45, 1.0, 1.0);
                let _ = cr.fill();
            } else {
                cr.set_source_rgba(0.15, 0.45, 1.0, 0.55);
                cr.set_line_width(1.0);
                let _ = cr.stroke();
            }
        }
    }

    // Pencil cursor.
    if let Some((_, p, _)) = &st.cursor {
        let (cx, cy) = px(*p);
        cr.set_source_rgba(1.0, 0.55, 0.0, 0.45);
        cr.rectangle(cx, cy, cw, ch);
        let _ = cr.fill();
    }
}

thread_local! {
    /// Decoded image overlays keyed by a hash of their stored data (`None`: unreadable).
    static IMAGES: RefCell<std::collections::HashMap<u64, Option<gdk_pixbuf::Pixbuf>>> = RefCell::default();
}

/// The decoded image of overlay `id`, decoded once per distinct image data.
fn image_pixbuf(st: &App, id: &str) -> Option<gdk_pixbuf::Pixbuf> {
    use std::hash::{Hash, Hasher};
    let data = st.doc.get(id)?.str_("img_data")?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut h);
    let key = h.finish();
    IMAGES.with(|m| {
        m.borrow_mut()
            .entry(key)
            .or_insert_with(|| {
                let bytes = st.doc.image_data(id)?;
                let stream = gio::MemoryInputStream::from_bytes(&glib::Bytes::from_owned(bytes));
                gdk_pixbuf::Pixbuf::from_stream(&stream, gio::Cancellable::NONE).ok()
            })
            .clone()
    })
}

pub fn shade_alpha(c: char) -> Option<f64> {
    match c {
        '█' => Some(1.0),
        '▓' => Some(0.75),
        '▒' => Some(0.5),
        '░' => Some(0.25),
        _ => None,
    }
}

/// Draw half-segments from the cell centre (SPEC §2 segment bits).
pub fn draw_segments(cr: &gtk::cairo::Context, x: f64, y: f64, cw: f64, ch: f64, mask: u8, style: u8) {
    for q in segment_quads(x, y, cw, ch, mask, style) {
        cr.move_to(q[0].0, q[0].1);
        for p in &q[1..] {
            cr.line_to(p.0, p.1);
        }
        cr.close_path();
    }
    let _ = cr.fill();
}

/// The filled outline of a cell's half-segments: one quadrilateral per stroke, all wound the
/// same way so their union fills under the nonzero rule. Straight arms run from the centre to
/// the cell edge with square ends (so they meet at the centre and overlap the neighbour by half
/// a line width); diagonals run from the centre to a corner with flat ends. Shared by the
/// canvas and the SVG export so both draw the same geometry.
pub fn segment_quads(x: f64, y: f64, cw: f64, ch: f64, mask: u8, style: u8) -> Vec<[(f64, f64); 4]> {
    let lw = (cw / 7.0).round().max(1.0);
    let (w, gap) = match style {
        STYLE_HEAVY => (lw * 2.0, 0.0),
        STYLE_DOUBLE => (lw, lw * 1.5),
        _ => (lw, 0.0),
    };
    let snap = |v: f64| if (w as i64) % 2 == 1 { v.floor() + 0.5 } else { v.round() };
    let cx = snap(x + cw / 2.0);
    let cy = snap(y + ch / 2.0);
    let h = w / 2.0;
    // Axis-aligned stroke from (x0, y0) to (x1, y1), square caps, as a rectangle.
    let rect = |x0: f64, y0: f64, x1: f64, y1: f64| {
        let (l, r) = (x0.min(x1) - h, x0.max(x1) + h);
        let (t, b) = (y0.min(y1) - h, y0.max(y1) + h);
        [(l, b), (l, t), (r, t), (r, b)]
    };
    let mut out = Vec::new();
    let offsets: &[f64] = if gap > 0.0 { &[-gap, gap] } else { &[0.0] };
    // A straight run through the cell (both arms) is one rectangle edge to edge.
    let (top, bottom) = (if mask & SEG_U != 0 { y } else { cy }, if mask & SEG_D != 0 { y + ch } else { cy });
    let (left, right) = (if mask & SEG_L != 0 { x } else { cx }, if mask & SEG_R != 0 { x + cw } else { cx });
    for &o in offsets {
        if mask & (SEG_U | SEG_D) != 0 {
            out.push(rect(cx + o, top, cx + o, bottom));
        }
        if mask & (SEG_L | SEG_R) != 0 {
            out.push(rect(left, cy + o, right, cy + o));
        }
    }
    for (bit, ex, ey) in [(SEG_UL, x, y), (SEG_UR, x + cw, y), (SEG_DL, x, y + ch), (SEG_DR, x + cw, y + ch)] {
        if mask & bit != 0 {
            let (dx, dy) = (ex - cx, ey - cy);
            let len = (dx * dx + dy * dy).sqrt();
            let (nx, ny) = (-dy / len * h, dx / len * h);
            let mut q = [(cx + nx, cy + ny), (ex + nx, ey + ny), (ex - nx, ey - ny), (cx - nx, cy - ny)];
            // Same winding as the rectangles (positive shoelace sum in y-down coordinates).
            let area: f64 = (0..4).map(|i| q[i].0 * q[(i + 1) % 4].1 - q[(i + 1) % 4].0 * q[i].1).sum();
            if area < 0.0 {
                q.reverse();
            }
            out.push(q);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Rulers (line numbers on the left, column numbers on top). Numbers are canvas
// coordinates, 0 at the origin.

fn ruler_colors(w: &impl IsA<gtk::Widget>) -> (gdk::RGBA, f64) {
    let c = w.style_context().color();
    (c, 0.55)
}

pub fn draw_ruler_top(w: &gtk::DrawingArea, cr: &gtk::cairo::Context, ui: &Ui, st: &App) {
    let (cw, _) = cell_size(&ui.area, st);
    let (c, a) = ruler_colors(w);
    let scroll_x = ui.scroller.hadjustment().value();
    let left_w = 0.0;
    let width = w.width() as f64;
    let height = w.height() as f64;
    cr.set_source_rgba(c.red() as f64, c.green() as f64, c.blue() as f64, 0.04);
    let _ = cr.paint();
    cr.set_source_rgba(c.red() as f64, c.green() as f64, c.blue() as f64, a);
    cr.set_line_width(1.0);
    let layout = w.create_pango_layout(None);
    let mut fd = pango::FontDescription::from_string("Monospace");
    fd.set_absolute_size(8.5 * pango::SCALE as f64);
    layout.set_font_description(Some(&fd));
    let first = ((scroll_x) / cw).floor() as i32;
    let last = ((scroll_x + width) / cw).ceil() as i32;
    for col in first..=last {
        let x = left_w + col as f64 * cw - scroll_x;
        if x < left_w {
            continue;
        }
        let n = col - st.view_offset.x;
        let tick = if n % 10 == 0 { height * 0.55 } else if n % 5 == 0 { height * 0.3 } else { height * 0.15 };
        cr.move_to(x.floor() + 0.5, height);
        cr.line_to(x.floor() + 0.5, height - tick);
        if n % 10 == 0 {
            layout.set_text(&n.to_string());
            cr.move_to(x + 2.0, 0.0);
            pangocairo::functions::show_layout(cr, &layout);
        }
    }
    let _ = cr.stroke();
    cr.move_to(0.0, height - 0.5);
    cr.line_to(width, height - 0.5);
    let _ = cr.stroke();
}

pub fn draw_ruler_left(w: &gtk::DrawingArea, cr: &gtk::cairo::Context, ui: &Ui, st: &App) {
    let (_, ch) = cell_size(&ui.area, st);
    let (c, a) = ruler_colors(w);
    let scroll_y = ui.scroller.vadjustment().value();
    let width = w.width() as f64;
    let height = w.height() as f64;
    cr.set_source_rgba(c.red() as f64, c.green() as f64, c.blue() as f64, 0.04);
    let _ = cr.paint();
    cr.set_source_rgba(c.red() as f64, c.green() as f64, c.blue() as f64, a);
    let layout = w.create_pango_layout(None);
    let mut fd = font_desc(st);
    fd.set_absolute_size(st.font_size * 0.75 * pango::SCALE as f64);
    layout.set_font_description(Some(&fd));
    let first = (scroll_y / ch).floor() as i32;
    let last = ((scroll_y + height) / ch).ceil() as i32;
    for row in first..=last {
        let y = row as f64 * ch - scroll_y;
        let n = row - st.view_offset.y;
        layout.set_text(&n.to_string());
        let (tw, th) = layout.pixel_size();
        cr.move_to(width - tw as f64 - 6.0, y + (ch - th as f64) / 2.0);
        pangocairo::functions::show_layout(cr, &layout);
    }
    cr.set_line_width(1.0);
    cr.move_to(width - 0.5, 0.0);
    cr.line_to(width - 0.5, height);
    let _ = cr.stroke();
}

/// The box-drawing segments a cell is drawn with (mask, style), or `None` for a font glyph.
pub fn cell_segments(cell: &solodraw_core::tixel::Cell, c: char, unicode: bool) -> Option<(u8, u8)> {
    let segs = if unicode && cell.t.segs != 0 { Some(cell.t.visible()) } else { glyph_segments(c) };
    segs.filter(|(m, _)| *m != 0)
}

/// Draw one rendered cell with its top-left at (x, y), cell size (w, h). Box-drawing and shade
/// characters are drawn as geometry so lines join without gaps. Shared by the canvas and the
/// PNG/SVG export.
pub fn draw_cell(
    cr: &gtk::cairo::Context,
    layout: &pango::Layout,
    (lx, ly, cw, ch): (f64, f64, f64, f64),
    cell: &solodraw_core::tixel::Cell,
    unicode: bool,
    default_rgb: (f64, f64, f64),
    buf: &mut [u8; 4],
) {
    let Some(c) = cell.t.ch else { return };
    if c == ' ' {
        return;
    }
    let (r, g, b) = color_or(cell.color, default_rgb);
    cr.set_source_rgb(r, g, b);
    if let Some((mask, style)) = cell_segments(cell, c, unicode) {
        draw_segments(cr, lx, ly, cw, ch, mask, style);
        return;
    }
    if let Some(alpha) = shade_alpha(c) {
        cr.set_source_rgba(r, g, b, alpha);
        cr.rectangle(lx, ly, cw, ch);
        let _ = cr.fill();
        return;
    }
    layout.set_text(c.encode_utf8(buf));
    if layout.unknown_glyphs_count() > 0 {
        // No font has this character: Monodraw's marker, "?" on red.
        cr.set_source_rgb(0.85, 0.15, 0.15);
        cr.rectangle(lx, ly, cw, ch);
        let _ = cr.fill();
        cr.set_source_rgb(1.0, 1.0, 1.0);
        layout.set_text("?");
    }
    cr.move_to(lx, ly);
    pangocairo::functions::show_layout(cr, layout);
}
