//! Assistant inspector: collapsible sections chosen by the selection type (docs/UI_INVENTORY.md §4).
//! Values come from the primary selected shape; edits apply to every selected shape of a matching type.

use std::collections::HashSet;

use crate::prelude::*;

thread_local! {
    /// Section titles the user collapsed (kept across inspector rebuilds).
    static COLLAPSED: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

/// A collapsible section; returns the box its rows go into.
pub fn section(b: &gtk::Box, title: &str) -> gtk::Box {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 4);
    body.set_margin_top(4);
    let ex = gtk::Expander::new(None);
    let l = gtk::Label::new(None);
    l.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(title)));
    ex.set_label_widget(Some(&l));
    ex.set_margin_top(8);
    ex.set_expanded(!COLLAPSED.with(|c| c.borrow().contains(title)));
    ex.set_child(Some(&body));
    let t = title.to_string();
    ex.connect_expanded_notify(move |e| {
        COLLAPSED.with(|c| {
            let mut c = c.borrow_mut();
            if e.is_expanded() {
                c.remove(&t);
            } else {
                c.insert(t.clone());
            }
        })
    });
    b.append(&ex);
    body
}

pub fn row(b: &gtk::Box, label: &str, w: &impl IsA<gtk::Widget>) {
    let r = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let l = gtk::Label::new(Some(label));
    l.set_halign(gtk::Align::Start);
    l.set_hexpand(true);
    l.set_ellipsize(pango::EllipsizeMode::End);
    r.append(&l);
    r.append(w);
    b.append(&r);
}

fn hint(b: &gtk::Box, text: &str) {
    let l = gtk::Label::new(Some(text));
    l.set_halign(gtk::Align::Start);
    l.set_wrap(true);
    l.set_xalign(0.0);
    l.set_max_width_chars(28);
    l.set_margin_top(6);
    l.add_css_class("dim-label");
    b.append(&l);
}

/// Apply an edit with undo + redraw.
pub fn apply(ui: &U, st: &St, f: impl FnOnce(&mut App)) {
    {
        let mut s = st.borrow_mut();
        s.checkpoint();
        f(&mut s);
        s.rerender();
    }
    refresh(ui, &st.borrow());
}

/// The shapes an edit applies to.
#[derive(Clone)]
struct Ctx {
    ui: U,
    st: St,
    ids: Rc<Vec<String>>,
}

impl Ctx {
    /// Run `f` for every target shape inside one undo step.
    fn each<T: Clone + 'static>(&self, f: impl Fn(&mut App, &str, T) + 'static) -> impl Fn(&mut App, T) + 'static {
        let ids = self.ids.clone();
        move |s, v| {
            for id in ids.iter() {
                f(s, id, v.clone());
            }
        }
    }

    /// Set `key` on every target shape; mixed when the targets store different values.
    fn key<T: Clone + 'static>(&self, key: &'static str, conv: impl Fn(T) -> serde_json::Value + 'static) -> Edit<T> {
        let mixed = self.differs(|s, id| s.doc.get(id).and_then(|o| o.get(key)).cloned());
        Edit::from(self.each(move |s, id, v: T| s.doc.set_value(id, key, conv(v)))).mixed(mixed)
    }

    /// Set `key` on every target shape's text object; mixed like [`Ctx::key`].
    fn text_key<T: Clone + 'static>(&self, key: &'static str, conv: impl Fn(T) -> serde_json::Value + 'static) -> Edit<T> {
        let mixed = self.differs(|s, id| s.doc.text_of(id).and_then(|t| s.doc.get(&t)).and_then(|o| o.get(key)).cloned());
        let f = self.each(move |s, id, v: T| {
            if let Some(t) = s.doc.text_of(id) {
                s.doc.set_value(&t, key, conv(v));
            }
        });
        Edit::from(f).mixed(mixed)
    }

    /// Whether `read` gives different values for the target shapes ("Multiple Values").
    fn differs<V: PartialEq>(&self, read: impl Fn(&App, &str) -> V) -> bool {
        let s = self.st.borrow();
        let mut vals = self.ids.iter().map(|id| read(&s, id));
        let Some(first) = vals.next() else { return false };
        vals.any(|v| v != first)
    }
}

/// What an inspector control does when changed, and whether the selected shapes currently
/// disagree on the value it shows (the control then shows "Multiple Values").
struct Edit<T> {
    f: Rc<dyn Fn(&mut App, T)>,
    mixed: bool,
}

impl<T> Edit<T> {
    fn mixed(mut self, mixed: bool) -> Self {
        self.mixed |= mixed;
        self
    }
}

impl<T, F: Fn(&mut App, T) + 'static> From<F> for Edit<T> {
    fn from(f: F) -> Self {
        Edit { f: Rc::new(f), mixed: false }
    }
}

const MULTIPLE: &str = "Multiple Values";

fn dropdown(c: &Ctx, b: &gtk::Box, label: &str, items: &[&str], selected: usize, on: impl Into<Edit<usize>>) {
    let Edit { f: on, mixed } = on.into();
    // A mixed value selects an extra "Multiple Values" item that does nothing when picked.
    let mut all = items.to_vec();
    if mixed {
        all.push(MULTIPLE);
    }
    let dd = gtk::DropDown::from_strings(&all);
    dd.set_selected(if mixed { items.len() } else { selected } as u32);
    let (ui, st, n) = (c.ui.clone(), c.st.clone(), items.len());
    dd.connect_selected_notify(move |d| {
        let i = d.selected() as usize;
        if i < n {
            apply(&ui, &st, |s| on(s, i));
        }
    });
    row(b, label, &dd);
}

fn check(c: &Ctx, b: &gtk::Box, label: &str, active: bool, on: impl Into<Edit<bool>>) -> gtk::CheckButton {
    let Edit { f: on, mixed } = on.into();
    let cb = gtk::CheckButton::new();
    cb.set_active(active);
    cb.set_inconsistent(mixed);
    let (ui, st) = (c.ui.clone(), c.st.clone());
    cb.connect_toggled(move |cb| {
        cb.set_inconsistent(false);
        let v = cb.is_active();
        apply(&ui, &st, |s| on(s, v));
    });
    row(b, label, &cb);
    cb
}

fn char_box(c: &Ctx, value: &str, on: impl Into<Edit<String>>) -> gtk::Entry {
    let Edit { f: on, mixed } = on.into();
    let e = gtk::Entry::new();
    if mixed {
        e.set_tooltip_text(Some(MULTIPLE));
    } else {
        e.set_text(value);
    }
    e.set_max_length(1);
    e.set_width_chars(2);
    e.set_max_width_chars(2);
    EntryExt::set_alignment(&e, 0.5);
    e.set_placeholder_text(Some(if mixed { "…" } else { "·" }));
    let (ui, st) = (c.ui.clone(), c.st.clone());
    e.connect_changed(move |e| {
        let v = e.text().to_string();
        apply(&ui, &st, |s| on(s, v));
    });
    e
}

