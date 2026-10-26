//! Toolbar visibility and customisation: View ▸ Show Toolbar and View ▸ Customize Toolbar…
//! (docs/UI_INVENTORY.md §1.1). Prefs `toolbar.visible`, `toolbar.hidden` (item ids),
//! `toolbar.display` (0 icon and text, 1 icon only, 2 text only) and `toolbar.order` (the layout:
//! item ids plus `"separator"` / `"space"` entries) apply live to every window.

use std::cell::Cell;

use crate::prelude::*;

pub const VISIBLE: &str = "toolbar.visible";
pub const HIDDEN: &str = "toolbar.hidden";
pub const DISPLAY: &str = "toolbar.display";
pub const ORDER: &str = "toolbar.order";

struct Item {
    id: String,
    label: String,
    widget: gtk::Widget,
}

/// One position in the toolbar layout.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Slot {
    Item(usize),
    Sep(usize),
    Space(usize),
}

/// The toolbar box and its items in order. Items are grouped by the separators between them.
/// `bar` sits in a scroller that never forces the window wider; items cut off on the right are
/// offered from the `more` button at the end (like NSToolbar's overflow chevron).
pub struct Toolbar {
    /// What the window packs: the item row's scroller plus the overflow button.
    pub root: gtk::Box,
    pub bar: gtk::Box,
    scroller: gtk::ScrolledWindow,
    more: gtk::MenuButton,
    items: Vec<Item>,
    seps: Vec<gtk::Separator>,
    spaces: Vec<gtk::Box>,
    /// The built-in layout, in order.
    default: Vec<Slot>,
}

impl Toolbar {
    pub fn new() -> Self {
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let scroller = gtk::ScrolledWindow::new();
        // External: no scrollbar and no minimum width, so a narrow window clips the row.
        scroller.set_policy(gtk::PolicyType::External, gtk::PolicyType::Never);
        scroller.set_propagate_natural_height(true);
        scroller.set_hexpand(true);
        scroller.set_child(Some(&bar));
        if let Some(vp) = scroller.child().and_then(|c| c.downcast::<gtk::Viewport>().ok()) {
            vp.set_scroll_to_focus(false);
        }
        let more = gtk::MenuButton::new();
        more.set_icon_name("pan-down-symbolic");
        more.set_tooltip_text(Some("More toolbar items"));
        more.set_valign(gtk::Align::Center);
        more.set_visible(false);
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        root.add_css_class("md-toolbar");
        root.append(&scroller);
        root.append(&more);
        Toolbar { root, bar, scroller, more, items: Vec::new(), seps: Vec::new(), spaces: Vec::new(), default: Vec::new() }
    }

    /// Keep the overflow menu in step with the items that don't fit. Recomputed from an idle
    /// callback after the row's allocation changes (never during allocation).
    pub fn watch_overflow(&self) {
        let items: Vec<(String, gtk::Widget)> = self.items.iter().map(|i| (i.label.clone(), i.widget.clone())).collect();
        let seps: Vec<gtk::Widget> = self.seps.iter().map(|s| s.clone().upcast()).collect();
        let (sc, bar, more) = (self.scroller.clone(), self.bar.clone(), self.more.clone());
        let last: Rc<RefCell<Option<Vec<usize>>>> = Rc::new(RefCell::new(None));
        let update = Rc::new(move || {
            let page = sc.width() as f32;
            if page <= 0.0 {
                return;
            }
            let clipped = |w: &gtk::Widget| w.compute_bounds(&bar).is_some_and(|b| b.x() + b.width() > page + 0.5);
            let mut over: Vec<usize> = items.iter().enumerate().filter(|(_, (_, w))| w.get_visible() && clipped(w)).map(|(i, _)| i).collect();
            // In on-screen order (the layout can be reordered).
            let x = |i: &usize| items[*i].1.compute_bounds(&bar).map_or(0, |b| b.x() as i32);
            over.sort_by_key(x);
            // Clipped items stop drawing but keep their slot, so the layout doesn't change.
            for (_, w) in &items {
                w.set_child_visible(!clipped(w));
            }
            for w in &seps {
                w.set_child_visible(!clipped(w));
            }
            if last.borrow().as_ref() == Some(&over) {
                return;
            }
            more.set_visible(!over.is_empty());
            let pop = gtk::Popover::new();
            let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let entry = |label: &str, f: Box<dyn Fn()>| {
                let b = gtk::Button::with_label(label);
                b.add_css_class("flat");
                if let Some(l) = b.child() {
                    l.set_halign(gtk::Align::Start);
                }
                let p = pop.clone();
                b.connect_clicked(move |_| {
                    p.popdown();
                    f();
                });
                list.append(&b);
            };
            for &i in &over {
                let (label, w) = &items[i];
                if let Some(t) = w.downcast_ref::<gtk::ToggleButton>() {
                    let t = t.clone();
                    entry(label, Box::new(move || t.set_active(true)));
                } else if let Some(b) = w.downcast_ref::<gtk::Button>() {
                    let b = b.clone();
                    entry(label, Box::new(move || b.emit_clicked()));
                } else {
                    // The zoom box: offer its two actions.
                    for (l, a) in [("Zoom In", "win.zoom-in"), ("Zoom Out", "win.zoom-out")] {
                        let w = w.clone();
                        entry(l, Box::new(move || {
                            let _ = w.activate_action(a, None);
                        }));
                    }
                }
            }
            pop.set_child(Some(&list));
            more.set_popover(Some(&pop));
            *last.borrow_mut() = Some(over);
        });
        let pending = Rc::new(Cell::new(false));
        let schedule = move || {
            if pending.replace(true) {
                return;
            }
            let (u, p) = (update.clone(), pending.clone());
            glib::idle_add_local_once(move || {
                p.set(false);
                u();
            });
        };
        self.scroller.hadjustment().connect_changed(move |_| schedule());
    }

