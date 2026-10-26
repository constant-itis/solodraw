//! Small form widgets bound to preferences, for the Export and Preferences windows.
//! Each control writes its preference on change and then calls `changed`.

use crate::prelude::*;
use crate::prefs;

pub type Changed = Rc<dyn Fn()>;

/// A two-column grid with a running row counter.
pub struct Form {
    pub grid: gtk::Grid,
    row: std::cell::Cell<i32>,
}

impl Form {
    pub fn new() -> Self {
        let grid = gtk::Grid::new();
        grid.set_row_spacing(6);
        grid.set_column_spacing(12);
        grid.set_margin_start(16);
        grid.set_margin_end(16);
        grid.set_margin_top(12);
        grid.set_margin_bottom(12);
        Form { grid, row: std::cell::Cell::new(0) }
    }

    /// A labelled row ("Label:" right-aligned, widget left-aligned).
    pub fn row(&self, label: &str, w: &impl IsA<gtk::Widget>) {
        let r = self.row.get();
        if !label.is_empty() {
            let l = gtk::Label::new(Some(label));
            l.set_xalign(1.0);
            self.grid.attach(&l, 0, r, 1, 1);
        }
        w.set_halign(gtk::Align::Start);
        self.grid.attach(w, 1, r, 1, 1);
        self.row.set(r + 1);
    }

    /// A widget spanning both columns.
    pub fn wide(&self, w: &impl IsA<gtk::Widget>) {
        let r = self.row.get();
        self.grid.attach(w, 0, r, 2, 1);
        self.row.set(r + 1);
    }
}

pub fn check(label: &str, key: &'static str, default: bool, changed: &Changed) -> gtk::CheckButton {
    let c = gtk::CheckButton::with_label(label);
    c.set_active(prefs::get_bool(key, default));
    let ch = changed.clone();
    c.connect_toggled(move |c| {
        prefs::set(key, json!(c.is_active()));
        ch();
    });
    c
}

/// Popup of `(id, name)` pairs stored as the id.
pub fn choice(items: &[(i64, &str)], key: &'static str, default: i64, changed: &Changed) -> gtk::DropDown {
    let names: Vec<&str> = items.iter().map(|i| i.1).collect();
    let ids: Vec<i64> = items.iter().map(|i| i.0).collect();
    let dd = gtk::DropDown::from_strings(&names);
    let cur = prefs::get_int(key, default);
    dd.set_selected(ids.iter().position(|i| *i == cur).unwrap_or(0) as u32);
    let ch = changed.clone();
    dd.connect_selected_notify(move |d| {
        if let Some(id) = ids.get(d.selected() as usize) {
            prefs::set(key, json!(id));
            ch();
        }
    });
    dd
}

pub fn spin(min: f64, max: f64, key: &'static str, default: i64, changed: &Changed) -> gtk::SpinButton {
    let s = gtk::SpinButton::with_range(min, max, 1.0);
    s.set_value(prefs::get_int(key, default) as f64);
    let ch = changed.clone();
    s.connect_value_changed(move |s| {
        prefs::set(key, json!(s.value() as i64));
        ch();
    });
    s
}

pub fn rgb_of(v: u64) -> gdk::RGBA {
    let (r, g, b) = (((v >> 16) & 0xff) as f32 / 255.0, ((v >> 8) & 0xff) as f32 / 255.0, (v & 0xff) as f32 / 255.0);
    gdk::RGBA::new(r, g, b, 1.0)
}

pub fn argb_of(c: &gdk::RGBA) -> u64 {
    0xff00_0000u64 | ((c.red() * 255.0).round() as u64) << 16 | ((c.green() * 255.0).round() as u64) << 8 | (c.blue() * 255.0).round() as u64
}

/// Colour well storing 0xAARRGGBB.
#[allow(deprecated)]
pub fn color(key: &'static str, default: u64, changed: &Changed) -> gtk::ColorButton {
    let w = gtk::ColorButton::with_rgba(&rgb_of(prefs::get_u64(key, default)));
    w.set_use_alpha(false);
    let ch = changed.clone();
    w.connect_color_set(move |w| {
        prefs::set(key, json!(argb_of(&w.rgba())));
        ch();
    });
    w
}

/// Reset colour wells to their defaults.
#[allow(deprecated)]
pub fn reset_colors(wells: Vec<(gtk::ColorButton, &'static str, u64)>, changed: &Changed) -> gtk::Button {
    let b = gtk::Button::with_label("Reset Colors");
    let ch = changed.clone();
    b.connect_clicked(move |_| {
        for (w, key, default) in &wells {
            prefs::set(key, json!(default));
            w.set_rgba(&rgb_of(*default));
        }
        ch();
    });
    b
}
