//! Window actions, keyboard accelerators and the menu bar (docs/UI_INVENTORY.md §2).

use gtk::gio;

use crate::prelude::*;

/// Menu bar layout. `None` entries are section breaks; `Some((label, action))`, where an empty
/// action means a disabled item that exists only so the menu matches Monodraw's. A label
/// `"Sub ▸ Item"` puts the item in submenu "Sub" of its section.
type Item = Option<(&'static str, &'static str)>;

const MENUS: &[(&str, &[Item])] = &[
    (
        "_File",
        &[
            Some(("New", "win.new")),
            Some(("Open…", "win.open")),
            Some(("Welcome to solodraw", "win.welcome")),
            None,
            Some(("Close", "win.close")),
            Some(("Save…", "win.save")),
            Some(("Save As…", "win.save-as")),
            Some(("Revert to Saved", "win.revert")),
            None,
            Some(("Export…", "win.export")),
            Some(("Import…", "win.import")),
            None,
            Some(("Preferences…", "win.preferences")),
            None,
            Some(("Quit", "app.quit")),
        ],
    ),
    (
        "_Edit",
        &[
            Some(("Undo", "win.undo")),
            Some(("Redo", "win.redo")),
            None,
            Some(("Cut", "win.cut")),
            Some(("Copy", "win.copy")),
            Some(("Paste", "win.paste")),
            Some(("Paste Raw", "win.paste-raw")),
            Some(("Duplicate", "win.duplicate")),
            Some(("Delete", "win.delete")),
            Some(("Select All", "win.select-all")),
            Some(("Deselect All", "win.deselect-all")),
            None,
            Some(("Add Snippet…", "win.add-snippet")),
            Some(("Assistant Search", "win.assistant-search")),
        ],
    ),
    (
        "_Shape",
        &[
            Some(("Move Forwards", "win.move-forward")),
            Some(("Move Backwards", "win.move-backward")),
            None,
            Some(("Move to Front", "win.move-to-front")),
            Some(("Move to Back", "win.move-to-back")),
            None,
            Some(("Group", "win.group")),
            Some(("Ungroup", "win.ungroup")),
            Some(("Flatten", "win.flatten")),
            None,
            Some(("Lock / Unlock", "win.lock")),
            Some(("Hide / Unhide", "win.hide")),
            None,
            Some(("Copy Style", "win.copy-style")),
            Some(("Paste Style", "win.paste-style")),
            None,
            Some(("Locate", "win.locate")),
            Some(("Edit", "win.edit-shape")),
            Some(("Reset Content", "win.reset-content")),
        ],
    ),
    (
        "_View",
        &[
            Some(("Zoom In", "win.zoom-in")),
            Some(("Zoom Out", "win.zoom-out")),
            Some(("Zoom to 100%", "win.zoom-100")),
            None,
            Some(("Sidebar", "win.show-sidebar")),
            Some(("Assistant", "win.show-assistant")),
            None,
            Some(("Shape Inspector", "win.tab-inspector")),
            Some(("Character Palette", "win.tab-characters")),
            Some(("Snippets", "win.tab-snippets")),
            None,
            Some(("Line Numbers", "win.line-numbers")),
            Some(("Column Numbers", "win.column-numbers")),
            None,
            Some(("Vertical Canvas Grid", "win.vgrid")),
            Some(("Horizontal Canvas Grid", "win.hgrid")),
            None,
            Some(("Movement Alignment Guides", "win.alignment-guides")),
            None,
            Some(("Show Toolbar", "win.show-toolbar")),
            Some(("Customize Toolbar…", "win.customize-toolbar")),
        ],
    ),
    (
        "_Window",
        &[
            Some(("Welcome to solodraw", "win.welcome")),
            Some(("Character Picker", "win.character-picker")),
            None,
            Some(("Developer ▸ Table Shape", "win.table-shape")),
        ],
    ),
    (
        "E_xtras",
        &[
            Some(("Reveal Fonts Folder…", "win.reveal-fonts")),
            Some(("Reveal Characters Folder…", "win.reveal-chars")),
            Some(("Reveal User Snippets Folder…", "win.reveal-snippets")),
            None,
            Some(("Command Line Tool…", "win.cli-tool")),
        ],
    ),
    ("_Help", &[Some(("solodraw Help", "win.help")), Some(("About solodraw", "win.about"))]),
];

