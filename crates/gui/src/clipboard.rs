//! Clipboard and structural shape commands: cut/copy/paste (shapes + text), Paste Raw, Duplicate,
//! Group/Ungroup/Flatten, Copy/Paste Style, Reset Content (docs/UI_INVENTORY.md §1.3, §2).
//!
//! Copy puts two formats on the clipboard: the selection rendered as text, and the shapes as a
//! fragment (`solodraw_core::fragment`) under a private MIME type, so pasting into another
//! window (or another instance) gives editable shapes and pasting into a text editor gives text.

use gtk::gio;

use crate::prelude::*;

pub const SHAPES_MIME: &str = "application/x-solodraw-shapes";

thread_local! {
    /// Copied style (Copy Style / Paste Style), shared by every window.
    static STYLE: RefCell<Option<serde_json::Value>> = const { RefCell::new(None) };
}

/// Top-left cell of the visible part of the canvas, in canvas coordinates.
pub fn visible_origin(ui: &Ui, st: &App) -> Point {
    to_cell(&ui.area, st, ui.scroller.hadjustment().value(), ui.scroller.vadjustment().value())
}

/// Centre cell of the visible part of the canvas.
pub fn visible_centre(ui: &Ui, st: &App) -> Point {
    let (h, v) = (ui.scroller.hadjustment(), ui.scroller.vadjustment());
    to_cell(&ui.area, st, h.value() + h.page_size() / 2.0, v.value() + v.page_size() / 2.0)
}

/// When a text field has focus, run its own clipboard/selection action (`clipboard.copy`,
/// `selection.select-all`, ...) instead of the canvas command. Returns true if it did.
pub fn forward_to_text(ui: &Ui, action: &str) -> bool {
    let Some(w) = gtk::prelude::GtkWindowExt::focus(&ui.window) else { return false };
    if w.is::<gtk::Text>() || w.is::<gtk::TextView>() {
        let _ = w.activate_action(action, None);
        return true;
    }
    false
}

pub fn copy(ui: &U, st: &St) {
    if forward_to_text(ui, "clipboard.copy") {
        return;
    }
    let s = st.borrow();
    let roots = s.selection_roots();
    if roots.is_empty() {
        drop(s);
        copy_text(ui, st);
        return;
    }
    let frag = s.doc.extract_fragment(&s.canvas, &roots);
    let text = crate::exportwin::clipboard_text(&s);
    drop(s);
    let bytes = glib::Bytes::from_owned(serde_json::to_vec(&frag).unwrap_or_default());
    let provider = gdk::ContentProvider::new_union(&[
        gdk::ContentProvider::for_bytes(SHAPES_MIME, &bytes),
        gdk::ContentProvider::for_value(&text.to_value()),
    ]);
    if ui.window.clipboard().set_content(Some(&provider)).is_err() {
        ui.window.clipboard().set_text(&text);
    }
    ui.status.set_text(&format!("Copied {} shape{}", roots.len(), if roots.len() == 1 { "" } else { "s" }));
}

pub fn cut(ui: &U, st: &St) {
    if forward_to_text(ui, "clipboard.cut") {
        return;
    }
    if st.borrow().selection_roots().is_empty() {
        return;
    }
    copy(ui, st);
    delete_selection(ui, st);
    // The first paste after a cut lands where the shapes were.
}

/// Paste shapes if the clipboard has them, otherwise its text as a Surface.
pub fn paste(ui: &U, st: &St) {
    if forward_to_text(ui, "clipboard.paste") {
        return;
    }
    let clip = ui.window.clipboard();
    let (ui, st) = (ui.clone(), st.clone());
    if clip.formats().contain_mime_type(SHAPES_MIME) {
        glib::spawn_future_local(async move {
            match read_mime(&clip, SHAPES_MIME).await.and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) {
                Some(frag) => paste_fragment(&ui, &st, &frag),
                None => ui.status.set_text("Nothing to paste"),
            }
        });
    } else {
        glib::spawn_future_local(async move {
            if let Ok(Some(text)) = clip.read_text_future().await {
                paste_text(&ui, &st, &text);
            }
        });
    }
}

/// Paste the clipboard's text as a Surface, even when it also holds shapes.
pub fn paste_raw(ui: &U, st: &St) {
    if forward_to_text(ui, "clipboard.paste") {
        return;
    }
    let clip = ui.window.clipboard();
    let (ui, st) = (ui.clone(), st.clone());
    glib::spawn_future_local(async move {
        if let Ok(Some(text)) = clip.read_text_future().await {
            paste_text(&ui, &st, &text);
        }
    });
}

async fn read_mime(clip: &gdk::Clipboard, mime: &str) -> Option<glib::Bytes> {
    let (stream, _) = clip.read_future(&[mime], glib::Priority::DEFAULT).await.ok()?;
    let out = gio::MemoryOutputStream::new_resizable();
    out.splice_future(&stream, gio::OutputStreamSpliceFlags::CLOSE_SOURCE | gio::OutputStreamSpliceFlags::CLOSE_TARGET, glib::Priority::DEFAULT)
        .await
        .ok()?;
    Some(out.steal_as_bytes())
}

