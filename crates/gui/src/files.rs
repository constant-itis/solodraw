//! Open, save and error dialogs.

use crate::prelude::*;

pub fn load_path(ui: &U, st: &St, path: PathBuf) {
    match Document::open(&path) {
        Ok(doc) => {
            gtk::RecentManager::default().add_item(&gtk::gio::File::for_path(&path).uri());
            crate::viewstate::remember(ui, &st.borrow());
            *st.borrow_mut() = App::new(doc, Some(path));
            sync(ui, st);
            refresh(ui, &st.borrow());
            crate::viewstate::restore(ui, st);
        }
        Err(e) => error(ui, &format!("Could not open {}:\n{e}", path.display())),
    }
}

pub fn error(ui: &U, msg: &str) {
    let d = gtk::MessageDialog::builder()
        .transient_for(&ui.window)
        .modal(true)
        .message_type(gtk::MessageType::Error)
        .buttons(gtk::ButtonsType::Close)
        .text(msg)
        .build();
    d.connect_response(|d, _| d.close());
    d.show();
}

pub fn file_filter() -> gtk::FileFilter {
    let f = gtk::FileFilter::new();
    f.set_name(Some("Monodraw documents (.monopic)"));
    f.add_pattern("*.monopic");
    f.add_pattern("*.monojson");
    f
}

pub fn open_dialog(ui: &U, st: &St) {
    let d = gtk::FileChooserNative::new(Some("Open"), Some(&ui.window), gtk::FileChooserAction::Open, Some("_Open"), Some("_Cancel"));
    d.add_filter(&file_filter());
    let (ui2, st2) = (ui.clone(), st.clone());
    d.connect_response(move |d, resp| {
        if resp == gtk::ResponseType::Accept {
            if let Some(p) = d.file().and_then(|f| f.path()) {
                load_path(&ui2, &st2, p);
            }
        }
        ui2.dialog.replace(None);
    });
    d.show();
    ui.dialog.replace(Some(d));
}

pub fn save(ui: &U, st: &St, save_as: bool) {
    let path = st.borrow().path.clone();
    match path {
        Some(p) if !save_as => {
            let r = st.borrow().doc.save(&p);
            match r {
                Ok(()) => {
                    st.borrow_mut().dirty = false;
                    refresh(ui, &st.borrow());
                }
                Err(e) => error(ui, &format!("Could not save:\n{e}")),
            }
        }
        _ => {
            let d = gtk::FileChooserNative::new(Some("Save"), Some(&ui.window), gtk::FileChooserAction::Save, Some("_Save"), Some("_Cancel"));
            d.add_filter(&file_filter());
            d.set_current_name("Untitled.monopic");
            let (ui2, st2) = (ui.clone(), st.clone());
            d.connect_response(move |d, resp| {
                if resp == gtk::ResponseType::Accept {
                    if let Some(mut p) = d.file().and_then(|f| f.path()) {
                        if p.extension().is_none() {
                            p.set_extension("monopic");
                        }
                        st2.borrow_mut().path = Some(p);
                        save(&ui2, &st2, false);
                    }
                }
                ui2.dialog.replace(None);
            });
            d.show();
            ui.dialog.replace(Some(d));
        }
    }
}

pub fn copy_text(ui: &U, st: &St) {
    let text = crate::exportwin::clipboard_text(&st.borrow());
    ui.window.clipboard().set_text(&text);
    ui.status.set_text(&format!("Copied {} lines as text", text.lines().count()));
}
