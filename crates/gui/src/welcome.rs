//! Welcome window: New / Open / recent documents (docs/UI_INVENTORY.md §5.1).

use gtk::gio;

use crate::icons::icon;
use crate::prelude::*;

fn recent_monopics() -> Vec<PathBuf> {
    let mut items: Vec<_> = gtk::RecentManager::default()
        .items()
        .into_iter()
        .filter_map(|i| {
            let path = gio::File::for_uri(&i.uri()).path()?;
            let ok = path.extension().is_some_and(|e| e == "monopic" || e == "monojson") && path.exists();
            ok.then(|| (i.modified().to_unix(), path))
        })
        .collect();
    items.sort_by(|a, b| b.0.cmp(&a.0));
    items.into_iter().map(|(_, p)| p).take(20).collect()
}

fn tilde(p: &std::path::Path) -> String {
    let home = glib::home_dir();
    match p.parent().and_then(|d| d.strip_prefix(&home).ok()) {
        Some(rel) => format!("~/{}", rel.display()),
        None => p.parent().map(|d| d.display().to_string()).unwrap_or_default(),
    }
}

pub fn show(app: &gtk::Application) {
    if let Some(w) = app.windows().into_iter().find(|w| w.widget_name() == "welcome") {
        w.present();
        return;
    }
    let win = gtk::Window::builder().application(app).title("Welcome to solodraw").default_width(720).default_height(440).resizable(false).build();
    win.set_widget_name("welcome");

    let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);

    // Left: identity + actions.
    let left = gtk::Box::new(gtk::Orientation::Vertical, 10);
    left.set_width_request(380);
    left.set_valign(gtk::Align::Center);
    left.set_margin_start(32);
    left.set_margin_end(32);
    let logo = icon("md-rect", 72);
    logo.set_halign(gtk::Align::Center);
    left.append(&logo);
    let title = gtk::Label::new(None);
    title.set_markup("<span size='xx-large' weight='bold'>solodraw</span>");
    left.append(&title);
    let ver = gtk::Label::new(Some(&format!("Version {} for Linux", env!("CARGO_PKG_VERSION"))));
    ver.add_css_class("dim-label");
    ver.set_margin_bottom(18);
    left.append(&ver);

    let big = |label: &str, sub: &str, ic: &'static str| {
        let b = gtk::Button::new();
        let hb = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        hb.append(&icon(ic, 28));
        let vb = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let l = gtk::Label::new(None);
        l.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(label)));
        l.set_halign(gtk::Align::Start);
        let s = gtk::Label::new(Some(sub));
        s.add_css_class("dim-label");
        s.set_halign(gtk::Align::Start);
        vb.append(&l);
        vb.append(&s);
        hb.append(&vb);
        b.set_child(Some(&hb));
        b.add_css_class("flat");
        b
    };
    let new_b = big("Create a new document", "Start with an empty canvas", "md-edit");
    let open_b = big("Open a document", "Open an existing .monopic file", "md-export");
    left.append(&new_b);
    left.append(&open_b);

    let show_at_launch = gtk::CheckButton::with_label("Show this window when solodraw launches");
    show_at_launch.set_active(crate::prefs::get_bool(crate::prefs::WELCOME_AT_LAUNCH, true));
    show_at_launch.set_margin_top(24);
    show_at_launch.connect_toggled(|c| crate::prefs::set(crate::prefs::WELCOME_AT_LAUNCH, json!(c.is_active())));
    left.append(&show_at_launch);
    root.append(&left);

    // Right: recent documents.
    let list = gtk::ListBox::new();
    list.add_css_class("navigation-sidebar");
    let recent = recent_monopics();
    if recent.is_empty() {
        let l = gtk::Label::new(Some("No Recent Documents"));
        l.add_css_class("dim-label");
        l.set_vexpand(true);
        list.set_placeholder(Some(&l));
    }
    for p in &recent {
        let row = gtk::ListBoxRow::new();
        row.set_widget_name(&p.to_string_lossy());
        let vb = gtk::Box::new(gtk::Orientation::Vertical, 2);
        vb.set_margin_top(6);
        vb.set_margin_bottom(6);
        vb.set_margin_start(8);
        let n = gtk::Label::new(Some(&p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()));
        n.set_halign(gtk::Align::Start);
        let d = gtk::Label::new(Some(&tilde(p)));
        d.add_css_class("dim-label");
        d.set_halign(gtk::Align::Start);
        d.set_ellipsize(pango::EllipsizeMode::Middle);
        vb.append(&n);
        vb.append(&d);
        row.set_child(Some(&vb));
        list.append(&row);
    }
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_child(Some(&list));
    scroll.set_hexpand(true);
    scroll.add_css_class("md-welcome-recent");
    root.append(&scroll);
    win.set_child(Some(&root));

    let (a, w) = (app.clone(), win.clone());
    list.connect_row_activated(move |_, row| {
        build(&a, Some(PathBuf::from(row.widget_name().as_str())));
        w.close();
    });
    let (a, w) = (app.clone(), win.clone());
    new_b.connect_clicked(move |_| {
        build(&a, None);
        w.close();
    });
    let (a, w) = (app.clone(), win.clone());
    open_b.connect_clicked(move |_| {
        let d = gtk::FileChooserNative::new(Some("Open"), Some(&w), gtk::FileChooserAction::Open, Some("_Open"), Some("_Cancel"));
        d.add_filter(&file_filter());
        let (a2, w2) = (a.clone(), w.clone());
        d.connect_response(move |d, resp| {
            if resp == gtk::ResponseType::Accept {
                if let Some(p) = d.file().and_then(|f| f.path()) {
                    build(&a2, Some(p));
                    w2.close();
                }
            }
        });
        d.show();
        // Keep the dialog alive with the window.
        unsafe { w.set_data("open-dialog", d) };
    });
    win.present();
}