fn char_entry(c: &Ctx, b: &gtk::Box, label: &str, value: &str, on: impl Into<Edit<String>>) {
    let e = char_box(c, value, on);
    row(b, label, &e);
}

fn spin(c: &Ctx, b: &gtk::Box, label: &str, value: i32, range: (f64, f64), on: impl Into<Edit<i32>>) {
    let Edit { f: on, mixed } = on.into();
    let sp = gtk::SpinButton::with_range(range.0, range.1, 1.0);
    sp.set_value(value as f64);
    // Mixed: show a dash (too narrow for "Multiple Values", which goes in the tooltip) until the
    // first change; +/- step from the primary's value. Numeric mode would reject the dash, and the
    // input handler stops GTK parsing it as 0 before a step.
    let showing_mixed = Rc::new(std::cell::Cell::new(mixed));
    if mixed {
        sp.set_tooltip_text(Some(MULTIPLE));
        sp.set_numeric(false);
        let flag = showing_mixed.clone();
        sp.connect_map(move |sp| {
            if flag.get() {
                sp.set_text("—");
            }
        });
        // A typed number replaces the dash; anything else keeps the primary's value.
        let flag = showing_mixed.clone();
        sp.connect_input(move |sp| flag.get().then(|| Ok(sp.text().trim().parse::<f64>().unwrap_or_else(|_| sp.value()))));
    }
    let on = Rc::new(on);
    let commit = {
        let (ui, st, on, flag) = (c.ui.clone(), c.st.clone(), on.clone(), showing_mixed.clone());
        Rc::new(move |sp: &gtk::SpinButton| {
            if flag.replace(false) {
                sp.set_tooltip_text(None);
                sp.set_numeric(true);
                sp.set_text(&sp.value_as_int().to_string());
            }
            let v = sp.value_as_int();
            apply(&ui, &st, |s| on(s, v));
        })
    };
    let c2 = commit.clone();
    sp.connect_value_changed(move |sp| c2(sp));
    // Typing the primary's own value into a mixed field changes nothing, so no value-changed:
    // Enter still applies it to every selected shape.
    // (GTK 4.6 has no SpinButton::activate; the inner Text's runs after the spin's own update.)
    let flag = showing_mixed.clone();
    if let Some(text) = sp.delegate().and_then(|d| d.downcast::<gtk::Text>().ok()) {
        let sp2 = sp.downgrade();
        text.connect_activate(move |_| {
            let Some(sp) = sp2.upgrade() else { return };
            if flag.get() && sp.text().trim().parse::<f64>().is_ok() {
                commit(&sp);
            }
        });
    }
    row(b, label, &sp);
}

/// Linked toggle buttons; exactly one is active (none while the value is mixed).
fn segmented(c: &Ctx, b: &gtk::Box, label: &str, items: &[&str], selected: usize, on: impl Into<Edit<usize>>) {
    let Edit { f: on, mixed } = on.into();
    let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    bx.add_css_class("linked");
    if mixed {
        bx.set_tooltip_text(Some(MULTIPLE));
    }
    let mut first: Option<gtk::ToggleButton> = None;
    for (i, item) in items.iter().enumerate() {
        let t = gtk::ToggleButton::with_label(item);
        if let Some(f) = &first {
            t.set_group(Some(f));
        } else {
            first = Some(t.clone());
        }
        t.set_active(!mixed && i == selected);
        let (ui, st, on) = (c.ui.clone(), c.st.clone(), on.clone());
        t.connect_toggled(move |t| {
            if t.is_active() {
                apply(&ui, &st, |s| on(s, i));
            }
        });
        bx.append(&t);
    }
    row(b, label, &bx);
}

/// A colour well plus a reset button. `value` is 0xAARRGGBB or NO_COLOR (default).
fn color(c: &Ctx, b: &gtk::Box, label: &str, value: u64, on: impl Into<Edit<u64>>) {
    let Edit { f: on, mixed } = on.into();
    let bx = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    let rgba = if value == NO_COLOR || value == 0 {
        gdk::RGBA::new(0.1, 0.1, 0.12, 1.0)
    } else {
        let (r, g, b) = color_rgba(value);
        gdk::RGBA::new(r as f32, g as f32, b as f32, 1.0)
    };
    #[allow(deprecated)]
    let well = gtk::ColorButton::with_rgba(&rgba);
    #[allow(deprecated)]
    well.set_use_alpha(false);
    let reset = gtk::Button::from_icon_name("edit-clear-symbolic");
    reset.set_tooltip_text(Some("Default colour"));
    reset.set_sensitive(mixed || (value != NO_COLOR && value != 0));
    let note = gtk::Label::new(Some(MULTIPLE));
    note.add_css_class("dim-label");
    note.set_visible(mixed);
    let (ui, st, on2, note2) = (c.ui.clone(), c.st.clone(), on.clone(), note.clone());
    #[allow(deprecated)]
    well.connect_color_set(move |w| {
        note2.set_visible(false);
        let c = w.rgba();
        let v = 0xff00_0000u64 | ((c.red() * 255.0).round() as u64) << 16 | ((c.green() * 255.0).round() as u64) << 8 | (c.blue() * 255.0).round() as u64;
        apply(&ui, &st, |s| on2(s, v));
    });
    let (ui, st) = (c.ui.clone(), c.st.clone());
    let note2 = note.clone();
    reset.connect_clicked(move |r| {
        r.set_sensitive(false);
        note2.set_visible(false);
        apply(&ui, &st, |s| on(s, NO_COLOR));
    });
    bx.append(&note);
    bx.append(&well);
    bx.append(&reset);
    row(b, label, &bx);
}

fn col(o: &serde_json::Map<String, serde_json::Value>, key: &str) -> u64 {
    o.uint(key).unwrap_or(NO_COLOR)
}

// ---------------------------------------------------------------------------

