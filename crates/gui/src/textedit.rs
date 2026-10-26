//! Text editing popover/dialog.

use crate::prelude::*;

pub fn edit_text_dialog(ui: &U, st: &St, shape: &str) {
    let Some(tid) = st.borrow().doc.text_of(shape) else { return };
    let current = st.borrow().doc.get(&tid).and_then(|o| o.str_("text")).unwrap_or("").to_string();
    text_dialog(ui, st, "Edit Text", &current, move |s, text| s.doc.set_value(&tid, "text", json!(text)));
}

/// Edit the text of one table cell (created on first text, removed when emptied).
pub fn edit_cell_dialog(ui: &U, st: &St, table: &str, row: usize, col: usize) {
    let current = st.borrow().doc.table_cell_text(table, row, col);
    let table = table.to_string();
    let title = format!("Edit Cell (row {}, column {})", row + 1, col + 1);
    text_dialog(ui, st, &title, &current, move |s, text| s.doc.set_table_cell_text(&table, row, col, text));
}

fn text_dialog(ui: &U, st: &St, title: &str, current: &str, apply: impl Fn(&mut App, &str) + 'static) {
    let win = gtk::Window::builder().transient_for(&ui.window).modal(true).title(title).default_width(420).default_height(220).build();
    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
    vbox.set_margin_top(10);
    vbox.set_margin_bottom(10);
    vbox.set_margin_start(10);
    vbox.set_margin_end(10);
    let tv = gtk::TextView::new();
    tv.set_monospace(true);
    tv.buffer().set_text(current);
    let sw = gtk::ScrolledWindow::new();
    sw.set_vexpand(true);
    sw.set_child(Some(&tv));
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    let ok = gtk::Button::with_label("Apply");
    ok.add_css_class("suggested-action");
    row.append(&cancel);
    row.append(&ok);
    vbox.append(&sw);
    vbox.append(&row);
    win.set_child(Some(&vbox));
    let w2 = win.clone();
    cancel.connect_clicked(move |_| w2.close());
    let (ui2, st2, w3) = (ui.clone(), st.clone(), win.clone());
    ok.connect_clicked(move |_| {
        let b = tv.buffer();
        let (a, z) = b.bounds();
        let text = b.text(&a, &z, false).to_string();
        {
            let mut s = st2.borrow_mut();
            s.checkpoint();
            apply(&mut s, &text);
            s.rerender();
        }
        refresh(&ui2, &st2.borrow());
        w3.close();
    });
    win.present();
}
