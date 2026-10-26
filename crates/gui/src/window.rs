//! Document window construction (docs/UI_INVENTORY.md §1).

use std::cell::Cell;

use crate::assistant::build_assistant;
use crate::icons::icon;
use crate::prelude::*;

const CSS: &str = r#"
.md-toolbar { padding: 2px 6px; border-bottom: 1px solid alpha(currentColor, 0.12); }
.md-toolbar button { padding: 2px 4px; min-width: 44px; }
.md-toolbar button label { font-size: 0.78em; }
.md-toolbar button:checked { background: alpha(currentColor, 0.14); }
.md-tool-sep { margin: 6px 4px; }
.md-sidebar { border-right: 1px solid alpha(currentColor, 0.12); }
.md-assistant { border-left: 1px solid alpha(currentColor, 0.12); }
.md-sidebar list.navigation-sidebar { padding: 2px 0; }
.md-sidebar list.navigation-sidebar > row { padding: 0 4px; margin: 0 4px; min-height: 22px; }
.md-sidebar button.md-expander, .md-sidebar button.md-row-toggle { padding: 0; margin: 0; min-width: 18px; min-height: 18px; }
.md-sidebar row entry { min-height: 18px; padding: 0 4px; }
.md-dim { opacity: 0.3; }
.md-sidebar row:hover .md-dim { opacity: 0.7; }
.md-char-button { font-family: monospace; font-size: 1.15em; padding: 2px 0; min-width: 24px; min-height: 24px; border-radius: 3px; }
.md-pencil-char { font-family: monospace; font-size: 1.4em; padding: 0 8px; border: 1px solid alpha(currentColor, 0.25); border-radius: 4px; }
.md-ruler { background: alpha(currentColor, 0.03); }
.md-status { font-size: 0.85em; }
.md-assistant stackswitcher button { padding: 2px 8px; min-height: 22px; font-size: 0.9em; }
.md-section-title { font-weight: bold; }
.md-compact button { min-height: 16px; min-width: 20px; padding: 0 2px; }
.md-compact entry { min-height: 20px; min-width: 22px; padding: 0 2px; font-family: monospace; }
.md-align button { min-height: 22px; min-width: 28px; padding: 0; }
.md-snippet-preview { font-family: monospace; font-size: 7pt; line-height: 1; opacity: 0.85; }
"#;

fn install_css() {
    let p = gtk::CssProvider::new();
    p.load_from_data(CSS);
    if let Some(d) = gdk::Display::default() {
        gtk::StyleContext::add_provider_for_display(&d, &p, gtk::STYLE_PROVIDER_PRIORITY_USER + 1);
    }
}

/// Toolbar button: icon above a small label.
fn tool_button(icon_name: &'static str, label: &str, tip: &str) -> gtk::ToggleButton {
    let b = gtk::ToggleButton::new();
    b.set_child(Some(&button_face(icon_name, label)));
    b.set_tooltip_text(Some(tip));
    b.add_css_class("flat");
    b
}

fn button_face(icon_name: &'static str, label: &str) -> gtk::Box {
    let vb = gtk::Box::new(gtk::Orientation::Vertical, 1);
    vb.append(&icon(icon_name, 22));
    vb.append(&gtk::Label::new(Some(label)));
    vb
}

fn action_button(icon_name: &'static str, label: &str, tip: &str, action: &str) -> gtk::Button {
    let b = gtk::Button::new();
    b.set_child(Some(&button_face(icon_name, label)));
    b.set_tooltip_text(Some(tip));
    b.add_css_class("flat");
    b.set_action_name(Some(action));
    b
}

thread_local! {
    /// Every open document window, for applying preference changes live.
    static WINDOWS: RefCell<Vec<(std::rc::Weak<Ui>, std::rc::Weak<RefCell<App>>)>> = const { RefCell::new(Vec::new()) };
}

/// Run `f` for every open document window.
pub fn for_each_window(f: impl Fn(&U, &St)) {
    let live: Vec<(U, St)> = WINDOWS.with(|w| {
        let mut w = w.borrow_mut();
        w.retain(|(u, s)| u.strong_count() > 0 && s.strong_count() > 0);
        w.iter().filter_map(|(u, s)| Some((u.upgrade()?, s.upgrade()?))).collect()
    });
    for (u, s) in live {
        f(&u, &s);
    }
}

/// The app icons (original artwork, `data/icons`), written to a private icon-theme folder so the
/// window icon works without installing anything.
const ICONS: [(&str, &str); 2] =
    [("solodraw", include_str!("../../../data/icons/solodraw.svg")), ("solodraw-dark", include_str!("../../../data/icons/solodraw-dark.svg"))];