/// Linux accelerators (Cmd → Ctrl; † remaps in UI_INVENTORY.md).
const ACCELS: &[(&str, &[&str])] = &[
    ("win.new", &["<Control>n"]),
    ("win.open", &["<Control>o"]),
    ("win.welcome", &["<Control><Shift>1", "<Control>exclam"]),
    ("win.close", &["<Control>w"]),
    ("win.save", &["<Control>s"]),
    ("win.save-as", &["<Control><Shift>s"]),
    ("win.export", &["<Control>e"]),
    ("win.import", &["<Control>i"]),
    ("win.preferences", &["<Control>comma"]),
    ("app.quit", &["<Control>q"]),
    ("win.undo", &["<Control>z"]),
    ("win.redo", &["<Control><Shift>z", "<Control>y"]),
    ("win.cut", &["<Control>x"]),
    ("win.copy", &["<Control>c"]),
    ("win.paste", &["<Control>v"]),
    ("win.paste-raw", &["<Control><Shift>v"]),
    ("win.duplicate", &["<Control>d"]),
    ("win.select-all", &["<Control>a"]),
    ("win.deselect-all", &["<Control><Shift>a"]),
    ("win.assistant-search", &["<Control>k"]),
    ("win.move-forward", &["<Control><Shift>f"]),
    ("win.move-backward", &["<Control><Shift>b"]),
    ("win.move-to-front", &["<Control><Alt>f"]),
    ("win.move-to-back", &["<Control><Alt>b"]),
    ("win.group", &["<Control>g"]),
    ("win.ungroup", &["<Control><Shift>g"]),
    ("win.flatten", &["<Control><Shift>backslash", "<Control>bar"]),
    ("win.copy-style", &["<Control><Alt>c"]),
    ("win.paste-style", &["<Control><Alt>v"]),
    ("win.lock", &["<Control>l"]),
    ("win.hide", &["<Control><Alt>h"]),
    ("win.edit-shape", &["<Control><Shift>e"]),
    ("win.zoom-in", &["<Control>plus", "<Control>equal", "<Control>KP_Add"]),
    ("win.zoom-out", &["<Control>minus", "<Control>KP_Subtract"]),
    ("win.zoom-100", &["<Control>0"]),
    ("win.show-sidebar", &["<Control><Shift>bracketleft", "<Control>braceleft", "F9"]),
    ("win.show-assistant", &["<Control><Shift>bracketright", "<Control>braceright"]),
    ("win.tab-inspector", &["<Control>1"]),
    ("win.tab-characters", &["<Control>2"]),
    ("win.tab-snippets", &["<Control>3"]),
    ("win.character-picker", &["<Control><Shift>2", "<Control>at"]),
    ("win.help", &["F1"]),
];

pub fn menu_model() -> gio::Menu {
    let bar = gio::Menu::new();
    for (title, items) in MENUS {
        let menu = gio::Menu::new();
        let mut section = gio::Menu::new();
        let mut subs: Vec<(&str, gio::Menu)> = Vec::new();
        for it in *items {
            match it {
                None => {
                    menu.append_section(None, &section);
                    section = gio::Menu::new();
                    subs.clear();
                }
                Some((label, action)) => match label.split_once(" ▸ ") {
                    Some((sub, item)) => {
                        if !subs.iter().any(|(s, _)| *s == sub) {
                            let m = gio::Menu::new();
                            section.append_submenu(Some(sub), &m);
                            subs.push((sub, m));
                        }
                        if let Some((_, m)) = subs.iter().find(|(s, _)| *s == sub) {
                            m.append(Some(item), Some(action));
                        }
                    }
                    None => section.append(Some(label), Some(action)),
                },
            }
        }
        menu.append_section(None, &section);
        bar.append_submenu(Some(title), &menu);
    }
    bar
}