pub fn rebuild_inspector(ui: &U, st: &St) {
    while let Some(c) = ui.inspector.first_child() {
        ui.inspector.remove(&c);
    }
    let b = &ui.inspector;
    let s = st.borrow();
    let Some(primary) = s.primary() else {
        drop(s);
        document(ui, st, b);
        return;
    };
    let Some(o) = s.doc.get(&primary).cloned() else { return };
    let t = o.type_id();
    let roots = s.selection_roots();
    let locked: Vec<String> = s.selection.iter().filter(|id| s.flag(id, "locked")).cloned().collect();
    // Edits go to the selected shapes with the primary's type.
    let ids: Vec<String> = s.selection.iter().filter(|id| s.doc.get(id).is_some_and(|x| x.type_id() == t)).cloned().collect();
    let n = s.selection.len();
    drop(s);
    let c = Ctx { ui: ui.clone(), st: st.clone(), ids: Rc::new(ids) };

    if !locked.is_empty() {
        let body = section(b, "Locked");
        hint(&body, "The current selection is locked.");
        let unlock = gtk::Button::with_label("Unlock");
        let (ui2, st2) = (ui.clone(), st.clone());
        unlock.connect_clicked(move |_| {
            apply(&ui2, &st2, |s| {
                for id in &locked {
                    s.doc.set_value(id, "locked", json!(false));
                }
            });
            sync(&ui2, &st2);
        });
        body.append(&unlock);
        return;
    }

    if n > 1 {
        alignment(ui, st, b, roots);
        hint(b, &format!("{n} shapes selected. Changes below apply to every selected {}.", type_label(t)));
    } else {
        name_row(&c, b, &primary, &o);
    }

    match t {
        20 | 28 => {
            position(&c, b, &primary, &o);
            geometry(&c, b, &primary);
            attachment(&c, b, &o);
            if t == 20 {
                rect_border(&c, b, &o);
                dash(&c, b, &o, "border_dash", "border_dash_enabled");
            } else {
                ellipse_border(&c, b, &o);
                text_container(&c, b, &o);
            }
            background(&c, b, &o);
            shadow(&c, b, &o);
            text(&c, b, &primary);
            content_color(&c, b, &o);
        }
        21 => {
            diamond_anchor(&c, b, &o);
            diamond_border(&c, b, &o);
            shadow(&c, b, &o);
            text(&c, b, &primary);
            content_color(&c, b, &o);
        }
        5 => {
            line(&c, b, &o);
            dash(&c, b, &o, "line_dash_enabled", "line_dash");
            shadow(&c, b, &o);
            content_color(&c, b, &o);
        }
        25 => {
            let body = section(b, "Image");
            let sc = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.05);
            sc.set_value(o.float("img_opacity").unwrap_or(0.0));
            sc.set_hexpand(true);
            let Edit { f: set, mixed } = c.key("img_opacity", |v: f64| json!(v));
            if mixed {
                sc.set_tooltip_text(Some(MULTIPLE));
            }
            let (ui2, st2) = (ui.clone(), st.clone());
            sc.connect_value_changed(move |sc| {
                let v = sc.value();
                apply(&ui2, &st2, |s| set(s, v));
            });
            row(&body, "Opacity", &sc);
            check(&c, &body, "Clickthrough", o.boolean("cthrough"), c.key("cthrough", |v: bool| json!(v)));
        }
        26 => {
            position(&c, b, &primary, &o);
            geometry(&c, b, &primary);
            table(&c, b, &primary);
            background(&c, b, &o);
            shadow(&c, b, &o);
        }
        3 => {
            let body = section(b, "Group");
            check(&c, &body, "Allows sub-shape selection", o.boolean("subshape_selection"), c.key("subshape_selection", |v: bool| json!(v)));
            shadow(&c, b, &o);
        }
        _ => {}
    }
}

fn type_label(t: i64) -> &'static str {
    match t {
        20 => "rectangle",
        28 => "ellipse",
        21 => "diamond",
        5 => "line",
        3 => "group",
        26 => "table",
        _ => "shape",
    }
}

fn document(ui: &U, st: &St, b: &gtk::Box) {
    let c = Ctx { ui: ui.clone(), st: st.clone(), ids: Rc::new(vec![]) };
    let body = section(b, "Document");
    let alpha = if st.borrow().doc.unicode() { 0 } else { 1 };
    dropdown(&c, &body, "Alphabet", &["Unicode", "ASCII"], alpha, |s: &mut App, i: usize| {
        if let Some(root) = s.doc.root().map(|(k, _)| k.to_string()) {
            s.doc.set_value(&root, "alphabet", json!(if i == 0 { 1 } else { 2 }));
        }
    });
    let table = if Tool::Table.available() { "H table · " } else { "" };
    hint(
        b,
        &format!("No Selection.\n\nS select · R rect · {table}D diamond · C ellipse · L line · T text · F surface · P pencil · B fill · E eraser · I picker · Space pan. Shift+key keeps a tool. Shift-click adds to the selection; drag on empty canvas to marquee. Ctrl+C copies shapes (and their text), Ctrl+V pastes, Ctrl+D duplicates, Ctrl+G groups."),
    );
}

fn name_row(c: &Ctx, b: &gtk::Box, id: &str, o: &serde_json::Map<String, serde_json::Value>) {
    let name_e = gtk::Entry::new();
    name_e.set_text(o.str_("name").unwrap_or(""));
    let (ui, st, id) = (c.ui.clone(), c.st.clone(), id.to_string());
    name_e.connect_activate(move |e| {
        let v = e.text().to_string();
        apply(&ui, &st, |s| {
            s.doc.set_value(&id, "name", json!(v));
            s.doc.set_value(&id, "name_edited", json!(true));
        });
        sync(&ui, &st);
    });
    row(b, "Name", &name_e);
}

// ---------------------------------------------------------------------------
// Alignment (2+ shapes)

#[derive(Clone, Copy)]
enum Align {
    Left,
    CentreX,
    Right,
    Top,
    CentreY,
    Bottom,
    DistH,
    DistV,
}

fn alignment(ui: &U, st: &St, b: &gtk::Box, roots: Vec<String>) {
    let body = section(b, "Alignment");
    let grid = gtk::Grid::new();
    grid.add_css_class("md-align");
    grid.set_column_spacing(2);
    grid.set_row_spacing(2);
    let items = [
        (Align::Left, "⇤", "Align left edges"),
        (Align::CentreX, "↔", "Align horizontal centres"),
        (Align::Right, "⇥", "Align right edges"),
        (Align::Top, "⤒", "Align top edges"),
        (Align::CentreY, "↕", "Align vertical centres"),
        (Align::Bottom, "⤓", "Align bottom edges"),
        (Align::DistH, "⋯", "Distribute horizontally"),
        (Align::DistV, "⋮", "Distribute vertically"),
    ];
    let roots = Rc::new(roots);
    for (i, (how, glyph, tip)) in items.into_iter().enumerate() {
        let btn = gtk::Button::with_label(glyph);
        btn.set_tooltip_text(Some(tip));
        btn.add_css_class("flat");
        let (ui2, st2, r) = (ui.clone(), st.clone(), roots.clone());
        btn.connect_clicked(move |_| {
            apply(&ui2, &st2, |s| align(s, &r, how));
            sync(&ui2, &st2);
        });
        // Row 1: horizontal alignment + distribute; row 2: vertical.
        let (col, row) = match i {
            0..=2 => (i, 0),
            6 => (3, 0),
            3..=5 => (i - 3, 1),
            _ => (3, 1),
        };
        grid.attach(&btn, col as i32, row as i32, 1, 1);
    }
    body.append(&grid);
}