    /// Append an item; `id` is what the hidden-items and order prefs store.
    pub fn add(&mut self, id: &str, label: &str, w: &impl IsA<gtk::Widget>) {
        self.bar.append(w);
        self.default.push(Slot::Item(self.items.len()));
        self.items.push(Item { id: id.to_string(), label: label.to_string(), widget: w.clone().upcast() });
    }

    /// A separator line between groups.
    pub fn sep(&mut self) {
        let s = gtk::Separator::new(gtk::Orientation::Vertical);
        s.add_css_class("md-tool-sep");
        self.bar.append(&s);
        self.default.push(Slot::Sep(self.seps.len()));
        self.seps.push(s);
    }

    /// Flexible space (pushes what follows to the right).
    pub fn spacer(&mut self) {
        let s = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        s.set_hexpand(true);
        self.bar.append(&s);
        self.default.push(Slot::Space(self.spaces.len()));
        self.spaces.push(s);
    }

    fn widget(&self, slot: Slot) -> gtk::Widget {
        match slot {
            Slot::Item(i) => self.items[i].widget.clone(),
            Slot::Sep(i) => self.seps[i].clone().upcast(),
            Slot::Space(i) => self.spaces[i].clone().upcast(),
        }
    }

    fn slot_name(&self, slot: Slot) -> String {
        match slot {
            Slot::Item(i) => self.items[i].id.clone(),
            Slot::Sep(_) => "separator".into(),
            Slot::Space(_) => "space".into(),
        }
    }

    /// The layout from the order pref; the built-in one when the pref is missing or doesn't name
    /// exactly this toolbar's items, separators and spaces.
    fn layout(&self) -> Vec<Slot> {
        let names: Vec<String> =
            crate::prefs::get_value(ORDER).and_then(|v| v.as_array().cloned()).unwrap_or_default().iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
        let (mut seps, mut spaces) = (0, 0);
        let mut out = Vec::new();
        for n in &names {
            let slot = match n.as_str() {
                "separator" => {
                    seps += 1;
                    (seps <= self.seps.len()).then(|| Slot::Sep(seps - 1))
                }
                "space" => {
                    spaces += 1;
                    (spaces <= self.spaces.len()).then(|| Slot::Space(spaces - 1))
                }
                id => self.items.iter().position(|it| it.id == id).map(Slot::Item),
            };
            match slot {
                Some(s) if !out.contains(&s) => out.push(s),
                _ => return self.default.clone(),
            }
        }
        if out.len() == self.default.len() {
            out
        } else {
            self.default.clone()
        }
    }
}

fn save_layout(names: &[String]) {
    crate::prefs::set(ORDER, json!(names));
    apply_all();
}

fn hidden() -> Vec<String> {
    crate::prefs::get_value(HIDDEN).and_then(|v| v.as_array().cloned()).unwrap_or_default().iter().filter_map(|v| v.as_str().map(str::to_string)).collect()
}

/// Apply the toolbar prefs to one window's toolbar.
pub fn apply(tb: &Toolbar) {
    tb.root.set_visible(crate::prefs::get_bool(VISIBLE, true));
    let hidden = hidden();
    let display = crate::prefs::get_int(DISPLAY, 0);
    for it in &tb.items {
        let gated = it.id == "table" && !Tool::Table.available();
        it.widget.set_visible(!hidden.contains(&it.id) && !gated);
        // Tool and action buttons hold an icon over a label.
        if it.widget.is::<gtk::Button>() {
            if let Some(face) = it.widget.first_child() {
                if let (Some(icon), Some(label)) = (face.first_child(), face.last_child()) {
                    icon.set_visible(display != 2);
                    label.set_visible(display != 1);
                }
            }
        }
    }
    // Put the widgets in layout order.
    let layout = tb.layout();
    let mut prev: Option<gtk::Widget> = None;
    for &slot in &layout {
        let w = tb.widget(slot);
        tb.bar.reorder_child_after(&w, prev.as_ref());
        prev = Some(w);
    }
    // A separator shows only between visible items: one before it, and one after it before the
    // next separator or space. (`get_visible`: the item's own flag; `is_visible` also needs every
    // ancestor shown, which is false before the window maps.)
    let shown = |s: &Slot| matches!(s, Slot::Item(i) if tb.items[*i].widget.get_visible());
    for (k, slot) in layout.iter().enumerate() {
        if let Slot::Sep(i) = slot {
            let before = layout[..k].iter().any(shown);
            let after = layout[k + 1..].iter().take_while(|s| matches!(s, Slot::Item(_))).any(shown);
            tb.seps[*i].set_visible(before && after);
        }
    }
}