pub fn set_accels(app: &gtk::Application) {
    for (action, keys) in ACCELS {
        app.set_accels_for_action(action, keys);
    }
}

fn add(ui: &U, st: &St, name: &str, f: impl Fn(&U, &St) + 'static) -> gio::SimpleAction {
    let a = gio::SimpleAction::new(name, None);
    let (u, s) = (ui.clone(), st.clone());
    a.connect_activate(move |_, _| f(&u, &s));
    ui.window.add_action(&a);
    a
}

fn add_toggle(ui: &U, st: &St, name: &str, init: bool, f: impl Fn(&U, &St, bool) + 'static) {
    let a = gio::SimpleAction::new_stateful(name, None, &init.to_variant());
    let (u, s) = (ui.clone(), st.clone());
    a.connect_change_state(move |a, v| {
        let Some(v) = v.and_then(|v| v.get::<bool>()) else { return };
        a.set_state(&v.to_variant());
        f(&u, &s, v);
    });
    ui.window.add_action(&a);
}

/// Actions that exist (so menus match Monodraw) but are not implemented yet.
pub fn install(ui: &U, st: &St) {
    add(ui, st, "help", |ui, _| help_window(ui));
    add(ui, st, "new", |ui, _| {
        if let Some(app) = ui.window.application() {
            build(&app, None);
        }
    });
    add(ui, st, "open", open_dialog);
    add(ui, st, "welcome", |ui, _| {
        if let Some(app) = ui.window.application() {
            crate::welcome::show(&app);
        }
    });
    add(ui, st, "close", |ui, _| ui.window.close());
    add(ui, st, "save", |ui, st| save(ui, st, false));
    add(ui, st, "save-as", |ui, st| save(ui, st, true));
    add(ui, st, "export", crate::exportwin::show);
    add(ui, st, "edit-shape", toggle_edit_mode);
    add(ui, st, "add-midpoint", add_midpoint);
    add(ui, st, "delete-midpoint", delete_midpoint);
    add(ui, st, "add-label", add_label);
    add_toggle(ui, st, "alignment-guides", crate::prefs::get_bool(crate::prefs::GUIDES, false), |_, _, v| {
        crate::prefs::set(crate::prefs::GUIDES, json!(v))
    });
    add(ui, st, "import", import_dialog);
    add(ui, st, "revert", revert);
    add(ui, st, "preferences", |ui, _| crate::prefswin::show(ui));

    add(ui, st, "undo", |ui, st| {
        st.borrow_mut().undo();
        sync(ui, st);
    });
    add(ui, st, "redo", |ui, st| {
        st.borrow_mut().redo();
        sync(ui, st);
    });
    add(ui, st, "copy", crate::clipboard::copy);
    add(ui, st, "cut", crate::clipboard::cut);
    add(ui, st, "paste", crate::clipboard::paste);
    // Context-menu Paste: centred on the right-clicked point.
    add(ui, st, "paste-here", |ui, st| {
        let p = st.borrow().context_point;
        st.borrow_mut().paste_at = p;
        crate::clipboard::paste(ui, st);
    });
    add(ui, st, "paste-raw", crate::clipboard::paste_raw);
    add(ui, st, "duplicate", crate::clipboard::duplicate);
    add(ui, st, "group", crate::clipboard::group);
    add(ui, st, "ungroup", crate::clipboard::ungroup);
    add(ui, st, "flatten", crate::clipboard::flatten);
    add(ui, st, "copy-style", crate::clipboard::copy_style);
    add(ui, st, "paste-style", crate::clipboard::paste_style);
    add(ui, st, "reset-content", crate::clipboard::reset_content);
    add(ui, st, "add-snippet", crate::snippets::add_snippet_dialog);
    // Focus the search field of the Characters tab when it shows, else the Snippets tab's.
    add(ui, st, "assistant-search", |ui, _| {
        let chars = ui.assistant_pane.is_visible() && ui.assistant.visible_child_name().as_deref() == Some("characters");
        ui.assistant_pane.set_visible(true);
        if chars {
            ui.char_search.grab_focus();
        } else {
            ui.assistant.set_visible_child_name("snippets");
            ui.snippet_search.grab_focus();
        }
    });
    add(ui, st, "character-picker", crate::assistant::character_picker);
    add(ui, st, "delete", delete_selection);
    add(ui, st, "select-all", |ui, st| {
        if crate::clipboard::forward_to_text(ui, "selection.select-all") {
            return;
        }
        {
            let mut s = st.borrow_mut();
            let kids: Vec<String> = s.doc.get(&s.canvas).map(|c| c.refs("subshape_refs").into_iter().map(str::to_string).collect()).unwrap_or_default();
            let sel = kids.into_iter().rev().filter(|k| !s.flag(k, "locked") && !s.flag(k, "hidden")).collect();
            s.selection = sel;
        }
        sync(ui, st);
    });
    add(ui, st, "deselect-all", |ui, st| {
        st.borrow_mut().selection.clear();
        sync(ui, st);
    });

    add(ui, st, "move-forward", |ui, st| reorder(ui, st, Reorder::Forward));
    add(ui, st, "move-backward", |ui, st| reorder(ui, st, Reorder::Backward));
    add(ui, st, "move-to-front", |ui, st| reorder(ui, st, Reorder::Front));
    add(ui, st, "move-to-back", |ui, st| reorder(ui, st, Reorder::Back));
    add(ui, st, "lock", |ui, st| toggle_selected(ui, st, "locked"));
    add(ui, st, "hide", |ui, st| toggle_selected(ui, st, "hidden"));
    add(ui, st, "locate", |ui, st| {
        if let Some(id) = st.borrow().primary() {
            locate(ui, st, &id);
        }
    });

    add(ui, st, "zoom-in", |ui, st| zoom(ui, st, Some(1.0)));
    add(ui, st, "zoom-out", |ui, st| zoom(ui, st, Some(-1.0)));
    add(ui, st, "zoom-100", |ui, st| zoom(ui, st, None));
    add_toggle(ui, st, "show-sidebar", true, |ui, _, v| ui.sidebar_pane.set_visible(v));
    add_toggle(ui, st, "show-assistant", true, |ui, _, v| ui.assistant_pane.set_visible(v));
    // Window ▸ Developer ▸ Table Shape: opt-in Table tool.
    add_toggle(ui, st, "table-shape", crate::prefs::get_bool(crate::prefs::TABLE_SHAPE, false), |ui, st, v| {
        crate::prefs::set(crate::prefs::TABLE_SHAPE, json!(v));
        crate::toolbar::apply_all();
        if !v && st.borrow().tool == Tool::Table {
            crate::input::set_tool(ui, st, Tool::Select);
        }
        sync(ui, st);
    });
    add(ui, st, "table-edit-cell", |ui, st| {
        let hit = {
            let s = st.borrow();
            let id = s.single();
            id.zip(s.context_point).and_then(|(id, p)| s.scene().table_grid(&id)?.cell_at(p).map(|(r, c)| (id, r, c)))
        };
        if let Some((id, r, c)) = hit {
            crate::textedit::edit_cell_dialog(ui, st, &id, r, c);
        }
    });
    add(ui, st, "table-row-above", |ui, st| crate::input::table_track(ui, st, true, Some(0)));
    add(ui, st, "table-row-below", |ui, st| crate::input::table_track(ui, st, true, Some(1)));
    add(ui, st, "table-col-left", |ui, st| crate::input::table_track(ui, st, false, Some(0)));
    add(ui, st, "table-col-right", |ui, st| crate::input::table_track(ui, st, false, Some(1)));
    add(ui, st, "table-delete-row", |ui, st| crate::input::table_track(ui, st, true, None));
    add(ui, st, "table-delete-col", |ui, st| crate::input::table_track(ui, st, false, None));
    add_toggle(ui, st, "show-toolbar", crate::prefs::get_bool(crate::toolbar::VISIBLE, true), |_, _, v| crate::toolbar::set_visible(v));
    add(ui, st, "customize-toolbar", |ui, _| crate::toolbar::customize_window(ui));
    add(ui, st, "insert-image", crate::image::insert_dialog);
    for (name, page) in [("tab-inspector", "inspector"), ("tab-characters", "characters"), ("tab-snippets", "snippets")] {
        add(ui, st, name, move |ui, _| {
            ui.assistant_pane.set_visible(true);
            ui.assistant.set_visible_child_name(page);
        });
    }
    let s = st.borrow();
    let (ln, cn, vg, hg) = (s.show_line_numbers, s.show_column_numbers, s.show_vgrid, s.show_hgrid);
    drop(s);
    add_toggle(ui, st, "line-numbers", ln, |ui, st, v| {
        st.borrow_mut().show_line_numbers = v;
        refresh(ui, &st.borrow());
    });
    add_toggle(ui, st, "column-numbers", cn, |ui, st, v| {
        st.borrow_mut().show_column_numbers = v;
        refresh(ui, &st.borrow());
    });
    add_toggle(ui, st, "vgrid", vg, |ui, st, v| {
        st.borrow_mut().show_vgrid = v;
        refresh(ui, &st.borrow());
    });
    add_toggle(ui, st, "hgrid", hg, |ui, st, v| {
        st.borrow_mut().show_hgrid = v;
        refresh(ui, &st.borrow());
    });

    for (name, sub) in [("reveal-fonts", "fonts"), ("reveal-chars", "char-palettes"), ("reveal-snippets", "snippets")] {
        add(ui, st, name, move |ui, _| reveal_data_dir(ui, sub));
    }
    add(ui, st, "cli-tool", cli_tool_dialog);
    add(ui, st, "about", |ui, _| {
        let d = gtk::AboutDialog::builder()
            .transient_for(&ui.window)
            .modal(true)
            .program_name("solodraw")
            .version(env!("CARGO_PKG_VERSION"))
            .comments("Plain-text diagram editor, compatible with Monodraw .monopic files.")
            .license_type(gtk::License::MitX11)
            .build();
        d.present();
    });
}