fn align(s: &mut App, ids: &[String], how: Align) {
    let frames: Vec<(String, Rect)> = {
        let scene = s.scene();
        ids.iter().filter_map(|id| scene.frame(id).map(|f| (id.clone(), f))).collect()
    };
    if frames.len() < 2 {
        return;
    }
    let all = frames.iter().map(|(_, f)| *f).reduce(|a, b| a.union(&b)).unwrap_or_default();
    let mut moves: Vec<(String, Point)> = Vec::new();
    match how {
        Align::Left | Align::CentreX | Align::Right | Align::Top | Align::CentreY | Align::Bottom => {
            for (id, f) in &frames {
                let d = match how {
                    Align::Left => Point::new(all.x - f.x, 0),
                    Align::Right => Point::new(all.x1() - f.x1(), 0),
                    Align::CentreX => Point::new(all.x + all.w / 2 - (f.x + f.w / 2), 0),
                    Align::Top => Point::new(0, all.y - f.y),
                    Align::Bottom => Point::new(0, all.y1() - f.y1()),
                    _ => Point::new(0, all.y + all.h / 2 - (f.y + f.h / 2)),
                };
                moves.push((id.clone(), d));
            }
        }
        Align::DistH | Align::DistV => {
            // Equal gaps between neighbours; the outermost shapes stay put.
            let horiz = matches!(how, Align::DistH);
            let mut sorted = frames.clone();
            sorted.sort_by_key(|(_, f)| if horiz { f.x } else { f.y });
            let size: i32 = sorted.iter().map(|(_, f)| if horiz { f.w } else { f.h }).sum();
            let span = if horiz { all.w } else { all.h };
            let gaps = (sorted.len() - 1) as i32;
            let free = span - size;
            let mut pos = if horiz { all.x } else { all.y };
            for (i, (id, f)) in sorted.iter().enumerate() {
                let cur = if horiz { f.x } else { f.y };
                moves.push((id.clone(), if horiz { Point::new(pos - cur, 0) } else { Point::new(0, pos - cur) }));
                let gap = free / gaps + if (i as i32) < free % gaps { 1 } else { 0 };
                pos += (if horiz { f.w } else { f.h }) + gap;
            }
        }
    }
    for (id, d) in moves {
        if d != Point::default() {
            s.doc.move_shape(&id, d);
        }
    }
}

// ---------------------------------------------------------------------------
// Rect / ellipse sections

/// Keypad corner shown by the 3×3 anchor picker.
fn position(c: &Ctx, b: &gtk::Box, id: &str, o: &serde_json::Map<String, serde_json::Value>) {
    let body = section(b, "Position");
    let corner = o.int_or("frame_corner", 1).clamp(1, 9);
    let f = c.st.borrow().scene().frame(id).unwrap_or_default();
    let p = f.keypad(corner).map(|(p, _)| p).unwrap_or_default();

    // 3×3 anchor picker.
    let grid = gtk::Grid::new();
    grid.add_css_class("md-compact");
    grid.set_row_spacing(1);
    grid.set_column_spacing(1);
    let mut first: Option<gtk::ToggleButton> = None;
    for k in 1..=9i64 {
        let t = gtk::ToggleButton::new();
        t.set_label(if k == corner { "●" } else { "·" });
        t.add_css_class("flat");
        t.set_size_request(22, 18);
        if let Some(f) = &first {
            t.set_group(Some(f));
        } else {
            first = Some(t.clone());
        }
        t.set_active(k == corner);
        let set = c.each(|s, id, k: i64| set_frame_corner(s, id, k));
        let (ui, st) = (c.ui.clone(), c.st.clone());
        t.connect_toggled(move |t| {
            if t.is_active() {
                apply(&ui, &st, |s| set(s, k));
                sync(&ui, &st);
            }
        });
        grid.attach(&t, ((k - 1) % 3) as i32, ((k - 1) / 3) as i32, 1, 1);
    }
    row(&body, "Anchor", &grid);

    // X/Y place the anchor corner; other selected shapes move by the same amount.
    let target = |s: &App, id: &str| -> Option<Point> {
        let k = s.doc.get(id)?.int_or("frame_corner", 1).clamp(1, 9);
        s.scene().frame(id)?.keypad(k).map(|(p, _)| p)
    };
    for (label, value, horiz) in [("X", p.x, true), ("Y", p.y, false)] {
        let (ids, primary) = (c.ids.clone(), id.to_string());
        let mixed = c.differs(|s, id| target(s, id).map(|p| if horiz { p.x } else { p.y }));
        let set = move |s: &mut App, v: i32| {
            let Some(base) = target(s, &primary) else { return };
            let d = if horiz { Point::new(v - base.x, 0) } else { Point::new(0, v - base.y) };
            for id in ids.iter() {
                s.doc.move_shape(id, d);
            }
        };
        spin(c, &body, label, value, (-9999.0, 9999.0), Edit::from(set).mixed(mixed));
    }

    // Offset only matters when the frame hangs off a line (non-static anchor).
    let anchored_to_line = o
        .str_("frame_anchor_ref")
        .and_then(|a| c.st.borrow().doc.get(a).map(|x| x.type_id() != 11))
        .unwrap_or(false);
    if anchored_to_line {
        let off = o.point("frame_offset").unwrap_or_default();
        let offset = |s: &App, id: &str| s.doc.get(id).and_then(|o| o.point("frame_offset")).unwrap_or_default();
        let set_x = c.each(|s, id, v: i32| {
            let cur = s.doc.get(id).and_then(|o| o.point("frame_offset")).unwrap_or_default();
            s.doc.set_value(id, "frame_offset", json!(format!("{},{}", v, cur.y)));
        });
        spin(c, &body, "Offset X", off.x, (-999.0, 999.0), Edit::from(set_x).mixed(c.differs(|s, id| offset(s, id).x)));
        let set_y = c.each(|s, id, v: i32| {
            let cur = s.doc.get(id).and_then(|o| o.point("frame_offset")).unwrap_or_default();
            s.doc.set_value(id, "frame_offset", json!(format!("{},{}", cur.x, v)));
        });
        spin(c, &body, "Offset Y", off.y, (-999.0, 999.0), Edit::from(set_y).mixed(c.differs(|s, id| offset(s, id).y)));
    }
}