pub fn install_icons() {
    let root = glib::user_data_dir().join("solodraw").join("icons");
    let dir = root.join("hicolor").join("scalable").join("apps");
    let _ = std::fs::create_dir_all(&dir);
    for (name, svg) in ICONS {
        let path = dir.join(format!("{name}.svg"));
        if std::fs::read_to_string(&path).ok().as_deref() != Some(svg) {
            let _ = std::fs::write(&path, svg);
        }
    }
    if let Some(display) = gdk::Display::default() {
        gtk::IconTheme::for_display(&display).add_search_path(&root);
    }
}

thread_local! {
    static ICON: RefCell<Option<&'static str>> = const { RefCell::new(None) };
}

/// "Use dark icon in dark mode" (on by default): the dark icon while the app is dark, else the
/// normal one. Set only when it changes; re-run on appearance changes.
pub fn apply_icon() {
    let dark = gtk::Settings::default().is_some_and(|s| s.is_gtk_application_prefer_dark_theme());
    let name = if dark && crate::prefs::get_bool(crate::prefs::DARK_ICON, true) { "solodraw-dark" } else { "solodraw" };
    if ICON.with(|c| c.replace(Some(name))) == Some(name) {
        return;
    }
    gtk::Window::set_default_icon_name(name);
    for_each_window(|ui, _| ui.window.set_icon_name(Some(name)));
}