pub fn delete_selection(ui: &U, st: &St) {
    {
        let mut s = st.borrow_mut();
        let roots = s.selection_roots();
        if roots.is_empty() {
            return;
        }
        s.selection.clear();
        s.checkpoint();
        let canvas = s.canvas.clone();
        for sel in roots {
            s.doc.delete_shape(&canvas, &sel);
        }
        s.rerender();
    }
    sync(ui, st);
}

fn toggle_selected(ui: &U, st: &St, key: &str) {
    let ids = st.borrow().selection_roots();
    let Some(first) = ids.first() else { return };
    // One undo step; every shape takes the first one's new state.
    let v = !st.borrow().flag(first, key);
    {
        let mut s = st.borrow_mut();
        s.checkpoint();
        for id in &ids {
            s.doc.set_value(id, key, json!(v));
        }
        if v {
            s.selection.retain(|x| !ids.contains(x));
        }
        s.rerender();
    }
    sync(ui, st);
}

#[derive(Clone, Copy)]
pub enum Reorder {
    Forward,
    Backward,
    Front,
    Back,
}

/// Move the selection within its parent's z-order (index 0 = top).
pub fn reorder(ui: &U, st: &St, how: Reorder) {
    {
        let mut s = st.borrow_mut();
        let mut ids = s.selection_roots();
        if ids.is_empty() {
            return;
        }
        // Process in an order that keeps the selected shapes' relative stacking.
        let index = |s: &App, id: &str| {
            let p = s.scene().parent(id).map(str::to_string).unwrap_or_default();
            s.doc.get(&p).and_then(|o| o.refs("subshape_refs").iter().position(|x| *x == id)).unwrap_or(0)
        };
        ids.sort_by_key(|id| index(&s, id));
        if matches!(how, Reorder::Backward | Reorder::Front) {
            ids.reverse();
        }
        s.checkpoint();
        for id in ids {
            let Some(parent) = s.scene().parent(&id).map(str::to_string) else { continue };
            let n = s.doc.get(&parent).map(|p| p.refs("subshape_refs").len()).unwrap_or(0);
            let Some(i) = s.doc.detach_child(&parent, &id) else { continue };
            let j = match how {
                Reorder::Forward => i.saturating_sub(1),
                Reorder::Backward => (i + 1).min(n - 1),
                Reorder::Front => 0,
                Reorder::Back => n - 1,
            };
            s.doc.attach_child(&parent, &id, j);
        }
        s.rerender();
    }
    sync(ui, st);
}

