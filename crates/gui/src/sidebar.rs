//! Left sidebar: utility bar + shape tree (docs/UI_INVENTORY.md §1.2).

use gtk::gio;
use solodraw_core::edit::shape_display_name;

use crate::icons::{icon, kind_icon};
use crate::prelude::*;

pub fn build_sidebar(ui_slot: &Rc<RefCell<Option<U>>>, st: &St) -> (gtk::Box, gtk::ListBox, gtk::DropDown) {
    let pane = gtk::Box::new(gtk::Orientation::Vertical, 0);
    pane.set_width_request(222);
    pane.add_css_class("md-sidebar");

    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    bar.set_halign(gtk::Align::Center);
    bar.set_margin_top(4);
    bar.set_margin_bottom(4);
    for (name, action, tip) in [
        ("md-group", "win.group", "Group"),
        ("md-ungroup", "win.ungroup", "Ungroup"),
        ("md-forward", "win.move-forward", "Move Forwards (Alt: to front)"),
        ("md-backward", "win.move-backward", "Move Backwards (Alt: to back)"),
    ] {
        let b = gtk::Button::new();
        b.set_child(Some(&icon(name, 18)));
        b.add_css_class("flat");
        b.set_tooltip_text(Some(tip));
        // Alt-click goes all the way to the front/back.
        let alt_action = match action {
            "win.move-forward" => Some("win.move-to-front"),
            "win.move-backward" => Some("win.move-to-back"),
            _ => None,
        };
        let click = gtk::GestureClick::new();
        click.set_propagation_phase(gtk::PropagationPhase::Capture);
        let b2 = b.clone();
        click.connect_pressed(move |g, _, _, _| {
            let alt = g.current_event_state().contains(gdk::ModifierType::ALT_MASK);
            let a = if alt { alt_action.unwrap_or(action) } else { action };
            let _ = b2.activate_action(a, None);
            g.set_state(gtk::EventSequenceState::Claimed);
        });
        b.add_controller(click);
        bar.append(&b);
    }
    pane.append(&bar);
    pane.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

    // Canvas switcher, shown only for documents with several canvases (Monodraw 1.7 has no
    // visible switcher, but its files can hold more than one).
    let canvases = gtk::DropDown::from_strings(&[]);
    canvases.set_margin_start(6);
    canvases.set_margin_end(6);
    canvases.set_margin_top(4);
    canvases.set_visible(false);
    canvases.set_tooltip_text(Some("Canvas"));
    {
        let (slot, s) = (ui_slot.clone(), st.clone());
        canvases.connect_selected_notify(move |d| {
            let Some(ui) = slot.borrow().clone() else { return };
            if ui.syncing.get() {
                return;
            }
            {
                let mut st = s.borrow_mut();
                let Some(c) = st.doc.canvases().get(d.selected() as usize).cloned() else { return };
                if c == st.canvas {
                    return;
                }
                st.canvas = c;
                st.selection.clear();
                st.cursor = None;
                st.rerender();
            }
            sync(&ui, &s);
        });
    }
    pane.append(&canvases);

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Multiple);
    list.add_css_class("navigation-sidebar");
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_child(Some(&list));
    scroll.set_vexpand(true);
    scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    pane.append(&scroll);

    let (slot, s) = (ui_slot.clone(), st.clone());
    list.connect_selected_rows_changed(move |list| {
        let Some(ui) = slot.borrow().clone() else { return };
        if ui.syncing.get() {
            return;
        }
        let rows: Vec<String> = list.selected_rows().iter().map(|r| r.widget_name().to_string()).collect();
        {
            // Keep the existing order (primary last); append newly selected rows.
            let mut st = s.borrow_mut();
            st.selection.retain(|x| rows.contains(x));
            for r in rows {
                if !st.is_selected(&r) {
                    st.selection.push(r);
                }
            }
        }
        rebuild_inspector(&ui, &s);
        refresh(&ui, &s.borrow());
    });
    (pane, list, canvases)
}

