//! solodraw: GTK4 editor.

#![allow(deprecated)] // FileChooserNative / Dialog-era APIs are the GTK 4.6 way.

mod actions;
mod assistant;
mod charnames;
mod canvas;
mod clipboard;
mod exportwin;
mod files;
mod form;
mod icons;
mod image;
mod input;
mod inspector;
mod prefs;
mod prefswin;
mod prelude;
mod remote;
mod sidebar;
mod snippets;
mod state;
mod textedit;
mod toolbar;
mod ui;
mod viewstate;
mod welcome;
mod window;

use prelude::*;

fn main() -> glib::ExitCode {
    let file = std::env::args().nth(1).map(PathBuf::from);
    let app = gtk::Application::builder().application_id("io.github.constant_itis.solodraw").build();
    app.connect_startup(|_| {
        prefs::apply_appearance();
        window::install_icons();
        window::apply_icon();
        if let Some(s) = gtk::Settings::default() {
            s.connect_gtk_application_prefer_dark_theme_notify(|_| window::apply_icon());
        }
    });
    app.connect_activate(move |gapp| match &file {
        Some(f) => build(gapp, Some(f.clone())),
        None if prefs::get_bool(prefs::WELCOME_AT_LAUNCH, true) => {
            // Menus/accels are installed by the first document window; set them up now too.
            welcome::show(gapp);
        }
        None => build(gapp, None),
    });
    app.run_with_args(&[std::env::args().next().unwrap_or_default()])
}
