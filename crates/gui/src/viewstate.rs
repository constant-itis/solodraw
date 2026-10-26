//! Per-document view state: zoom, scroll position, grids and rulers, sidebar selection and
//! expanded groups, and the split-view divider positions, kept by file path in
//! `$XDG_STATE_HOME/solodraw/view-state.json` (docs/UI_INVENTORY.md "Saved per document").
//! Monodraw keeps the same through macOS window restoration; here it is saved when a window closes or switches file and comes back
//! whenever the file is opened.

use std::cell::Cell;

use serde_json::{Map, Value};

use crate::prelude::*;

/// Most documents remembered; the least recently saved are dropped.
const KEEP: usize = 200;

fn path() -> PathBuf {
    let state = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| glib::home_dir().join(".local/state"));
    state.join("solodraw").join("view-state.json")
}

fn key(p: &std::path::Path) -> String {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()).to_string_lossy().into_owned()
}

fn read_all() -> Map<String, Value> {
    std::fs::read_to_string(path()).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v.as_object().cloned()).unwrap_or_default()
}

/// Save the view state of the document shown in `ui` (no-op for an unsaved document).
pub fn remember(ui: &U, st: &App) {
    let Some(p) = &st.path else { return };
    let mut all = read_all();
    // Divider positions only while their side pane shows (a hidden pane has no width to keep).
    let sidebar_width = ui.sidebar_pane.is_visible().then(|| ui.outer_pane.position());
    let inner = &ui.inner_pane;
    let assistant_width = (ui.assistant_pane.is_visible() && inner.width() > 0).then(|| inner.width() - inner.position());
    let mut expanded: Vec<&String> = st.expanded.iter().collect();
    expanded.sort();
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    all.insert(
        key(p),
        json!({
            "font_size": st.font_size,
            "scroll": [ui.scroller.hadjustment().value(), ui.scroller.vadjustment().value()],
            "vgrid": st.show_vgrid,
            "hgrid": st.show_hgrid,
            "line_numbers": st.show_line_numbers,
            "column_numbers": st.show_column_numbers,
            "selection": st.selection,
            "expanded": expanded,
            "sidebar_width": sidebar_width,
            "assistant_width": assistant_width,
            "t": stamp,
        }),
    );
    if all.len() > KEEP {
        let mut by_age: Vec<(u64, String)> = all.iter().map(|(k, v)| (v["t"].as_u64().unwrap_or(0), k.clone())).collect();
        by_age.sort();
        for (_, k) in by_age.into_iter().take(all.len() - KEEP) {
            all.remove(&k);
        }
    }
    let file = path();
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(file, serde_json::to_string(&Value::Object(all)).unwrap_or_default());
}

/// Apply the saved view state of the document now in `st`, if there is one. The scroll position
/// is applied once the canvas has been laid out at the restored zoom.
pub fn restore(ui: &U, st: &St) {
    let Some(p) = st.borrow().path.clone() else { return };
    let all = read_all();
    let Some(v) = all.get(&key(&p)) else { return };
    {
        let mut guard = st.borrow_mut();
        let s = &mut *guard;
        if let Some(f) = v["font_size"].as_f64() {
            s.font_size = f.clamp(6.0, 40.0);
        }
        for (k, field) in [("vgrid", &mut s.show_vgrid), ("hgrid", &mut s.show_hgrid)] {
            if let Some(b) = v[k].as_bool() {
                *field = b;
            }
        }
        for (k, field) in [("line_numbers", &mut s.show_line_numbers), ("column_numbers", &mut s.show_column_numbers)] {
            if let Some(b) = v[k].as_bool() {
                *field = b;
            }
        }
        // Only ids still in the document (and, for the selection, on the current canvas).
        let ids = |k: &str| -> Vec<String> { v[k].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default() };
        s.expanded = ids("expanded").into_iter().filter(|id| s.doc.get(id).is_some()).collect();
        let on_canvas: std::collections::HashSet<String> = s.scene().draw_order().into_iter().collect();
        s.selection = ids("selection").into_iter().filter(|id| on_canvas.contains(id)).collect();
        s.rerender();
    }
    restore_panes(ui, v["sidebar_width"].as_i64(), v["assistant_width"].as_i64());
    refresh(ui, &st.borrow());
    sync(ui, st);
    for (adj, want) in [(ui.scroller.hadjustment(), v["scroll"][0].as_f64()), (ui.scroller.vadjustment(), v["scroll"][1].as_f64())] {
        let Some(want) = want else { continue };
        scroll_when_ready(&adj, want);
    }
}

/// Set `adj` to `want` now if the range allows it, else as soon as the canvas grows enough; after
/// two seconds settle for the nearest reachable value so a later zoom is never yanked back.
fn scroll_when_ready(adj: &gtk::Adjustment, want: f64) {
    let fits = move |a: &gtk::Adjustment| a.upper() - a.page_size() >= want;
    if fits(adj) {
        adj.set_value(want);
        return;
    }
    let handler: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::default();
    let h2 = handler.clone();
    // "changed" fires while the viewport is allocating; a value set right then moves the scrollbar
    // but not the canvas, so apply it from an idle callback once allocation is done.
    let id = adj.connect_changed(move |a| {
        if fits(a) {
            if let Some(id) = h2.borrow_mut().take() {
                a.disconnect(id);
            }
            let a = a.clone();
            glib::idle_add_local_once(move || a.set_value(want));
        }
    });
    handler.replace(Some(id));
    let adj = adj.clone();
    glib::timeout_add_local_once(std::time::Duration::from_secs(2), move || {
        if let Some(id) = handler.borrow_mut().take() {
            adj.disconnect(id);
            adj.set_value(want.min(adj.upper() - adj.page_size()).max(adj.lower()));
        }
    });
}

/// Set the divider positions once the window has been laid out (from an idle callback, never
/// during allocation). The assistant width is measured from the right edge.
fn restore_panes(ui: &U, sidebar: Option<i64>, assistant: Option<i64>) {
    if sidebar.is_none() && assistant.is_none() {
        return;
    }
    let (outer, inner) = (ui.outer_pane.clone(), ui.inner_pane.clone());
    let tries = Cell::new(0);
    glib::idle_add_local(move || {
        tries.set(tries.get() + 1);
        if outer.width() <= 0 && tries.get() < 200 {
            return glib::ControlFlow::Continue;
        }
        if let Some(w) = sidebar {
            outer.set_position(w as i32);
        }
        if let Some(w) = assistant {
            // The inner pane's width follows the outer position on the next layout.
            let inner = inner.clone();
            glib::idle_add_local_once(move || {
                if inner.width() > 0 {
                    inner.set_position((inner.width() - w as i32).max(0));
                }
            });
        }
        glib::ControlFlow::Break
    });
}
