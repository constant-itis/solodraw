//! Toolbar / sidebar icons, drawn with cairo in the widget's foreground colour so they follow
//! the light/dark theme. Our own simple line art (not Monodraw's artwork).

use std::f64::consts::PI;

use crate::prelude::*;

use gtk::cairo::Context;

/// A fixed-size drawing area showing icon `name`.
pub fn icon(name: &'static str, size: i32) -> gtk::DrawingArea {
    let a = gtk::DrawingArea::new();
    a.set_content_width(size);
    a.set_content_height(size);
    a.set_can_target(false);
    a.set_draw_func(move |w, cr, width, height| {
        let c = w.style_context().color();
        cr.set_source_rgba(c.red() as f64, c.green() as f64, c.blue() as f64, c.alpha() as f64);
        let s = width.min(height) as f64;
        cr.translate((width as f64 - s) / 2.0, (height as f64 - s) / 2.0);
        cr.scale(s / 24.0, s / 24.0);
        cr.set_line_width(1.6);
        cr.set_line_cap(gtk::cairo::LineCap::Round);
        cr.set_line_join(gtk::cairo::LineJoin::Round);
        paint(name, cr);
    });
    a
}

fn rect(cr: &Context, x: f64, y: f64, w: f64, h: f64) {
    cr.rectangle(x, y, w, h);
}

fn line(cr: &Context, pts: &[(f64, f64)]) {
    if let Some((x, y)) = pts.first() {
        cr.move_to(*x, *y);
    }
    for (x, y) in &pts[1..] {
        cr.line_to(*x, *y);
    }
}

fn circle(cr: &Context, x: f64, y: f64, r: f64) {
    cr.new_sub_path();
    cr.arc(x, y, r, 0.0, 2.0 * PI);
}

fn stroke(cr: &Context) {
    let _ = cr.stroke();
}

fn fill(cr: &Context) {
    let _ = cr.fill();
}