pub fn zoom(ui: &U, st: &St, step: Option<f64>) {
    {
        let mut s = st.borrow_mut();
        s.font_size = match step {
            Some(d) => (s.font_size + d).clamp(6.0, 40.0),
            None => BASE_FONT_SIZE,
        };
    }
    refresh(ui, &st.borrow());
}

/// Scroll the canvas so shape `id` is centred.
pub fn locate(ui: &U, st: &St, id: &str) {
    let s = st.borrow();
    let Some(f) = s.scene().frame(id) else { return };
    let (cw, ch) = cell_size(&ui.area, &s);
    let cx = (f.x + s.view_offset.x) as f64 * cw + f.w as f64 * cw / 2.0;
    let cy = (f.y + s.view_offset.y) as f64 * ch + f.h as f64 * ch / 2.0;
    let (h, v) = (ui.scroller.hadjustment(), ui.scroller.vadjustment());
    h.set_value(cx - h.page_size() / 2.0);
    v.set_value(cy - v.page_size() / 2.0);
}

pub fn data_dir(sub: &str) -> PathBuf {
    glib::user_data_dir().join("solodraw").join(sub)
}

fn reveal_data_dir(ui: &U, sub: &str) {
    let dir = data_dir(sub);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        error(ui, &format!("Could not create {}:\n{e}", dir.display()));
        return;
    }
    let uri = gio::File::for_path(&dir).uri();
    if let Err(e) = gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>) {
        error(ui, &format!("Could not open {}:\n{e}", dir.display()));
    }
}

