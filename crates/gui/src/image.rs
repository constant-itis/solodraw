//! Toolbar ▸ Image: insert an image overlay for tracing. The image keeps
//! its own pixels: one image pixel per screen point at 100% zoom, rounded to whole cells, at 50%
//! opacity, centred in the visible canvas and selected. It draws no characters and is not exported.

use crate::prelude::*;

pub fn insert_dialog(ui: &U, st: &St) {
    let d = gtk::FileChooserNative::new(Some("Insert Image"), Some(&ui.window), gtk::FileChooserAction::Open, Some("_Insert"), Some("_Cancel"));
    let f = gtk::FileFilter::new();
    f.set_name(Some("Images"));
    f.add_pixbuf_formats();
    d.add_filter(&f);
    let (ui2, st2) = (ui.clone(), st.clone());
    d.connect_response(move |d, resp| {
        if resp == gtk::ResponseType::Accept {
            if let Some(p) = d.file().and_then(|f| f.path()) {
                insert_file(&ui2, &st2, &p);
            }
        }
        ui2.dialog.replace(None);
    });
    d.show();
    ui.dialog.replace(Some(d));
}

pub fn insert_file(ui: &U, st: &St, path: &std::path::Path) {
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) => return error(ui, &format!("Could not open {}:\n{e}", path.display())),
    };
    let stream = gio::MemoryInputStream::from_bytes(&glib::Bytes::from(&data));
    let Ok(pb) = gdk_pixbuf::Pixbuf::from_stream(&stream, gio::Cancellable::NONE) else {
        return error(ui, &format!("Could not open {}:\nnot an image this system can read", path.display()));
    };
    {
        let mut s = st.borrow_mut();
        // Cell size at 100% zoom.
        let (cw, ch) = cell_size(&ui.area, &s);
        let k = BASE_FONT_SIZE / s.font_size;
        let w = (pb.width() as f64 / (cw * k)).round() as i32;
        let h = (pb.height() as f64 / (ch * k)).round() as i32;
        if w < 1 || h < 1 {
            drop(s);
            return error(ui, "The image is too small to insert.");
        }
        let c = crate::clipboard::visible_centre(ui, &s);
        let canvas = s.canvas.clone();
        s.checkpoint();
        let id = s.doc.add_image(&canvas, &data, Rect::new(c.x - w / 2, c.y - h / 2, w, h), 0.5);
        s.select_only(Some(id));
        s.rerender();
    }
    sync(ui, st);
}