pub fn build(gapp: &gtk::Application, file: Option<PathBuf>) {
    static CSS_ONCE: std::sync::Once = std::sync::Once::new();
    CSS_ONCE.call_once(install_css);
    if gapp.menubar().is_none() {
        gapp.set_menubar(Some(&crate::actions::menu_model()));
        crate::actions::set_accels(gapp);
        let quit = gtk::gio::SimpleAction::new("quit", None);
        let a = gapp.clone();
        quit.connect_activate(move |_, _| {
            for w in a.windows() {
                w.close();
            }
        });
        gapp.add_action(&quit);
    }

    let window = gtk::ApplicationWindow::builder().application(gapp).default_width(1280).default_height(820).title("solodraw").build();
    window.set_show_menubar(true);

    // Toolbar.
    let mut toolbar = crate::toolbar::Toolbar::new();
    let mut tools = Vec::new();
    let mut first: Option<gtk::ToggleButton> = None;
    for (i, (tool, label, tip, _, icon_name, implemented)) in Tool::ALL.iter().enumerate() {
        if matches!(i, 2 | 9) {
            toolbar.sep();
        }
        let b = tool_button(icon_name, label, if *implemented { tip } else { "Not available yet" });
        b.set_sensitive(*implemented);
        if let Some(f) = &first {
            b.set_group(Some(f));
        } else {
            b.set_active(true);
            first = Some(b.clone());
        }
        toolbar.add(&label.to_lowercase().replace(' ', "-"), label, &b);
        tools.push((*tool, b));
    }
    toolbar.sep();
    for (ic, label, tip, action) in [
        ("md-group", "Group", "Group shapes", "win.group"),
        ("md-ungroup", "Ungroup", "Ungroup shapes", "win.ungroup"),
        ("md-edit", "Edit", "Edit Shape", "win.edit-shape"),
        ("md-image", "Image", "Image overlay", "win.insert-image"),
        ("md-backward", "Backward", "Move Backwards", "win.move-backward"),
        ("md-forward", "Forward", "Move Forwards", "win.move-forward"),
    ] {
        toolbar.add(&label.to_lowercase(), label, &action_button(ic, label, tip, action));
    }
    toolbar.spacer();

    let zoom_box = gtk::Box::new(gtk::Orientation::Vertical, 1);
    let zoom_btns = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    zoom_btns.add_css_class("linked");
    for (ic, action, tip) in [("zoom-out-symbolic", "win.zoom-out", "Zoom Out"), ("zoom-in-symbolic", "win.zoom-in", "Zoom In")] {
        let b = gtk::Button::from_icon_name(ic);
        b.set_action_name(Some(action));
        b.set_tooltip_text(Some(tip));
        zoom_btns.append(&b);
    }
    let zoom_label = gtk::Label::new(Some("100%"));
    zoom_box.append(&zoom_btns);
    zoom_box.append(&zoom_label);
    zoom_box.set_valign(gtk::Align::Center);
    zoom_box.set_margin_end(6);
    toolbar.add("zoom", "Zoom", &zoom_box);
    toolbar.add("chars", "Chars", &action_button("md-chars", "Chars", "Characters", "win.tab-characters"));
    toolbar.add("export", "Export", &action_button("md-export", "Export", "Export as Text", "win.export"));
    toolbar.sep();
    toolbar.add("sidebar", "Sidebar", &action_button("md-sidebar", "Sidebar", "Toggle Sidebar", "win.show-sidebar"));
    toolbar.add("assistant", "Assistant", &action_button("md-assistant", "Assistant", "Toggle Assistant", "win.show-assistant"));

    // Canvas with rulers.
    let area = gtk::DrawingArea::new();
    area.set_focusable(true);
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_child(Some(&area));
    if let Some(vp) = scroller.child().and_then(|c| c.downcast::<gtk::Viewport>().ok()) {
        // Clicking the canvas grabs focus; don't let that scroll the view.
        vp.set_scroll_to_focus(false);
    }
    scroller.set_hexpand(true);
    scroller.set_vexpand(true);
    let ruler_top = gtk::DrawingArea::new();
    ruler_top.set_content_height(18);
    ruler_top.add_css_class("md-ruler");
    let ruler_left = gtk::DrawingArea::new();
    ruler_left.set_content_width(40);
    ruler_left.add_css_class("md-ruler");
    let corner = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let grid = gtk::Grid::new();
    grid.attach(&corner, 0, 0, 1, 1);
    grid.attach(&ruler_top, 1, 0, 1, 1);
    grid.attach(&ruler_left, 0, 1, 1, 1);
    grid.attach(&scroller, 1, 1, 1, 1);
    corner.set_size_request(40, -1);

    let st: St = Rc::new(RefCell::new(App::empty()));
    let slot: Rc<RefCell<Option<U>>> = Rc::new(RefCell::new(None));
    let (sidebar_pane, sidebar, canvas_picker) = build_sidebar(&slot, &st);
    let parts = build_assistant(&slot, &st);

    let inner = gtk::Paned::new(gtk::Orientation::Horizontal);
    inner.set_start_child(Some(&grid));
    inner.set_end_child(Some(&parts.pane));
    inner.set_resize_end_child(false);
    inner.set_shrink_end_child(false);
    inner.set_shrink_start_child(false);
    let outer = gtk::Paned::new(gtk::Orientation::Horizontal);
    outer.set_start_child(Some(&sidebar_pane));
    outer.set_end_child(Some(&inner));
    outer.set_resize_start_child(false);
    outer.set_shrink_start_child(false);
    outer.set_shrink_end_child(false);
    outer.set_position(222);
    inner.set_position(1280 - 222 - 246);

    let status = gtk::Label::new(None);
    status.set_halign(gtk::Align::Start);
    status.set_margin_start(8);
    status.set_margin_top(2);
    status.set_margin_bottom(2);
    status.add_css_class("dim-label");
    status.add_css_class("md-status");

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
    toolbar.watch_overflow();
    vbox.append(&toolbar.root);
    vbox.append(&outer);
    outer.set_vexpand(true);
    vbox.append(&status);
    window.set_child(Some(&vbox));

    let ui = Rc::new(Ui {
        window: window.clone(),
        area: area.clone(),
        scroller: scroller.clone(),
        ruler_top: ruler_top.clone(),
        ruler_left: ruler_left.clone(),
        inspector: parts.inspector,
        assistant: parts.stack,
        sidebar,
        sidebar_pane,
        canvas_picker,
        assistant_pane: parts.pane,
        outer_pane: outer.clone(),
        inner_pane: inner.clone(),
        status,
        zoom_label,
        pencil_label: parts.pencil_label,
        snippet_search: parts.snippet_search,
        char_search: parts.char_search,
        tools,
        toolbar,
        syncing: Cell::new(false),
        sidebar_rows: Default::default(),
        dialog: RefCell::new(None),
    });
    *slot.borrow_mut() = Some(ui.clone());
    WINDOWS.with(|w| w.borrow_mut().push((Rc::downgrade(&ui), Rc::downgrade(&st))));
    ui.area.set_font_options(Some(&crate::prefs::font_options()));
    crate::actions::install(&ui, &st);
    crate::toolbar::apply(&ui.toolbar);
    {
        // Corner visibility follows the rulers.
        let c = corner.clone();
        let (rt, rl) = (ruler_top.clone(), ruler_left.clone());
        let upd = move || c.set_visible(rt.is_visible() && rl.is_visible());
        let u2 = upd.clone();
        ruler_top.connect_visible_notify(move |_| upd());
        ruler_left.connect_visible_notify(move |_| u2());
    }

    for (tool, b) in &ui.tools {
        let (ui2, st2, tool) = (ui.clone(), st.clone(), *tool);
        b.connect_toggled(move |b| {
            // set_tool() updates the state before activating the button; only user clicks get here.
            if b.is_active() && st2.borrow().tool != tool {
                let shift = b
                    .display()
                    .default_seat()
                    .and_then(|seat| seat.keyboard())
                    .is_some_and(|k| k.modifier_state().contains(gdk::ModifierType::SHIFT_MASK));
                choose_tool(&ui2, &st2, tool, shift && tool != Tool::Select);
                ui2.area.grab_focus();
            }
        });
    }

    // Canvas drawing and input.
    let s = st.clone();
    area.set_draw_func(move |a, cr, _, _| draw(a, cr, &s.borrow()));
    let (u, s) = (ui.clone(), st.clone());
    ruler_top.set_draw_func(move |w, cr, _, _| draw_ruler_top(w, cr, &u, &s.borrow()));
    let (u, s) = (ui.clone(), st.clone());
    ruler_left.set_draw_func(move |w, cr, _, _| draw_ruler_left(w, cr, &u, &s.borrow()));
    let rt = ruler_top.clone();
    scroller.hadjustment().connect_value_changed(move |_| rt.queue_draw());
    let rl = ruler_left.clone();
    scroller.vadjustment().connect_value_changed(move |_| rl.queue_draw());

    let drag = gtk::GestureDrag::new();
    drag.set_button(0);
    let (u, s) = (ui.clone(), st.clone());
    drag.connect_drag_begin(move |g, x, y| {
        u.area.grab_focus();
        let shift = g.current_event_state().contains(gdk::ModifierType::SHIFT_MASK);
        drag_begin(&u, &s, x, y, shift, g.current_button());
    });
    let (u, s) = (ui.clone(), st.clone());
    drag.connect_drag_update(move |g, dx, dy| {
        if let Some((x, y)) = g.start_point() {
            drag_update(&u, &s, x + dx, y + dy);
        }
    });
    let (u, s) = (ui.clone(), st.clone());
    drag.connect_drag_end(move |_, _, _| drag_end(&u, &s));
    area.add_controller(drag);

    let click = gtk::GestureClick::new();
    let (u, s) = (ui.clone(), st.clone());
    click.connect_pressed(move |_, n, x, y| {
        if n == 2 && s.borrow().tool == Tool::Select {
            if crate::input::edit_table_cell_at(&u, &s, x, y) {
                return;
            }
            let sel = s.borrow().single();
            let has_text = sel.as_ref().is_some_and(|id| s.borrow().doc.text_of(id).is_some());
            if let (Some(id), true) = (sel, has_text) {
                edit_text_dialog(&u, &s, &id);
            }
        }
    });
    area.add_controller(click);

    let menu_click = gtk::GestureClick::new();
    menu_click.set_button(3);
    let (u, s) = (ui.clone(), st.clone());
    menu_click.connect_pressed(move |_, _, x, y| canvas_context_menu(&u, &s, x, y));
    area.add_controller(menu_click);
    area.add_controller(crate::snippets::canvas_drop_target(&ui, &st));

    let keys = gtk::EventControllerKey::new();
    let (u, s) = (ui.clone(), st.clone());
    keys.connect_key_pressed(move |_, key, _, state| key_pressed(&u, &s, key, state));
    let (u, s) = (ui.clone(), st.clone());
    keys.connect_key_released(move |_, key, _, _| key_released(&u, &s, key));
    window.add_controller(keys);

    let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    let (u, s) = (ui.clone(), st.clone());
    scroll.connect_scroll(move |c, _, dy| {
        if !c.current_event_state().contains(gdk::ModifierType::CONTROL_MASK) {
            return glib::Propagation::Proceed;
        }
        crate::actions::zoom(&u, &s, Some(-dy.signum()));
        glib::Propagation::Stop
    });
    area.add_controller(scroll);

    let (u, s) = (ui.clone(), st.clone());
    window.connect_close_request(move |_| {
        crate::viewstate::remember(&u, &s.borrow());
        glib::Propagation::Proceed
    });

    if let Some(f) = file {
        load_path(&ui, &st, f);
    }
    sync(&ui, &st);
    crate::remote::listen(&ui, &st);
    window.present();
    area.grab_focus();
}