fn cli_tool_dialog(ui: &U, _: &St) {
    let path = glib::home_dir().join(".local/bin/solodraw");
    let state = if path.exists() { "is installed" } else { "is not installed" };
    let d = gtk::MessageDialog::builder()
        .transient_for(&ui.window)
        .modal(true)
        .message_type(gtk::MessageType::Info)
        .buttons(gtk::ButtonsType::Close)
        .text("Command Line Tool")
        .secondary_text(format!(
            "solodraw includes a command line tool, solodraw, for rendering and converting monopic files from a terminal.\n\nTool path: {} ({state}).\nRun `monodraw --help` for usage.",
            path.display()
        ))
        .build();
    d.connect_response(|d, _| d.close());
    d.present();
}

/// File ▸ Import: a text file becomes a Surface at the top-left of the view.
fn import_dialog(ui: &U, st: &St) {
    let d = gtk::FileChooserNative::new(Some("Import Text"), Some(&ui.window), gtk::FileChooserAction::Open, Some("_Import"), Some("_Cancel"));
    let f = gtk::FileFilter::new();
    f.set_name(Some("Text files"));
    f.add_mime_type("text/plain");
    f.add_pattern("*.txt");
    d.add_filter(&f);
    let (ui2, st2) = (ui.clone(), st.clone());
    d.connect_response(move |d, resp| {
        if resp == gtk::ResponseType::Accept {
            if let Some(p) = d.file().and_then(|f| f.path()) {
                match std::fs::read(&p) {
                    Ok(b) => crate::clipboard::paste_text(&ui2, &st2, &String::from_utf8_lossy(&b)),
                    Err(e) => error(&ui2, &format!("Could not import {}:\n{e}", p.display())),
                }
            }
        }
        ui2.dialog.replace(None);
    });
    d.show();
    ui.dialog.replace(Some(d));
}