pub fn apply_all() {
    for_each_window(|u, _| apply(&u.toolbar));
}

/// View ▸ Show Toolbar.
pub fn set_visible(v: bool) {
    crate::prefs::set(VISIBLE, json!(v));
    apply_all();
}

/// View ▸ Customize Toolbar…: pick the display style and which items show.
pub fn customize_window(ui: &U) {
    let w = gtk::Window::builder().title("Customize Toolbar").transient_for(&ui.window).modal(true).default_width(360).build();
    let vb = gtk::Box::new(gtk::Orientation::Vertical, 8);
    vb.set_margin_top(12);
    vb.set_margin_bottom(12);
    vb.set_margin_start(12);
    vb.set_margin_end(12);

    let show = gtk::DropDown::from_strings(&["Icon and Text", "Icon Only", "Text Only"]);
    show.set_selected(crate::prefs::get_int(DISPLAY, 0).clamp(0, 2) as u32);
    show.connect_selected_notify(|d| {
        crate::prefs::set(DISPLAY, json!(d.selected()));
        apply_all();
    });
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.append(&gtk::Label::new(Some("Show:")));
    row.append(&show);
    vb.append(&row);

    // One row per layout entry: items have a show/hide check box, every row moves up/down.
    let tb = &ui.toolbar;
    let label_of: Rc<std::collections::HashMap<String, String>> = Rc::new(
        tb.items.iter().map(|it| (it.id.clone(), it.label.clone())).chain([("separator".into(), "Separator".into()), ("space".into(), "Flexible Space".into())]).collect(),
    );
    let names: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(tb.layout().into_iter().map(|s| tb.slot_name(s)).collect()));
    let defaults_names: Vec<String> = tb.default.iter().map(|&s| tb.slot_name(s)).collect();
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_child(Some(&list));
    scroll.set_min_content_height(360);
    scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    vb.append(&scroll);
    let fill: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::default();
    let f = {
        let (list, names, label_of, fill) = (list.clone(), names.clone(), label_of.clone(), fill.clone());
        Rc::new(move || {
            while let Some(c) = list.first_child() {
                list.remove(&c);
            }
            let hidden_now = hidden();
            let n = names.borrow().len();
            for (k, name) in names.borrow().iter().enumerate() {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                let label = label_of.get(name).cloned().unwrap_or_else(|| name.clone());
                if name == "separator" || name == "space" {
                    let l = gtk::Label::new(Some(&format!("— {label} —")));
                    l.set_halign(gtk::Align::Start);
                    l.set_margin_start(28);
                    l.set_hexpand(true);
                    row.append(&l);
                } else {
                    let cb = gtk::CheckButton::with_label(&label);
                    cb.set_active(!hidden_now.contains(name));
                    cb.set_hexpand(true);
                    let id = name.clone();
                    cb.connect_toggled(move |cb| {
                        let mut h = hidden();
                        h.retain(|x| *x != id);
                        if !cb.is_active() {
                            h.push(id.clone());
                        }
                        crate::prefs::set(HIDDEN, json!(h));
                        apply_all();
                    });
                    row.append(&cb);
                }
                for (icon, tip, to) in [("go-up-symbolic", "Move Up", k.checked_sub(1)), ("go-down-symbolic", "Move Down", (k + 1 < n).then_some(k + 1))] {
                    let b = gtk::Button::from_icon_name(icon);
                    b.add_css_class("flat");
                    b.set_tooltip_text(Some(&format!("{tip}: {label}")));
                    b.set_sensitive(to.is_some());
                    let (names, fill) = (names.clone(), fill.clone());
                    b.connect_clicked(move |_| {
                        let Some(to) = to else { return };
                        names.borrow_mut().swap(k, to);
                        save_layout(&names.borrow());
                        // Rebuild after this click handler returns.
                        let fill = fill.clone();
                        glib::idle_add_local_once(move || {
                            if let Some(f) = fill.borrow().clone() {
                                f();
                            }
                        });
                    });
                    row.append(&b);
                }
                list.append(&row);
            }
        })
    };
    *fill.borrow_mut() = Some(f.clone());
    f();

    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    let defaults = gtk::Button::with_label("Restore Defaults");
    let done = gtk::Button::with_label("Done");
    done.add_css_class("suggested-action");
    let show2 = show.clone();
    defaults.connect_clicked(move |_| {
        crate::prefs::set(HIDDEN, json!([]));
        *names.borrow_mut() = defaults_names.clone();
        save_layout(&defaults_names);
        show2.set_selected(0);
        f();
    });
    let w2 = w.clone();
    done.connect_clicked(move |_| w2.close());
    buttons.append(&defaults);
    buttons.append(&done);
    vb.append(&buttons);
    w.set_child(Some(&vb));
    w.present();
}