/// Change which keypad point of the frame sits on the anchor, keeping the frame and its
/// custom content where they are.
fn set_frame_corner(s: &mut App, id: &str, k: i64) {
    let Some(o) = s.doc.get(id).cloned() else { return };
    let old = o.int_or("frame_corner", 1).clamp(1, 9);
    let Some(f) = s.scene().frame(id) else { return };
    let (Some((a, _)), Some((b, _))) = (f.keypad(old), f.keypad(k)) else { return };
    let d = b.sub(a);
    s.doc.set_value(id, "frame_corner", json!(k));
    // Static anchor: move the anchor point onto the new corner.
    if let Some(anchor) = o.str_("frame_anchor_ref").filter(|a| s.doc.get(a).is_some_and(|x| x.type_id() == 11)) {
        let pt = s.doc.get(anchor).and_then(|x| x.point("point")).unwrap_or_default();
        s.doc.set_value(anchor, "point", json!(format!("{},{}", pt.x + d.x, pt.y + d.y)));
    }
    let co = o.point("content_offset").unwrap_or_default();
    s.doc.set_value(id, "content_offset", json!(format!("{},{}", co.x - d.x, co.y - d.y)));
}

fn geometry(c: &Ctx, b: &gtk::Box, id: &str) {
    let body = section(b, "Geometry");
    let f = c.st.borrow().scene().frame(id).unwrap_or_default();
    let size = |s: &App, id: &str| s.scene().frame(id).map(|f| (f.w, f.h));
    let set_w = c.each(|s, id, v: i32| {
        let h = s.scene().frame(id).map(|f| f.h).unwrap_or(1);
        s.doc.resize_shape(id, v, h);
    });
    spin(c, &body, "Width", f.w, (1.0, 999.0), Edit::from(set_w).mixed(c.differs(|s, id| size(s, id).map(|z| z.0))));
    let set_h = c.each(|s, id, v: i32| {
        let w = s.scene().frame(id).map(|f| f.w).unwrap_or(1);
        s.doc.resize_shape(id, w, v);
    });
    spin(c, &body, "Height", f.h, (1.0, 999.0), Edit::from(set_h).mixed(c.differs(|s, id| size(s, id).map(|z| z.1))));
}

/// Table: row and column counts (adding or removing tracks at the end).
fn table(c: &Ctx, b: &gtk::Box, id: &str) {
    let body = section(b, "Table");
    let (rows, cols) = c.st.borrow().doc.table_tracks(id).unwrap_or_default();
    let count = |s: &App, id: &str, rows: bool| s.doc.table_tracks(id).map(|(r, c)| if rows { r.len() } else { c.len() });
    let (ui, st) = (c.ui.clone(), c.st.clone());
    let resize = move |rows: bool| {
        let (ui, st) = (ui.clone(), st.clone());
        move |s: &mut App, id: &str, v: i32| {
            // The frame size changes too: rebuild the inspector once this edit is done.
            let (ui, st) = (ui.clone(), st.clone());
            glib::idle_add_local_once(move || sync(&ui, &st));
            let canvas = s.canvas.clone();
            let want = v.max(1) as usize;
            while let Some(n) = count(s, id, rows).filter(|&n| n != want) {
                if n < want {
                    s.doc.insert_table_track(&canvas, id, rows, n);
                } else {
                    s.doc.delete_table_track(&canvas, id, rows, n - 1);
                }
                if count(s, id, rows) == Some(n) {
                    break;
                }
            }
        }
    };
    spin(c, &body, "Rows", rows.len() as i32, (1.0, 99.0), Edit::from(c.each(resize(true))).mixed(c.differs(|s, id| count(s, id, true))));
    spin(c, &body, "Columns", cols.len() as i32, (1.0, 99.0), Edit::from(c.each(resize(false))).mixed(c.differs(|s, id| count(s, id, false))));
    hint(&body, "Double-click a cell to edit its text; right-click a cell to insert or delete rows and columns.");
}

fn attachment(c: &Ctx, b: &gtk::Box, o: &serde_json::Map<String, serde_json::Value>) {
    let body = section(b, "Attachment Points");
    check(c, &body, "Attachable", o.boolean("attachable"), c.key("attachable", |v: bool| json!(v)));
}

const RECT_BORDERS: [(i64, &str); 5] = [(2, "Transparent"), (3, "Opaque"), (4, "Single (Light)"), (5, "Single (Heavy)"), (6, "Double")];