/// File ▸ Revert to Saved, after confirming when there are unsaved changes.
fn revert(ui: &U, st: &St) {
    let (path, dirty) = {
        let s = st.borrow();
        (s.path.clone(), s.dirty)
    };
    let Some(path) = path else {
        ui.status.set_text("This document has not been saved yet");
        return;
    };
    if !dirty {
        return;
    }
    let d = gtk::MessageDialog::builder()
        .transient_for(&ui.window)
        .modal(true)
        .message_type(gtk::MessageType::Warning)
        .text("Revert to the saved version?")
        .secondary_text("Your current changes will be lost.")
        .build();
    d.add_button("Cancel", gtk::ResponseType::Cancel);
    d.add_button("Revert", gtk::ResponseType::Accept);
    let (ui2, st2) = (ui.clone(), st.clone());
    d.connect_response(move |d, r| {
        d.close();
        if r == gtk::ResponseType::Accept {
            load_path(&ui2, &st2, path.clone());
        }
    });
    d.present();
}

/// Help ▸ Monodraw Help: tools and keyboard shortcuts, generated from the toolbar and menu tables.
fn help_window(ui: &U) {
    let win = gtk::Window::builder().title("solodraw Help").transient_for(&ui.window).default_width(560).default_height(640).build();
    let vb = gtk::Box::new(gtk::Orientation::Vertical, 4);
    vb.set_margin_start(16);
    vb.set_margin_end(16);
    vb.set_margin_top(12);
    vb.set_margin_bottom(12);
    let heading = |t: &str| {
        let l = gtk::Label::new(Some(t));
        l.set_xalign(0.0);
        l.add_css_class("md-section-title");
        l.set_margin_top(10);
        l
    };
    let line = |a: &str, b: &str| {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let l = gtk::Label::new(Some(a));
        l.set_xalign(0.0);
        l.set_hexpand(true);
        let k = gtk::Label::new(Some(b));
        k.add_css_class("dim-label");
        row.append(&l);
        row.append(&k);
        row
    };
    vb.append(&heading("Tools"));
    for (tool, label, _, key, _, _) in Tool::ALL {
        if !tool.available() {
            continue;
        }
        let key = if key == ' ' { "Space".to_string() } else { key.to_ascii_uppercase().to_string() };
        vb.append(&line(&format!("{label} — {}", tool.hint().split(": ").nth(1).unwrap_or_default()), &key));
    }
    vb.append(&line("Keep a tool after one shape", "Shift+key or Shift+click"));
    vb.append(&line("Back to Select", "Esc"));
    for (title, items) in MENUS {
        let mut rows = Vec::new();
        for (label, action) in items.iter().flatten() {
            let Some((_, keys)) = ACCELS.iter().find(|(a, _)| a == action) else { continue };
            // Skip the shifted-keysym aliases (`<Control>bar` for Ctrl+Shift+\\ and so on).
            let names: Vec<String> = keys
                .iter()
                .filter(|k| !["bar", "at", "exclam", "braceleft", "braceright"].iter().any(|a| k.ends_with(&format!(">{a}"))))
                .filter_map(|k| gtk::accelerator_parse(*k).map(|(key, m)| gtk::accelerator_get_label(key, m).to_string()))
                .take(2)
                .collect();
            rows.push(line(label, &names.join("  or  ")));
        }
        if !rows.is_empty() {
            vb.append(&heading(&title.replace('_', "")));
            for r in rows {
                vb.append(&r);
            }
        }
    }
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_child(Some(&vb));
    scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    win.set_child(Some(&scroll));
    win.present();
}
