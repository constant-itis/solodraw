//! Preferences window: Text, Clipboard, Canvas, Interface (docs/UI_INVENTORY.md §5.3).
//! Changes apply to every open window immediately.

use solodraw_core::export::COMMENT_STYLES;

use crate::form::{self, Changed, Form};
use crate::prefs::{self as p};
use crate::prelude::*;

/// Redraw every window (font, antialiasing, page guide).
fn redraw_all() {
    for_each_window(|ui, st| {
        ui.area.set_font_options(Some(&p::font_options()));
        refresh(ui, &st.borrow());
    });
}

pub fn show(ui: &U) {
    let win = gtk::Window::builder().title("Preferences").transient_for(&ui.window).resizable(false).build();
    let stack = gtk::Stack::new();
    let switcher = gtk::StackSwitcher::new();
    switcher.set_stack(Some(&stack));
    switcher.set_halign(gtk::Align::Center);
    switcher.set_margin_top(8);
    let redraw: Changed = Rc::new(redraw_all);
    let nothing: Changed = Rc::new(|| {});
    let appearance: Changed = Rc::new(|| {
        p::apply_appearance();
        crate::window::apply_icon();
    });
    let assistant_anim: Changed = Rc::new(|| for_each_window(|ui, _| crate::assistant::set_tab_animation(&ui.assistant)));
    let apply_icon: Changed = Rc::new(crate::window::apply_icon);

    // Text.
    let f = Form::new();
    #[allow(deprecated)]
    let font = {
        let b = gtk::FontButton::with_font(&p::get_str(p::FONT_NAME, p::DEFAULT_FONT));
        b.set_filter_func(|family, _| family.is_monospace());
        b.set_use_font(true);
        b.connect_font_set(|b| {
            if let Some(name) = b.font() {
                p::set(p::FONT_NAME, json!(name.as_str()));
                redraw_all();
            }
        });
        b
    };
    f.row("Font:", &font);
    let note = gtk::Label::new(Some("The size comes from the zoom level."));
    note.add_css_class("dim-label");
    f.row("", &note);
    f.wide(&form::check("Anti-aliasing", p::ANTI_ALIAS, true, &redraw));
    f.wide(&form::check("Subpixel anti-aliasing", p::SUBPIXEL_ANTI_ALIAS, false, &redraw));
    f.row("Default alphabet:", &form::choice(&[(1, "Unicode"), (2, "ASCII")], p::ALPHABET, 1, &nothing));
    stack.add_titled(&f.grid, Some("text"), "Text");

    // Clipboard.
    let f = Form::new();
    f.row("Copy with comment style:", &form::choice(&COMMENT_STYLES, p::CLIP_COMMENT, 0, &nothing));
    f.row("Comment style padding:", &form::spin(0.0, 20.0, p::CLIP_PADDING, 1, &nothing));
    f.wide(&form::check("Trim trailing whitespace", p::CLIP_TRIM_TRAILING, true, &nothing));
    stack.add_titled(&f.grid, Some("clipboard"), "Clipboard");

    // Canvas.
    let f = Form::new();
    f.wide(&form::check("Show vertical lines for new documents", p::VGRID, false, &nothing));
    f.wide(&form::check("Show horizontal lines for new documents", p::HGRID, false, &nothing));
    let guide = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    guide.append(&form::check("Page guide at column", p::PAGE_GUIDE, false, &redraw));
    guide.append(&form::spin(1.0, 999.0, p::PAGE_GUIDE_COLUMN, 80, &redraw));
    f.wide(&guide);
    f.wide(&form::check("Show line numbers for new documents", p::LINE_NUMBERS, true, &nothing));
    f.wide(&form::check("Show column numbers for new documents", p::COLUMN_NUMBERS, true, &nothing));
    f.wide(&form::check("Apply selection shape style on create", p::APPLY_STYLE_ON_CREATE, false, &nothing));
    stack.add_titled(&f.grid, Some("canvas"), "Canvas");

    // Interface.
    let f = Form::new();
    f.row("Default snippet list:", &form::choice(&[(0, "All Snippets"), (1, "User Snippets"), (2, "Standard Snippets")], p::DEFAULT_SNIPPET_LIST, 0, &nothing));
    f.row("Appearance:", &form::choice(&[(0, "System"), (1, "Light"), (2, "Dark")], p::APPEARANCE, 0, &appearance));
    f.wide(&form::check("Use dark icon in dark mode", p::DARK_ICON, true, &apply_icon));
    f.wide(&form::check("Animate sidebar", p::SIDEBAR_ANIMATED, true, &nothing));
    f.wide(&form::check("Animate assistant", p::ASSISTANT_ANIMATED, true, &assistant_anim));
    f.wide(&form::check("Show characters tab when using the pencil", p::CHARS_FOR_PENCIL, false, &nothing));
    stack.add_titled(&f.grid, Some("interface"), "Interface");

    let vb = gtk::Box::new(gtk::Orientation::Vertical, 0);
    vb.append(&switcher);
    vb.append(&stack);
    win.set_child(Some(&vb));
    let keys = gtk::EventControllerKey::new();
    let w = win.clone();
    keys.connect_key_pressed(move |_, k, _, _| {
        if k == gdk::Key::Escape {
            w.close();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    win.add_controller(keys);
    win.present();
}
