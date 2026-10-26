//! Shared UI handles and the status/title refresh.

use std::cell::Cell;

use crate::prelude::*;

pub struct Ui {
    pub window: gtk::ApplicationWindow,
    pub area: gtk::DrawingArea,
    pub scroller: gtk::ScrolledWindow,
    pub ruler_top: gtk::DrawingArea,
    pub ruler_left: gtk::DrawingArea,
    pub inspector: gtk::Box,
    pub assistant: gtk::Stack,
    pub sidebar: gtk::ListBox,
    pub sidebar_pane: gtk::Box,
    pub canvas_picker: gtk::DropDown,
    pub assistant_pane: gtk::Box,
    /// Sidebar | rest, and canvas | assistant split views.
    pub outer_pane: gtk::Paned,
    pub inner_pane: gtk::Paned,
    pub status: gtk::Label,
    pub zoom_label: gtk::Label,
    pub pencil_label: gtk::Label,
    pub snippet_search: gtk::SearchEntry,
    pub char_search: gtk::SearchEntry,
    pub tools: Vec<(Tool, gtk::ToggleButton)>,
    pub toolbar: crate::toolbar::Toolbar,
    /// Set while the sidebar is being rebuilt so its selection signal is ignored.
    pub syncing: Cell<bool>,
    /// Rows the sidebar showed after its last rebuild, in order: (id, depth, label). Used to
    /// animate rows that appear and to slide out rows that went away.
    pub sidebar_rows: RefCell<Vec<(String, usize, String)>>,
    /// Keeps the native file dialog alive until it responds.
    pub dialog: RefCell<Option<gtk::FileChooserNative>>,
}

pub type U = Rc<Ui>;

pub const BASE_FONT_SIZE: f64 = 13.0;

// ---------------------------------------------------------------------------
// Cell metrics

pub fn font_desc(st: &App) -> pango::FontDescription {
    let mut d = pango::FontDescription::new();
    d.set_family(&crate::prefs::font_family());
    d.set_absolute_size(st.font_size * pango::SCALE as f64);
    d
}

pub fn cell_size(area: &gtk::DrawingArea, st: &App) -> (f64, f64) {
    let layout = area.create_pango_layout(Some("M"));
    layout.set_font_description(Some(&font_desc(st)));
    let (w, h) = layout.pixel_size();
    (w.max(1) as f64, h.max(1) as f64)
}

pub fn to_cell(area: &gtk::DrawingArea, st: &App, x: f64, y: f64) -> Point {
    let (cw, ch) = cell_size(area, st);
    Point::new((x / cw).floor() as i32 - st.view_offset.x, (y / ch).floor() as i32 - st.view_offset.y)
}

pub fn update_size(ui: &Ui, st: &App) {
    let (cw, ch) = cell_size(&ui.area, st);
    let e = st.buffer.extent;
    let right = (e.x + e.w).max(80) + st.view_offset.x + 40;
    let bottom = (e.y + e.h).max(40) + st.view_offset.y + 20;
    ui.area.set_content_width((right as f64 * cw) as i32);
    ui.area.set_content_height((bottom as f64 * ch) as i32);
}

pub fn refresh(ui: &Ui, st: &App) {
    update_size(ui, st);
    ui.area.queue_draw();
    ui.ruler_top.set_visible(st.show_column_numbers);
    ui.ruler_left.set_visible(st.show_line_numbers);
    ui.ruler_top.queue_draw();
    ui.ruler_left.queue_draw();
    ui.window.set_title(Some(&st.title()));
    ui.zoom_label.set_text(&format!("{:.0}%", st.font_size / BASE_FONT_SIZE * 100.0));
    ui.pencil_label.set_text(&st.pencil_char.to_string());
    let sel = match st.selection.len() {
        0 => String::new(),
        1 => st.selection.first().and_then(|s| st.doc.get(s)).and_then(|o| o.str_("name")).map(|n| format!("   ·   {n}")).unwrap_or_default(),
        n => format!("   ·   {n} shapes selected"),
    };
    let sticky = if st.sticky { " (sticky)" } else { "" };
    if let Some(name) = st.edit_shape.as_ref().and_then(|e| st.doc.get(e)).and_then(|o| o.str_("name")) {
        ui.status.set_text(&format!("Editing {name}: Pencil, Eraser, Bucket Fill and Picker work on its content. Esc or Ctrl+Shift+E to finish."));
        return;
    }
    ui.status.set_text(&format!("{}{sticky}{sel}", st.tool.hint()));
}

/// Rebuild everything that depends on the document structure or selection.
pub fn sync(ui: &U, st: &St) {
    rebuild_sidebar(ui, st);
    rebuild_inspector(ui, st);
    refresh(ui, &st.borrow());
}