/// Paint icon `name` in a 24×24 box.
pub fn paint(name: &str, cr: &Context) {
    match name {
        "md-select" => {
            line(cr, &[(6.0, 3.5), (6.0, 19.0), (10.0, 15.0), (13.0, 21.0), (15.5, 20.0), (12.5, 14.0), (18.0, 14.0)]);
            cr.close_path();
            stroke(cr);
        }
        "md-pan" => {
            // Four-way arrow.
            line(cr, &[(12.0, 3.0), (12.0, 21.0)]);
            line(cr, &[(3.0, 12.0), (21.0, 12.0)]);
            stroke(cr);
            for (x, y, dx, dy) in [(12.0, 3.0, 0.0, 1.0), (12.0, 21.0, 0.0, -1.0), (3.0, 12.0, 1.0, 0.0), (21.0, 12.0, -1.0, 0.0)] {
                line(cr, &[(x + dx * 3.0 - dy * 3.0, y + dy * 3.0 - dx * 3.0), (x, y), (x + dx * 3.0 + dy * 3.0, y + dy * 3.0 + dx * 3.0)]);
            }
            stroke(cr);
        }
        "md-rect" => {
            rect(cr, 4.0, 6.0, 16.0, 12.0);
            stroke(cr);
        }
        "md-table" => {
            rect(cr, 4.0, 6.0, 16.0, 12.0);
            line(cr, &[(4.0, 12.0), (20.0, 12.0)]);
            line(cr, &[(9.5, 6.0), (9.5, 18.0)]);
            line(cr, &[(14.5, 6.0), (14.5, 18.0)]);
            stroke(cr);
        }
        "md-surface" => {
            rect(cr, 4.0, 6.0, 16.0, 12.0);
            cr.set_dash(&[2.0, 2.0], 0.0);
            stroke(cr);
            cr.set_dash(&[], 0.0);
            for (x, y) in [(8.0, 10.0), (12.0, 10.0), (16.0, 10.0), (8.0, 14.0), (12.0, 14.0), (16.0, 14.0)] {
                circle(cr, x, y, 0.9);
            }
            fill(cr);
        }
        "md-diamond" => {
            line(cr, &[(12.0, 3.5), (20.5, 12.0), (12.0, 20.5), (3.5, 12.0)]);
            cr.close_path();
            stroke(cr);
        }
        "md-ellipse" => {
            cr.save().ok();
            cr.translate(12.0, 12.0);
            cr.scale(9.0, 6.5);
            cr.arc(0.0, 0.0, 1.0, 0.0, 2.0 * PI);
            cr.restore().ok();
            stroke(cr);
        }
        "md-line" => {
            line(cr, &[(4.0, 18.0), (12.0, 18.0), (12.0, 7.0), (17.0, 7.0)]);
            stroke(cr);
            line(cr, &[(17.0, 4.0), (21.0, 7.0), (17.0, 10.0)]);
            cr.close_path();
            fill(cr);
        }
        "md-text" => {
            line(cr, &[(5.0, 6.0), (5.0, 4.5), (19.0, 4.5), (19.0, 6.0)]);
            line(cr, &[(12.0, 4.5), (12.0, 19.5)]);
            line(cr, &[(9.0, 19.5), (15.0, 19.5)]);
            stroke(cr);
        }
        "md-pencil" => {
            line(cr, &[(5.0, 19.0), (6.0, 15.0), (16.0, 5.0), (19.0, 8.0), (9.0, 18.0)]);
            cr.close_path();
            line(cr, &[(14.0, 7.0), (17.0, 10.0)]);
            stroke(cr);
        }
        "md-bucket" => {
            line(cr, &[(5.0, 11.0), (11.0, 5.0), (18.0, 12.0), (12.0, 18.0)]);
            cr.close_path();
            line(cr, &[(5.0, 11.0), (18.0, 12.0)]);
            stroke(cr);
            line(cr, &[(20.0, 15.0), (21.5, 18.5), (20.0, 20.0), (18.5, 18.5)]);
            cr.close_path();
            fill(cr);
        }
        "md-eraser" => {
            line(cr, &[(4.0, 15.0), (13.0, 6.0), (19.0, 12.0), (12.0, 19.0), (8.0, 19.0)]);
            cr.close_path();
            line(cr, &[(8.0, 11.0), (14.0, 17.0)]);
            line(cr, &[(12.0, 19.0), (20.0, 19.0)]);
            stroke(cr);
        }
        "md-picker" => {
            line(cr, &[(5.0, 19.0), (5.0, 16.0), (13.0, 8.0), (16.0, 11.0), (8.0, 19.0)]);
            cr.close_path();
            stroke(cr);
            line(cr, &[(12.0, 7.0), (15.5, 3.5), (20.5, 8.5), (17.0, 12.0)]);
            cr.close_path();
            fill(cr);
        }
        "md-group" => {
            rect(cr, 3.5, 3.5, 17.0, 17.0);
            cr.set_dash(&[2.0, 2.0], 0.0);
            stroke(cr);
            cr.set_dash(&[], 0.0);
            rect(cr, 7.0, 7.0, 5.0, 5.0);
            rect(cr, 12.0, 12.0, 5.0, 5.0);
            stroke(cr);
        }
        "md-ungroup" => {
            rect(cr, 4.0, 4.0, 7.0, 7.0);
            rect(cr, 13.0, 13.0, 7.0, 7.0);
            stroke(cr);
            line(cr, &[(14.0, 5.0), (19.0, 10.0)]);
            line(cr, &[(19.0, 5.0), (14.0, 10.0)]);
            stroke(cr);
        }
        "md-edit" => {
            rect(cr, 3.5, 5.5, 13.0, 13.0);
            stroke(cr);
            line(cr, &[(10.0, 15.0), (11.0, 12.0), (19.0, 4.0), (21.0, 6.0), (13.0, 14.0)]);
            cr.close_path();
            fill(cr);
        }
        "md-image" => {
            rect(cr, 3.5, 5.0, 17.0, 14.0);
            stroke(cr);
            line(cr, &[(5.0, 17.5), (10.0, 11.0), (14.0, 15.0), (16.0, 13.0), (19.0, 17.5)]);
            cr.close_path();
            fill(cr);
            circle(cr, 16.0, 9.0, 1.6);
            fill(cr);
        }
        "md-forward" | "md-backward" => {
            let front = name == "md-forward";
            let (a, b) = if front { ((9.0, 9.0), (4.0, 4.0)) } else { ((4.0, 4.0), (9.0, 9.0)) };
            // Back square outlined, front square filled.
            rect(cr, b.0 + 0.5, b.1 + 0.5, 10.0, 10.0);
            stroke(cr);
            rect(cr, a.0 + 0.5, a.1 + 0.5, 10.0, 10.0);
            fill(cr);
            let _ = (a, b);
            let y = if front { 21.5 } else { 21.5 };
            line(cr, &[(4.0, y), (20.0, y)]);
            stroke(cr);
        }
        "md-export" => {
            line(cr, &[(9.0, 8.0), (5.0, 8.0), (5.0, 20.0), (19.0, 20.0), (19.0, 8.0), (15.0, 8.0)]);
            line(cr, &[(12.0, 14.0), (12.0, 3.0)]);
            line(cr, &[(8.5, 6.5), (12.0, 3.0), (15.5, 6.5)]);
            stroke(cr);
        }
        "md-chars" => {
            rect(cr, 3.5, 3.5, 17.0, 17.0);
            stroke(cr);
            line(cr, &[(3.5, 12.0), (20.5, 12.0)]);
            line(cr, &[(12.0, 3.5), (12.0, 20.5)]);
            stroke(cr);
        }
        "md-sidebar" | "md-assistant" => {
            rect(cr, 3.5, 5.0, 17.0, 14.0);
            stroke(cr);
            let x = if name == "md-sidebar" { 3.5 } else { 14.5 };
            rect(cr, x, 5.0, 6.0, 14.0);
            fill(cr);
        }
        "md-zoom" => {
            circle(cr, 10.5, 10.5, 6.0);
            line(cr, &[(15.0, 15.0), (20.0, 20.0)]);
            stroke(cr);
        }
        "md-lock" | "md-unlock" => {
            rect(cr, 6.0, 11.0, 12.0, 9.0);
            fill(cr);
            if name == "md-lock" {
                cr.arc(12.0, 11.0, 4.0, PI, 2.0 * PI);
            } else {
                cr.arc(8.0, 11.0, 4.0, PI, 2.0 * PI);
            }
            stroke(cr);
        }
        "md-eye" | "md-eye-off" => {
            cr.move_to(3.0, 12.0);
            cr.curve_to(7.0, 5.0, 17.0, 5.0, 21.0, 12.0);
            cr.curve_to(17.0, 19.0, 7.0, 19.0, 3.0, 12.0);
            stroke(cr);
            circle(cr, 12.0, 12.0, 2.5);
            fill(cr);
            if name == "md-eye-off" {
                line(cr, &[(4.0, 20.0), (20.0, 4.0)]);
                stroke(cr);
            }
        }
        // Sidebar row icons by shape kind.
        "kind-rect" => {
            rect(cr, 4.5, 6.5, 15.0, 11.0);
            stroke(cr);
        }
        "kind-text" => {
            cr.set_font_size(15.0);
            cr.move_to(6.5, 17.5);
            let _ = cr.show_text("T");
        }
        "kind-line" => {
            line(cr, &[(4.0, 18.0), (12.0, 18.0), (12.0, 6.0), (20.0, 6.0)]);
            stroke(cr);
        }
        "kind-group" => {
            line(cr, &[(3.5, 7.0), (9.0, 7.0), (11.0, 9.0), (20.5, 9.0), (20.5, 18.5), (3.5, 18.5)]);
            cr.close_path();
            stroke(cr);
        }
        "kind-ellipse" => paint("md-ellipse", cr),
        "kind-diamond" => paint("md-diamond", cr),
        "kind-surface" => paint("md-surface", cr),
        "kind-image" => paint("md-image", cr),
        "kind-table" => paint("md-table", cr),
        _ => {
            circle(cr, 12.0, 12.0, 5.0);
            stroke(cr);
        }
    }
}

/// Icon for a shape object in the sidebar.
pub fn kind_icon(type_id: i64, shape_type: i64) -> &'static str {
    match (type_id, shape_type) {
        (20, 4) => "kind-text",
        (20, 5) => "kind-surface",
        (20, _) => "kind-rect",
        (5, _) => "kind-line",
        (3, _) => "kind-group",
        (28, _) => "kind-ellipse",
        (21, _) | (7, _) => "kind-diamond",
        (25, _) => "kind-image",
        (26, _) => "kind-table",
        _ => "",
    }
}