/// Shapes in sidebar order: depth-first, top of the z-order first.
fn rows(st: &App) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    fn walk(st: &App, id: &str, depth: usize, out: &mut Vec<(String, usize)>) {
        let Some(o) = st.doc.get(id) else { return };
        for k in o.refs("subshape_refs") {
            out.push((k.to_string(), depth));
            let expandable = st.doc.get(k).is_some_and(|c| !c.refs("subshape_refs").is_empty());
            if expandable && st.expanded.contains(k) {
                walk(st, k, depth + 1, out);
            }
        }
    }
    walk(st, &st.canvas, 0, &mut out);
    out
}

pub fn shape_label(st: &App, id: &str) -> String {
    let Some(o) = st.doc.get(id) else { return String::new() };
    match o.str_("name").filter(|n| !n.is_empty()) {
        Some(n) => n.to_string(),
        None => shape_display_name(o.int_or("shape_type", 0)).to_string(),
    }
}

pub fn rebuild_sidebar(ui: &U, st: &St) {
    ui.syncing.set(true);
    {
        // Expand ancestors of the selection so it is visible.
        let mut s = st.borrow_mut();
        for sel in s.selection.clone() {
            let mut cur = s.scene().parent(&sel).map(str::to_string);
            while let Some(p) = cur {
                if p == s.canvas {
                    break;
                }
                s.expanded.insert(p.clone());
                cur = s.scene().parent(&p).map(str::to_string);
            }
        }
    }
    while let Some(c) = ui.sidebar.first_child() {
        ui.sidebar.remove(&c);
    }
    let s = st.borrow();
    {
        let ids = s.doc.canvases();
        let names: Vec<String> = ids
            .iter()
            .enumerate()
            .map(|(i, c)| s.doc.get(c).and_then(|o| o.str_("name")).map(str::to_string).unwrap_or_else(|| format!("Canvas {}", i + 1)))
            .collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        ui.canvas_picker.set_model(Some(&gtk::StringList::new(&names)));
        ui.canvas_picker.set_selected(ids.iter().position(|c| *c == s.canvas).unwrap_or(0) as u32);
        ui.canvas_picker.set_visible(ids.len() > 1);
    }
    let mut selected_rows = Vec::new();
    let all = rows(&s);
    // "Animate sidebar" (Monodraw 1.6.1 capture, golden-gui step 06): on an incremental change
    // (some rows were already shown) a new row slides in from the right with ease-out in about
    // 0.15 s while the rows below move down; a deleted row slides out to the left while the rows
    // below move up (about 0.3 s); rows that vanish because new rows replaced them (grouping) fade
    // out instead. A full reload (new document, other canvas) just shows.
    let now: Vec<(String, usize, String)> = all.iter().map(|(id, d)| (id.clone(), *d, shape_label(&s, id))).collect();
    let before_rows = ui.sidebar_rows.replace(now);
    let before: std::collections::HashSet<String> = before_rows.iter().map(|(id, _, _)| id.clone()).collect();
    let current: std::collections::HashSet<String> = all.iter().map(|(id, _)| id.clone()).collect();
    let incremental = all.iter().any(|(id, _)| before.contains(id));
    let animate = incremental && crate::prefs::get_bool(crate::prefs::SIDEBAR_ANIMATED, true);
    let added_any = all.iter().any(|(id, _)| !before.contains(id));
    for (id, depth) in all {
        let Some(o) = s.doc.get(&id) else { continue };
        let row = gtk::ListBoxRow::new();
        row.set_widget_name(&id);
        let hb = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        hb.set_margin_start(4 + depth as i32 * 14);

        let has_kids = !o.refs("subshape_refs").is_empty();
        let exp = gtk::Button::new();
        exp.add_css_class("flat");
        exp.add_css_class("md-expander");
        if has_kids {
            exp.set_icon_name(if s.expanded.contains(&id) { "pan-down-symbolic" } else { "pan-end-symbolic" });
            let (u, st2, id2) = (ui.clone(), st.clone(), id.clone());
            exp.connect_clicked(move |_| {
                {
                    let mut s = st2.borrow_mut();
                    if !s.expanded.remove(&id2) {
                        s.expanded.insert(id2.clone());
                    }
                }
                rebuild_sidebar(&u, &st2);
            });
        } else {
            exp.set_sensitive(false);
            exp.set_opacity(0.0);
        }
        hb.append(&exp);
        hb.append(&icon(kind_icon(o.type_id(), o.int_or("shape_type", 0)), 16));

        let name = gtk::Label::new(Some(&shape_label(&s, &id)));
        name.set_halign(gtk::Align::Start);
        name.set_hexpand(true);
        name.set_ellipsize(pango::EllipsizeMode::End);
        let entry = gtk::Entry::new();
        entry.set_text(&shape_label(&s, &id));
        entry.set_width_chars(4);
        entry.set_max_width_chars(4);
        let stack = gtk::Stack::new();
        stack.add_named(&name, Some("label"));
        stack.add_named(&entry, Some("entry"));
        stack.set_hexpand(true);
        hb.append(&stack);
        {
            let (u, st2, id2, stack2) = (ui.clone(), st.clone(), id.clone(), stack.clone());
            entry.connect_activate(move |e| {
                let v = e.text().to_string();
                {
                    let mut s = st2.borrow_mut();
                    s.checkpoint();
                    s.doc.set_value(&id2, "name", json!(v));
                    s.doc.set_value(&id2, "name_edited", json!(true));
                }
                stack2.set_visible_child_name("label");
                sync(&u, &st2);
            });
            let stack3 = stack.clone();
            let keys = gtk::EventControllerKey::new();
            keys.connect_key_pressed(move |_, k, _, _| {
                if k == gdk::Key::Escape {
                    stack3.set_visible_child_name("label");
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            });
            entry.add_controller(keys);
        }

        for (key, on, off) in [("locked", "md-lock", "md-unlock"), ("hidden", "md-eye-off", "md-eye")] {
            let active = o.boolean(key);
            let b = gtk::Button::new();
            b.set_child(Some(&icon(if active { on } else { off }, 14)));
            b.add_css_class("flat");
            b.add_css_class("md-row-toggle");
            b.set_tooltip_text(Some(match (key, active) {
                ("locked", true) => "Unlock",
                ("locked", false) => "Lock",
                (_, true) => "Show",
                _ => "Hide",
            }));
            if !active {
                b.add_css_class("md-dim");
            }
            let (u, st2, id2) = (ui.clone(), st.clone(), id.clone());
            b.connect_clicked(move |_| {
                st2.borrow_mut().toggle_flag(&id2, key);
                sync(&u, &st2);
            });
            hb.append(&b);
        }
        if animate && !before.contains(&id) {
            // Height opens (rows below move down) while the content slides in from the right.
            let (outer, inner) = (gtk::Revealer::new(), gtk::Revealer::new());
            outer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
            inner.set_transition_type(gtk::RevealerTransitionType::SlideLeft);
            for r in [&outer, &inner] {
                r.set_transition_duration(150);
            }
            inner.set_child(Some(&hb));
            outer.set_child(Some(&inner));
            row.set_child(Some(&outer));
            glib::idle_add_local_once(move || {
                outer.set_reveal_child(true);
                inner.set_reveal_child(true);
            });
        } else {
            row.set_child(Some(&hb));
        }

        // Hover → highlight on canvas.
        let motion = gtk::EventControllerMotion::new();
        let (u, st2, id2) = (ui.clone(), st.clone(), id.clone());
        motion.connect_enter(move |_, _, _| {
            st2.borrow_mut().hover = Some(id2.clone());
            u.area.queue_draw();
        });
        let (u, st2) = (ui.clone(), st.clone());
        motion.connect_leave(move |_| {
            st2.borrow_mut().hover = None;
            u.area.queue_draw();
        });
        row.add_controller(motion);

        // Double-click renames; right-click opens the context menu.
        let click = gtk::GestureClick::new();
        click.set_button(0);
        let (u, st2, id2, stack2, entry2, row2) = (ui.clone(), st.clone(), id.clone(), stack.clone(), entry.clone(), row.clone());
        click.connect_pressed(move |g, n, x, y| {
            if g.current_button() == 3 {
                // Anchor on the pane: selecting rebuilds the rows, destroying this one.
                let (px, py) = row2.translate_coordinates(&u.sidebar_pane, x, y).unwrap_or((x, y));
                if !st2.borrow().is_selected(&id2) {
                    st2.borrow_mut().select_only(Some(id2.clone()));
                    sync(&u, &st2);
                }
                popup_menu(&u.sidebar_pane, &shape_menu(), px, py);
            } else if n == 2 {
                stack2.set_visible_child_name("entry");
                entry2.grab_focus();
            }
        });
        row.add_controller(click);

        ui.sidebar.append(&row);
        if s.is_selected(&id) {
            selected_rows.push(row);
        }
    }
    drop(s);
    if animate {
        // Ghost rows for shapes that went away, at their old place, animated out then removed.
        let mut index = 0;
        for (id, depth, label) in &before_rows {
            if current.contains(id) {
                index += 1;
                continue;
            }
            let row = gtk::ListBoxRow::new();
            row.set_selectable(false);
            row.set_activatable(false);
            let l = gtk::Label::new(Some(label));
            l.set_xalign(0.0);
            l.set_margin_start(4 + *depth as i32 * 14 + 22);
            let (outer, inner) = (gtk::Revealer::new(), gtk::Revealer::new());
            outer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
            // Deleted: slide out to the left (hiding a SlideRight revealer moves it left).
            // Replaced (grouping): fade out.
            inner.set_transition_type(if added_any {
                gtk::RevealerTransitionType::Crossfade
            } else {
                gtk::RevealerTransitionType::SlideRight
            });
            let ms = if added_any { 200 } else { 300 };
            for r in [&outer, &inner] {
                r.set_transition_duration(ms);
                r.set_reveal_child(true);
            }
            inner.set_child(Some(&l));
            outer.set_child(Some(&inner));
            row.set_child(Some(&outer));
            ui.sidebar.insert(&row, index);
            index += 1;
            glib::idle_add_local_once(move || {
                outer.set_reveal_child(false);
                inner.set_reveal_child(false);
            });
            let (list, r) = (ui.sidebar.clone(), row.clone());
            glib::timeout_add_local_once(std::time::Duration::from_millis(ms as u64 + 50), move || {
                if r.parent().is_some() {
                    list.remove(&r);
                }
            });
        }
    }
    ui.sidebar.unselect_all();
    for row in &selected_rows {
        ui.sidebar.select_row(Some(row));
    }
    ui.syncing.set(false);
}

/// A menu section of (label, action) items.
pub fn menu_section(items: &[(&str, &str)]) -> gio::Menu {
    let s = gio::Menu::new();
    for (l, a) in items {
        s.append(Some(l), Some(a));
    }
    s
}

/// Shape context menu (sidebar rows and the canvas), docs/UI_INVENTORY.md §1.2.
pub fn shape_menu() -> gio::Menu {
    let m = gio::Menu::new();
    let sec = menu_section;
    m.append_section(None, &sec(&[("Locate", "win.locate")]));
    m.append_section(None, &sec(&[("Lock / Unlock", "win.lock"), ("Hide / Unhide", "win.hide")]));
    m.append_section(None, &sec(&[("Copy Style", "win.copy-style"), ("Paste Style", "win.paste-style")]));
    m.append_section(None, &sec(&[("Duplicate", "win.duplicate"), ("Delete", "win.delete"), ("Reset Content", "win.reset-content")]));
    m.append_section(
        None,
        &sec(&[
            ("Move Forwards", "win.move-forward"),
            ("Move Backwards", "win.move-backward"),
            ("Move to Front", "win.move-to-front"),
            ("Move to Back", "win.move-to-back"),
        ]),
    );
    m.append_section(None, &sec(&[("Group", "win.group"), ("Ungroup", "win.ungroup")]));
    m
}

/// Pop up `m` at (x, y) in `anchor`'s coordinates.
pub fn popup_menu(anchor: &impl IsA<gtk::Widget>, m: &gio::Menu, x: f64, y: f64) {
    let p = gtk::PopoverMenu::from_model(Some(m));
    p.set_parent(anchor);
    p.set_has_arrow(false);
    p.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    p.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    p.popup();
}