fn paste_fragment(ui: &U, st: &St, frag: &serde_json::Value) {
    {
        let mut s = st.borrow_mut();
        // Centred on the context-menu point or the middle of the view; not renamed, so
        // repeated pastes land in the same place (as Monodraw).
        let target = s.paste_at.take().unwrap_or_else(|| visible_centre(ui, &s));
        let (parent, t) = s.insert_parent();
        let mut doc = s.doc.clone();
        match doc.paste_fragment(frag, &parent, t, Point::default()) {
            Ok(roots) if !roots.is_empty() => {
                let bounds = {
                    let scene = solodraw_core::Scene::new(&doc, &s.canvas, doc.unicode());
                    roots.iter().filter_map(|r| scene.frame(r)).reduce(|a, b| a.union(&b))
                };
                if let Some(b) = bounds {
                    let d = Point::new(target.x - (b.x + b.w / 2), target.y - (b.y + b.h / 2));
                    for r in &roots {
                        doc.translate_shape(r, d);
                    }
                }
                s.checkpoint();
                s.doc = doc;
                s.selection = roots.into_iter().rev().collect();
                s.rerender();
            }
            _ => {
                drop(s);
                ui.status.set_text("Nothing to paste");
                return;
            }
        }
    }
    sync(ui, st);
}

/// Add `text` as a Surface at the pencil cursor, or near the top-left of the view.
pub fn paste_text(ui: &U, st: &St, text: &str) {
    {
        let mut s = st.borrow_mut();
        s.paste_at = None;
        let at = match &s.cursor {
            Some((_, p, _)) => *p,
            None => visible_origin(ui, &s).offset(2, 1),
        };
        let (parent, t) = s.insert_parent();
        let mut doc = s.doc.clone();
        let Some(id) = doc.add_text_surface(&parent, at.sub(t), text) else { return };
        s.checkpoint();
        s.doc = doc;
        s.cursor = None;
        s.select_only(Some(id));
        s.rerender();
    }
    sync(ui, st);
}

pub fn duplicate(ui: &U, st: &St) {
    {
        let mut s = st.borrow_mut();
        let roots = s.selection_roots();
        if roots.is_empty() {
            return;
        }
        let frag = s.doc.extract_fragment(&s.canvas, &roots);
        let (parent, t) = s.insert_parent();
        let mut doc = s.doc.clone();
        let Ok(new) = doc.paste_fragment(&frag, &parent, t, Point::new(1, 1)) else { return };
        for id in &new {
            if let Some(name) = doc.get(id).and_then(|o| o.str_("name")).map(str::to_string) {
                doc.set_value(id, "name", json!(solodraw_core::fragment::copy_name(&name)));
            }
        }
        s.checkpoint();
        s.doc = doc;
        s.selection = new.into_iter().rev().collect();
        s.rerender();
    }
    sync(ui, st);
}

/// Selection roots that share the primary shape's parent.
fn siblings_of_primary(s: &App) -> Vec<String> {
    let roots = s.selection_roots();
    let scene = s.scene();
    let parent = s.primary().and_then(|p| {
        let mut cur = p;
        // The primary may be inside a selected group; use its selected ancestor.
        while !roots.contains(&cur) {
            cur = scene.parent(&cur)?.to_string();
        }
        scene.parent(&cur).map(str::to_string)
    });
    roots.into_iter().filter(|r| scene.parent(r).map(str::to_string) == parent).collect()
}

pub fn group(ui: &U, st: &St) {
    {
        let mut s = st.borrow_mut();
        let ids = siblings_of_primary(&s);
        if ids.len() < 2 {
            drop(s);
            ui.status.set_text("Select two or more shapes to group");
            return;
        }
        s.checkpoint();
        let canvas = s.canvas.clone();
        let g = s.doc.group_shapes(&canvas, &ids);
        s.select_only(g);
        s.rerender();
    }
    sync(ui, st);
}

pub fn ungroup(ui: &U, st: &St) {
    {
        let mut s = st.borrow_mut();
        let groups: Vec<String> =
            s.selection_roots().into_iter().filter(|id| s.doc.get(id).is_some_and(|o| o.type_id() == 3 && o.int("shape_type") == Some(8))).collect();
        if groups.is_empty() {
            return;
        }
        s.checkpoint();
        let canvas = s.canvas.clone();
        let mut sel = Vec::new();
        for g in groups {
            sel.extend(s.doc.ungroup(&canvas, &g).into_iter().rev());
        }
        s.selection = sel;
        s.rerender();
    }
    sync(ui, st);
}

pub fn flatten(ui: &U, st: &St) {
    {
        let mut s = st.borrow_mut();
        let ids = siblings_of_primary(&s);
        if ids.is_empty() {
            return;
        }
        let canvas = s.canvas.clone();
        let mut doc = s.doc.clone();
        let Some(surface) = doc.flatten(&canvas, &ids) else { return };
        s.checkpoint();
        s.doc = doc;
        s.select_only(Some(surface));
        s.rerender();
    }
    sync(ui, st);
}

pub fn copy_style(ui: &U, st: &St) {
    let s = st.borrow();
    let Some(style) = s.primary().and_then(|p| s.doc.style_of(&p)) else { return };
    STYLE.with(|c| *c.borrow_mut() = Some(style));
    ui.status.set_text("Style copied");
}

pub fn paste_style(ui: &U, st: &St) {
    let Some(style) = STYLE.with(|c| c.borrow().clone()) else {
        ui.status.set_text("No style copied");
        return;
    };
    {
        let mut s = st.borrow_mut();
        let ids = s.selection.clone();
        if ids.is_empty() {
            return;
        }
        s.checkpoint();
        for id in &ids {
            s.doc.apply_style(id, &style);
        }
        s.rerender();
    }
    sync(ui, st);
}

pub fn reset_content(ui: &U, st: &St) {
    {
        let mut s = st.borrow_mut();
        let ids = s.selection.clone();
        if ids.is_empty() {
            return;
        }
        s.checkpoint();
        for id in &ids {
            s.doc.reset_content(id);
        }
        s.cursor = None;
        s.rerender();
    }
    sync(ui, st);
}