fn rect_border(c: &Ctx, b: &gtk::Box, o: &serde_json::Map<String, serde_json::Value>) {
    let body = section(b, "Border");
    check(c, &body, "Border", o.boolean("rectangular_border_enabled"), c.key("rectangular_border_enabled", |v: bool| json!(v)));
    let cur = RECT_BORDERS.iter().position(|k| k.0 == o.int_or("rectangular_border", 4)).unwrap_or(2);
    let labels: Vec<&str> = RECT_BORDERS.iter().map(|k| k.1).collect();
    dropdown(c, &body, "Type", &labels, cur, c.key("rectangular_border", |i: usize| json!(RECT_BORDERS[i].0)));
    color(c, &body, "Color", col(o, "mps_border_color"), c.key("mps_border_color", |v: u64| json!(v)));
    check(c, &body, "Blended", o.boolean("content_blended"), c.key("content_blended", |v: bool| json!(v)));
    // Part characters laid out around a box.
    let parts: [[Option<(&'static str, &str)>; 3]; 3] = [
        [Some(("border_tixel_tl", "Top-left")), Some(("border_tixel_ts", "Top")), Some(("border_tixel_tr", "Top-right"))],
        [Some(("border_tixel_ls", "Left")), None, Some(("border_tixel_rs", "Right"))],
        [Some(("border_tixel_bl", "Bottom-left")), Some(("border_tixel_bs", "Bottom")), Some(("border_tixel_br", "Bottom-right"))],
    ];
    body.append(&part_grid(c, o, &parts));
    hint(&body, "Empty part = default character. ╭╮╰╯ in the corners give rounded boxes.");
}

/// A 3×3 grid of one-character entries for border parts.
fn part_grid(c: &Ctx, o: &serde_json::Map<String, serde_json::Value>, parts: &[[Option<(&'static str, &str)>; 3]; 3]) -> gtk::Grid {
    let grid = gtk::Grid::new();
    grid.add_css_class("md-compact");
    grid.set_row_spacing(2);
    grid.set_column_spacing(2);
    grid.set_halign(gtk::Align::Center);
    for (y, r) in parts.iter().enumerate() {
        for (x, p) in r.iter().enumerate() {
            match p {
                Some((key, tip)) => {
                    let edit = c.key(key, |v: String| json!(v));
                    let tip = if edit.mixed { format!("{tip} ({MULTIPLE})") } else { tip.to_string() };
                    let e = char_box(c, o.str_(key).unwrap_or(""), edit);
                    e.set_tooltip_text(Some(&tip));
                    grid.attach(&e, x as i32, y as i32, 1, 1);
                }
                None => {
                    let l = gtk::Label::new(Some("▢"));
                    l.add_css_class("dim-label");
                    grid.attach(&l, x as i32, y as i32, 1, 1);
                }
            }
        }
    }
    grid
}

/// Dash pattern section. `flag_key` holds the enable flag and `dict_key` the {p,u,o} pattern
/// (rects store them under swapped names, SPEC §1.6).
fn dash(c: &Ctx, b: &gtk::Box, o: &serde_json::Map<String, serde_json::Value>, flag_key: &'static str, dict_key: &'static str) {
    let body = section(b, "Line Dash");
    let d = o.val(dict_key).and_then(|v| v.as_object()).cloned().unwrap_or_default();
    let g = |k: &str| d.get(k).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let set = c.each(move |s, id, v: bool| {
        s.doc.set_value(id, flag_key, json!(v));
        let cur = s.doc.get(id).and_then(|o| o.val(dict_key)).and_then(|v| v.as_object()).cloned().unwrap_or_default();
        let has = |k: &str| cur.get(k).and_then(|v| v.as_i64()).unwrap_or(0) > 0;
        if v && !(has("p") && has("u")) {
            s.doc.set_value(id, dict_key, json!({"p": 1, "u": 1, "o": 0}));
        }
    });
    check(c, &body, "Dash Pattern", o.boolean(flag_key), Edit::from(set).mixed(c.differs(|s, id| s.doc.get(id).is_some_and(|o| o.boolean(flag_key)))));
    for (label, k, min) in [("Segment", "p", 1.0), ("Gap", "u", 1.0), ("Phase", "o", -99.0)] {
        let mixed = c.differs(|s, id| s.doc.get(id).and_then(|o| o.val(dict_key)).and_then(|d| d.get(k)).and_then(|v| v.as_i64()).unwrap_or(0));
        let set = c.each(move |s, id, v: i32| {
            let mut cur = s.doc.get(id).and_then(|o| o.val(dict_key)).and_then(|v| v.as_object()).cloned().unwrap_or_default();
            for (dk, dv) in [("p", 1), ("u", 1), ("o", 0)] {
                cur.entry(dk).or_insert(json!(dv));
            }
            cur.insert(k.into(), json!(v));
            s.doc.set_value(id, dict_key, serde_json::Value::Object(cur));
        });
        spin(c, &body, label, g(k), (min, 99.0), Edit::from(set).mixed(mixed));
    }
}

const BACKGROUNDS: [(i64, &str); 3] = [(2, "Opaque"), (3, "Character"), (4, "Block")];

fn background(c: &Ctx, b: &gtk::Box, o: &serde_json::Map<String, serde_json::Value>) {
    let body = section(b, "Background");
    check(c, &body, "Fill", o.boolean("background_enabled"), c.key("background_enabled", |v: bool| json!(v)));
    let cur = BACKGROUNDS.iter().position(|k| k.0 == o.int_or("background_type", 2)).unwrap_or(0);
    let labels: Vec<&str> = BACKGROUNDS.iter().map(|k| k.1).collect();
    dropdown(c, &body, "Type", &labels, cur, c.key("background_type", |i: usize| json!(BACKGROUNDS[i].0)));
    char_entry(c, &body, "Character", o.str_("background_tixel").unwrap_or(""), c.key("background_tixel", |v: String| json!(v)));
    color(c, &body, "Color", col(o, "mps_bckg_color"), c.key("mps_bckg_color", |v: u64| json!(v)));
}

fn shadow(c: &Ctx, b: &gtk::Box, o: &serde_json::Map<String, serde_json::Value>) {
    let body = section(b, "Shadow");
    let set = c.each(|s, id, v: bool| {
        s.doc.set_value(id, "shadow_enabled", json!(v));
        if v && s.doc.get(id).and_then(|o| o.point("shadow_offset")).unwrap_or_default() == Point::default() {
            s.doc.set_value(id, "shadow_offset", json!("1,1"));
        }
    });
    check(c, &body, "Shadow", o.boolean("shadow_enabled"), Edit::from(set).mixed(c.differs(|s, id| s.doc.get(id).is_some_and(|o| o.boolean("shadow_enabled")))));
    dropdown(c, &body, "Intensity", &["Low", "Medium", "High", "Full"], (o.int_or("shadow_intensity", 4).clamp(1, 4) - 1) as usize, c.key("shadow_intensity", |i: usize| json!(i + 1)));
    let off = o.point("shadow_offset").unwrap_or_default();
    let offset = |s: &App, id: &str| s.doc.get(id).and_then(|o| o.point("shadow_offset")).unwrap_or_default();
    let set_x = c.each(|s, id, v: i32| {
        let cur = s.doc.get(id).and_then(|o| o.point("shadow_offset")).unwrap_or_default();
        s.doc.set_value(id, "shadow_offset", json!(format!("{},{}", v, cur.y)));
    });
    spin(c, &body, "X", off.x, (-99.0, 99.0), Edit::from(set_x).mixed(c.differs(|s, id| offset(s, id).x)));
    let set_y = c.each(|s, id, v: i32| {
        let cur = s.doc.get(id).and_then(|o| o.point("shadow_offset")).unwrap_or_default();
        s.doc.set_value(id, "shadow_offset", json!(format!("{},{}", cur.x, v)));
    });
    spin(c, &body, "Y", off.y, (-99.0, 99.0), Edit::from(set_y).mixed(c.differs(|s, id| offset(s, id).y)));
    char_entry(c, &body, "Character", o.str_("shadow_tixel").unwrap_or(""), c.key("shadow_tixel", |v: String| json!(v)));
    color(c, &body, "Color", col(o, "shadow_color"), c.key("shadow_color", |v: u64| json!(v)));
}

/// FIGlet fonts from every font directory's `fonts.mflist`: (id, display name), sorted by name.
fn letter_fonts() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for dir in solodraw_core::figlet::font_dirs() {
        let Ok(data) = std::fs::read_to_string(dir.join("fonts.mflist")) else { continue };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) else { continue };
        for f in v.get("fonts").and_then(|f| f.as_array()).into_iter().flatten() {
            let (Some(id), Some(name)) = (f.get("id").and_then(|x| x.as_str()), f.get("name").and_then(|x| x.as_str())) else { continue };
            if !out.iter().any(|(i, _)| i == id) {
                out.push((id.to_string(), name.to_string()));
            }
        }
    }
    out.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    out
}

fn text(c: &Ctx, b: &gtk::Box, id: &str) {
    let Some(tid) = c.st.borrow().doc.text_of(id) else { return };
    let to = c.st.borrow().doc.get(&tid).cloned().unwrap_or_default();
    let body = section(b, "Text");
    let edit = gtk::Button::with_label("Edit Text…");
    let (ui, st, id2) = (c.ui.clone(), c.st.clone(), id.to_string());
    edit.connect_clicked(move |_| edit_text_dialog(&ui, &st, &id2));
    body.append(&edit);
    check(c, &body, "Text", to.boolean("enabled"), c.text_key("enabled", |v: bool| json!(v)));
    color(c, &body, "Color", col(&to, "color"), c.text_key("color", |v: u64| json!(v)));
    segmented(c, &body, "Alignment", &["⇤", "≡", "⇥"], to.int_or("alignment", 1).clamp(0, 2) as usize, c.text_key("alignment", |i: usize| json!(i)));
    segmented(c, &body, "Position", &["⤒", "≡", "⤓"], to.int_or("position", 1).clamp(0, 2) as usize, c.text_key("position", |i: usize| json!(i)));
    segmented(c, &body, "Sweep", &["Horizontal", "Vertical"], to.int_or("sweep", 0).clamp(0, 1) as usize, c.text_key("sweep", |i: usize| json!(i)));
    segmented(c, &body, "Movement", &["↘", "↖"], to.int_or("movement", 0).clamp(0, 1) as usize, c.text_key("movement", |i: usize| json!(i)));
    check(c, &body, "Text inside border", to.boolean("inside_border"), c.text_key("inside_border", |v: bool| json!(v)));
    for (label, key) in [("Padding top", "inset_top"), ("Padding bottom", "inset_bottom"), ("Padding left", "inset_left"), ("Padding right", "inset_right")] {
        spin(c, &body, label, to.int_or(key, 0) as i32, (0.0, 99.0), c.text_key(key, |v: i32| json!(v)));
    }
    let fonts = Rc::new(letter_fonts());
    let cur_font = to.str_("letter_font_id").unwrap_or("");
    let cur = fonts.iter().position(|(i, _)| i == cur_font).map_or(0, |i| i + 1);
    let mut labels: Vec<&str> = vec!["None"];
    labels.extend(fonts.iter().map(|(_, n)| n.as_str()));
    let f2 = fonts.clone();
    let mixed = c.differs(|s, id| s.doc.text_of(id).and_then(|t| s.doc.get(&t)).and_then(|o| o.str_("letter_font_id").map(str::to_string)));
    let set = c.each(move |s, id, i: usize| {
        let Some(t) = s.doc.text_of(id) else { return };
        match i.checked_sub(1).and_then(|i| f2.get(i)) {
            Some((fid, name)) => {
                s.doc.set_value(&t, "letter_font_id", json!(fid));
                s.doc.set_value(&t, "letter_font_name", json!(name));
            }
            None => {
                if let Some(o) = s.doc.get_mut(&t) {
                    o.remove("letter_font_id");
                    o.remove("letter_font_name");
                }
            }
        }
    });
    dropdown(c, &body, "Font", &labels, cur, Edit::from(set).mixed(mixed));
}

fn content_color(c: &Ctx, b: &gtk::Box, o: &serde_json::Map<String, serde_json::Value>) {
    let body = section(b, "Content Color");
    color(c, &body, "Content Color", col(o, "content_color"), c.key("content_color", |v: u64| json!(v)));
}

fn ellipse_border(c: &Ctx, b: &gtk::Box, o: &serde_json::Map<String, serde_json::Value>) {
    let body = section(b, "Border");
    check(c, &body, "Border", o.boolean("rectangular_border_enabled"), c.key("rectangular_border_enabled", |v: bool| json!(v)));
    check(c, &body, "Blended", o.boolean("content_blended"), c.key("content_blended", |v: bool| json!(v)));
    color(c, &body, "Color", col(o, "mps_border_color"), c.key("mps_border_color", |v: u64| json!(v)));
    char_entry(c, &body, "Character", o.str_("border_tixel").unwrap_or(""), c.key("border_tixel", |v: String| json!(v)));
}

fn text_container(c: &Ctx, b: &gtk::Box, o: &serde_json::Map<String, serde_json::Value>) {
    let body = section(b, "Text Container");
    let sc = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.05);
    sc.set_value(o.float("text_container_control").unwrap_or(0.25));
    sc.set_hexpand(true);
    let Edit { f: set, mixed } = c.key("text_container_control", |v: f64| json!(v));
    if mixed {
        sc.set_tooltip_text(Some(MULTIPLE));
    }
    let (ui, st) = (c.ui.clone(), c.st.clone());
    sc.connect_value_changed(move |sc| {
        let v = sc.value();
        apply(&ui, &st, |s| set(s, v));
    });
    let r = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    r.append(&gtk::Label::new(Some("Horizontal")));
    r.append(&sc);
    r.append(&gtk::Label::new(Some("Vertical")));
    body.append(&r);
}

// ---------------------------------------------------------------------------
// Diamond

fn diamond_anchor(c: &Ctx, b: &gtk::Box, o: &serde_json::Map<String, serde_json::Value>) {
    let body = section(b, "Diamond Anchor");
    let items = ["Center", "Top", "Top (Outside)", "Bottom", "Bottom (Outside)", "Left", "Left (Outside)", "Right", "Right (Outside)"];
    let cur = (o.int_or("anchor_pos", 1).clamp(1, 9) - 1) as usize;
    let set = c.each(|s, id, i: usize| set_anchor_pos(s, id, i as i64 + 1));
    let mixed = c.differs(|s, id| s.doc.get(id).map(|o| o.int_or("anchor_pos", 1)));
    dropdown(c, &body, "Anchor Position", &items, cur, Edit::from(set).mixed(mixed));
    check(c, &body, "Attachable", o.boolean("attachable"), c.key("attachable", |v: bool| json!(v)));
}

/// Change the diamond's anchor position, keeping the diamond in place.
fn set_anchor_pos(s: &mut App, id: &str, k: i64) {
    let Some((before, _)) = s.scene().diamond_geometry(id) else { return };
    s.doc.set_value(id, "anchor_pos", json!(k));
    let Some((after, _)) = s.scene().diamond_geometry(id) else { return };
    let d = before.sub(after);
    let anchor = s.doc.get(id).and_then(|o| o.str_("anchor_point")).map(str::to_string);
    if let Some(a) = anchor.filter(|a| s.doc.get(a).is_some_and(|x| x.type_id() == 11)) {
        let pt = s.doc.get(&a).and_then(|x| x.point("point")).unwrap_or_default();
        s.doc.set_value(&a, "point", json!(format!("{},{}", pt.x + d.x, pt.y + d.y)));
    }
}

fn diamond_border(c: &Ctx, b: &gtk::Box, o: &serde_json::Map<String, serde_json::Value>) {
    let body = section(b, "Border");
    check(c, &body, "Border", o.boolean("rectangular_border_enabled"), c.key("rectangular_border_enabled", |v: bool| json!(v)));
    color(c, &body, "Color", col(o, "mps_border_color"), c.key("mps_border_color", |v: u64| json!(v)));
    let parts: [[Option<(&'static str, &str)>; 3]; 3] = [
        [Some(("btd_ul", "Upper-left edge")), Some(("bt_tp", "Top point")), Some(("btd_ur", "Upper-right edge"))],
        [Some(("bt_lp", "Left point")), None, Some(("bt_rp", "Right point"))],
        [Some(("btd_ll", "Lower-left edge")), Some(("bt_bp", "Bottom point")), Some(("btd_lr", "Lower-right edge"))],
    ];
    body.append(&part_grid(c, o, &parts));
}

// ---------------------------------------------------------------------------
// Line

const LINE_TYPES: [(i64, &str); 5] = [(3, "Diagram"), (2, "Automatic"), (4, "Manual"), (1, "Step"), (5, "Staggered")];
const LINE_STYLES: [(i64, &str); 5] = [(2, "Transparent"), (1, "Opaque"), (3, "Single (Light)"), (4, "Single (Heavy)"), (5, "Double")];
const HEADS: [(i64, &str); 28] = [
    (0, "None"),
    (1, "▶ Arrow (Smart)"),
    (2, "▷ Arrow (Smart)"),
    (20, "▲ Arrow Up"),
    (21, "▼ Arrow Down"),
    (22, "◀ Arrow Left"),
    (23, "▶ Arrow Right"),
    (24, "△ Arrow Up"),
    (25, "▽ Arrow Down"),
    (26, "◁ Arrow Left"),
    (27, "▷ Arrow Right"),
    (3, "■ Square"),
    (4, "□ Square"),
    (5, "▣ Square"),
    (6, "◆ Diamond"),
    (7, "◇ Diamond"),
    (8, "◈ Diamond"),
    (9, "● Circle"),
    (10, "○ Circle"),
    (11, "◌ Circle"),
    (12, "◎ Circle"),
    (13, "◍ Circle"),
    (14, "ER One"),
    (15, "ER Many"),
    (16, "ER One Only"),
    (17, "ER Zero or One"),
    (18, "ER One or Many"),
    (19, "ER Zero or Many"),
];

fn line(c: &Ctx, b: &gtk::Box, o: &serde_json::Map<String, serde_json::Value>) {
    let body = section(b, "Line");
    let cur = LINE_TYPES.iter().position(|k| k.0 == o.int_or("line_type", 3)).unwrap_or(0);
    let labels: Vec<&str> = LINE_TYPES.iter().map(|k| k.1).collect();
    dropdown(c, &body, "Type", &labels, cur, c.key("line_type", |i: usize| json!(LINE_TYPES[i].0)));
    let cur = LINE_STYLES.iter().position(|k| k.0 == o.int_or("line_style", 3)).unwrap_or(2);
    let labels: Vec<&str> = LINE_STYLES.iter().map(|k| k.1).collect();
    dropdown(c, &body, "Style", &labels, cur, c.key("line_style", |i: usize| json!(LINE_STYLES[i].0)));

    let head_labels: Vec<&str> = HEADS.iter().map(|h| h.1).collect();
    let ends = o.refs("segment_end_refs").into_iter().map(str::to_string).collect::<Vec<_>>();
    let mut er = false;
    for (label, last, tixel_key) in [("Start Head", false, "line_head_start_tixel"), ("End Head", true, "line_head_end_tixel")] {
        let Some(end) = (if last { ends.last() } else { ends.first() }) else { continue };
        let cur_h = c.st.borrow().doc.get(end).map(|e| e.int_or("head_type", 0)).unwrap_or(0);
        er |= (14..=19).contains(&cur_h);
        let cur = HEADS.iter().position(|h| h.0 == cur_h).unwrap_or(0);
        let head = move |s: &App, id: &str| {
            let o = s.doc.get(id)?;
            let ends = o.refs("segment_end_refs");
            let e = if last { ends.last() } else { ends.first() }?;
            Some(s.doc.get(e)?.int_or("head_type", 0))
        };
        let set = c.each(move |s, id, i: usize| {
            let ends = s.doc.get(id).map(|o| o.refs("segment_end_refs").into_iter().map(str::to_string).collect::<Vec<_>>()).unwrap_or_default();
            if let Some(e) = if last { ends.last() } else { ends.first() } {
                s.doc.set_value(e, "head_type", json!(HEADS[i].0));
            }
        });
        dropdown(c, &body, label, &head_labels, cur, Edit::from(set).mixed(c.differs(head)));
        char_entry(c, &body, &format!("{label} character"), o.str_(tixel_key).unwrap_or(""), c.key(tixel_key, |v: String| json!(v)));
    }
    let swap = gtk::Button::with_label("Swap Heads");
    let set = c.each(|s, id, _: ()| swap_heads(s, id));
    let (ui, st) = (c.ui.clone(), c.st.clone());
    swap.connect_clicked(move |_| {
        apply(&ui, &st, |s| set(s, ()));
        sync(&ui, &st);
    });
    body.append(&swap);
    if er {
        let cur = (o.int_or("head_notation", 2).clamp(1, 3) - 1) as usize;
        dropdown(c, &body, "Notation", &["Crow's Foot (Orthogonal)", "Crow's Foot (Diagonal)", "Crow's Foot (Hat)"], cur, c.key("head_notation", |i: usize| json!(i + 1)));
    }
    check(c, &body, "Attachable", o.boolean("attachable"), c.key("attachable", |v: bool| json!(v)));
    check(c, &body, "Blended", o.boolean("content_blended"), c.key("content_blended", |v: bool| json!(v)));
}

fn swap_heads(s: &mut App, id: &str) {
    let Some(o) = s.doc.get(id).cloned() else { return };
    let ends = o.refs("segment_end_refs");
    let (Some(a), Some(b)) = (ends.first(), ends.last()) else { return };
    let ha = s.doc.get(a).map(|e| e.int_or("head_type", 0)).unwrap_or(0);
    let hb = s.doc.get(b).map(|e| e.int_or("head_type", 0)).unwrap_or(0);
    s.doc.set_value(a, "head_type", json!(hb));
    s.doc.set_value(b, "head_type", json!(ha));
    let ta = o.str_("line_head_start_tixel").unwrap_or("").to_string();
    let tb = o.str_("line_head_end_tixel").unwrap_or("").to_string();
    s.doc.set_value(id, "line_head_start_tixel", json!(tb));
    s.doc.set_value(id, "line_head_end_tixel", json!(ta));
}
