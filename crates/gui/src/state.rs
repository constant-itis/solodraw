//! Editor state: the document, selection, tools and undo history.

use std::path::PathBuf;

use solodraw_core::geom::{Point, Rect};
use solodraw_core::render::render_scene;
use solodraw_core::tixel::Buffer;
use solodraw_core::{Document, ObjExt, Scene};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Pan,
    Rect,
    Table,
    Diamond,
    Line,
    Ellipse,
    Text,
    Surface,
    Pencil,
    Bucket,
    Eraser,
    Picker,
}

impl Tool {
    /// (tool, toolbar label, tooltip, single-key shortcut, icon name, implemented)
    pub const ALL: [(Tool, &'static str, &'static str, char, &'static str, bool); 13] = [
        (Tool::Select, "Select", "Select tool (S)", 's', "md-select", true),
        (Tool::Pan, "Pan", "Pan tool (Space)", ' ', "md-pan", true),
        (Tool::Rect, "Rect", "Rectangle tool (R)", 'r', "md-rect", true),
        (Tool::Table, "Table", "Table tool (H)", 'h', "md-table", true),
        (Tool::Diamond, "Diamond", "Diamond tool (D)", 'd', "md-diamond", true),
        (Tool::Line, "Line", "Line tool (L)", 'l', "md-line", true),
        (Tool::Ellipse, "Ellipse", "Ellipse tool (C)", 'c', "md-ellipse", true),
        (Tool::Text, "Text", "Text tool (T)", 't', "md-text", true),
        (Tool::Surface, "Surface", "Surface tool (F)", 'f', "md-surface", true),
        (Tool::Pencil, "Pencil", "Pencil tool (P)", 'p', "md-pencil", true),
        (Tool::Bucket, "Bucket Fill", "Bucket fill tool (B)", 'b', "md-bucket", true),
        (Tool::Eraser, "Eraser", "Eraser tool (E)", 'e', "md-eraser", true),
        (Tool::Picker, "Picker", "Character Picker (I)", 'i', "md-picker", true),
    ];

    /// Whether the tool can be chosen: implemented, and for the Table tool the opt-in
    /// `feature-flags.table-shape` pref (Monodraw ships its table tool switched off).
    pub fn available(self) -> bool {
        let implemented = Tool::ALL.iter().any(|t| t.0 == self && t.5);
        implemented && (self != Tool::Table || crate::prefs::get_bool(crate::prefs::TABLE_SHAPE, false))
    }

    pub fn hint(self) -> &'static str {
        match self {
            Tool::Select => "Select: click or Shift-click shapes, drag empty canvas to marquee, drag handles to resize; double-click edits text",
            Tool::Rect => "Rectangle: drag to draw",
            Tool::Table => "Table: drag to draw; double-click a cell to edit its text, right-click for rows and columns",
            Tool::Line => "Line: drag between shapes to connect them",
            Tool::Text => "Text: click to place a text label",
            Tool::Pencil => "Pencil: click or drag to paint the current character, then type; right-drag erases",
            Tool::Pan => "Pan: drag to scroll the canvas",
            Tool::Diamond => "Diamond: drag out from the centre",
            Tool::Ellipse => "Ellipse: drag to draw",
            Tool::Surface => "Surface: drag to draw a borderless drawing surface",
            Tool::Bucket => "Bucket fill: click to fill an area of a shape with the current character",
            Tool::Eraser => "Eraser: click or drag to erase custom content",
            Tool::Picker => "Picker: click a character to make it the pencil character",
        }
    }
}

#[derive(Clone, Debug)]
pub enum Drag {
    /// Moving the selection: pointer cell at the start, translation applied so far.
    Move { start: Point, applied: Point, moved: bool },
    /// Resizing the selected rect/ellipse from handle (hx, hy), each -1, 0 or 1.
    Resize { origin: Rect, hx: i32, hy: i32 },
    /// Resizing the selected diamond (radius follows the pointer).
    Radius { centre: Point },
    /// Rubber band for a new rect / text / surface / ellipse.
    Create { start: Point, cur: Point },
    /// New diamond, centre to pointer.
    Diamond { centre: Point, cur: Point },
    /// New line being drawn.
    Line { start: Point, cur: Point },
    /// Dragging end `index` of the selected line.
    LineEnd { line: String, index: usize, cur: Point },
    /// Marquee selection; `base` = selection before the drag (Shift adds to it).
    Marquee { start: Point, cur: Point, base: Vec<String> },
    /// Scrolling the view: pointer start (canvas widget pixels) and the scroll values then.
    Pan { start: (f64, f64), scroll0: (f64, f64) },
    /// Pencil (`Some(char)`) or eraser (`None`) stroke into `shape`.
    Paint { shape: String, c: Option<char>, last: Point },
}

pub struct App {
    pub doc: Document,
    pub canvas: String,
    pub path: Option<PathBuf>,
    /// Selected shapes, in selection order (last = primary, shown in the inspector).
    pub selection: Vec<String>,
    pub tool: Tool,
    /// Keep the tool after creating a shape (activated with Shift).
    pub sticky: bool,
    /// Tool to return to when Space-pan ends.
    pub pan_return: Option<Tool>,
    /// Space is held (hold-to-pan) and a drag happened while held.
    pub space_down: bool,
    pub space_used: bool,
    pub undo: Vec<Document>,
    pub redo: Vec<Document>,
    pub buffer: Buffer,
    pub dirty: bool,
    pub drag: Option<Drag>,
    /// Pencil cursor: (shape receiving the cells, canvas cell, column where Enter returns).
    pub cursor: Option<(String, Point, i32)>,
    pub show_vgrid: bool,
    pub show_hgrid: bool,
    pub show_line_numbers: bool,
    pub show_column_numbers: bool,
    /// Shape highlighted from the sidebar.
    pub hover: Option<String>,
    /// Character the pencil paints (Characters tab / picker).
    pub pencil_char: char,
    /// Groups expanded in the sidebar.
    pub expanded: std::collections::HashSet<String>,
    /// Cells added on the left/top so negative coordinates stay visible.
    pub view_offset: Point,
    pub font_size: f64,
    /// Pastes since the last copy (each one lands one cell further down-right; -1 after a cut).
    /// Where the next shape paste is centred (the context-menu point); else the view centre.
    pub paste_at: Option<Point>,
    /// Alignment guides shown while moving: (vertical, canvas coordinate).
    pub guides: Vec<(bool, i32)>,
    /// Shape whose custom content is being edited (Shape ▸ Edit).
    pub edit_shape: Option<String>,
    /// Undo depth when edit mode began: edits made in it are undone one by one while editing,
    /// then folded into a single document undo step when it ends.
    pub edit_undo_base: Option<usize>,
    /// The document's redo stack, set aside while edit mode runs its own history.
    pub edit_outer_redo: Vec<Document>,
    /// Edit mode: the cursor at each in-edit undo step (where the character went) and at each
    /// redo step. Undo returns the cursor to the undone cell; redo leaves it where it is.
    pub edit_cursors: Vec<Option<(String, Point, i32)>>,
    pub edit_redo_cursors: Vec<Option<(String, Point, i32)>>,
    /// Canvas cell of the last right-click (for context-menu line commands).
    pub context_point: Option<Point>,
}

impl App {
    pub fn new(doc: Document, path: Option<PathBuf>) -> Self {
        let canvas = doc.canvases().first().cloned().unwrap_or_default();
        let mut a = App {
            doc,
            canvas,
            path,
            selection: vec![],
            tool: Tool::Select,
            sticky: false,
            pan_return: None,
            space_down: false,
            space_used: false,
            undo: vec![],
            redo: vec![],
            buffer: Buffer::new(),
            dirty: false,
            drag: None,
            cursor: None,
            show_vgrid: crate::prefs::get_bool(crate::prefs::VGRID, false),
            show_hgrid: crate::prefs::get_bool(crate::prefs::HGRID, false),
            show_line_numbers: crate::prefs::get_bool(crate::prefs::LINE_NUMBERS, true),
            show_column_numbers: crate::prefs::get_bool(crate::prefs::COLUMN_NUMBERS, true),
            hover: None,
            pencil_char: '*',
            expanded: Default::default(),
            view_offset: Point::new(4, 2),
            font_size: 13.0,
            paste_at: None,
            guides: vec![],
            edit_shape: None,
            edit_undo_base: None,
            edit_outer_redo: Vec::new(),
            edit_cursors: Vec::new(),
            edit_redo_cursors: Vec::new(),
            context_point: None,
        };
        a.rerender();
        a
    }

    pub fn empty() -> Self {
        let (mut doc, _) = Document::new_empty();
        if let Some(root) = doc.root().map(|(k, _)| k.to_string()) {
            doc.set_value(&root, "alphabet", serde_json::json!(crate::prefs::get_int(crate::prefs::ALPHABET, 1)));
        }
        App::new(doc, None)
    }

    pub fn scene(&self) -> Scene<'_> {
        Scene::new(&self.doc, &self.canvas, self.doc.unicode())
    }

    pub fn rerender(&mut self) {
        let buffer = render_scene(&self.scene());
        self.buffer = buffer;
        let e = self.buffer.extent;
        if !e.is_empty() {
            // Grow (never shrink) the offset so content left/above the origin stays visible.
            self.view_offset.x = self.view_offset.x.max(4 - e.x);
            self.view_offset.y = self.view_offset.y.max(2 - e.y);
        }
    }

    /// Record the current document for undo before a change.
    pub fn checkpoint(&mut self) {
        if self.edit_shape.is_some() {
            self.edit_cursors.push(self.cursor.clone());
            self.edit_redo_cursors.clear();
        }
        self.undo.push(self.doc.clone());
        if self.undo.len() > 200 {
            self.undo.remove(0);
            if let Some(b) = self.edit_undo_base.as_mut() {
                *b = b.saturating_sub(1);
            }
        }
        self.redo.clear();
        self.dirty = true;
    }

    /// Whether Undo is available. In edit mode only the edits made in it can be undone: on
    /// entry Undo is disabled (Monodraw 1.6.1, golden-gui step 07).
    pub fn can_undo(&self) -> bool {
        self.undo.len() > self.edit_undo_base.unwrap_or(0)
    }

    pub fn undo(&mut self) {
        if !self.can_undo() {
            return;
        }
        if let Some(prev) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.doc, prev));
            let at = self.edit_cursors.pop();
            self.after_history();
            if self.edit_shape.is_some() {
                if let Some(c) = at {
                    self.edit_redo_cursors.push(c.clone());
                    self.cursor = c;
                }
            }
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.doc, next));
            let keep = self.cursor.clone();
            self.after_history();
            if self.edit_shape.is_some() {
                if let Some(c) = self.edit_redo_cursors.pop() {
                    self.edit_cursors.push(c);
                }
                self.cursor = keep;
            }
        }
    }

    /// Enter Shape ▸ Edit mode on `id`. Edit mode has its own history (Monodraw 1.6.1, golden-gui
    /// step 07): Undo starts disabled, each typed character is one step, and the document's redo
    /// stack is set aside until edit mode ends.
    pub fn start_editing(&mut self, id: String) {
        self.edit_shape = Some(id);
        self.edit_undo_base = Some(self.undo.len());
        self.edit_outer_redo = std::mem::take(&mut self.redo);
        self.edit_cursors.clear();
        self.edit_redo_cursors.clear();
    }

    /// Leave edit mode: the whole session becomes ONE document undo step and any pending in-edit
    /// redo is discarded (observed: after Esc, the first Undo removes every edit, the second the
    /// shape). With no net edit the document's own redo stack comes back.
    pub fn stop_editing(&mut self) {
        self.edit_shape = None;
        let outer = std::mem::take(&mut self.edit_outer_redo);
        self.edit_cursors.clear();
        self.edit_redo_cursors.clear();
        if let Some(base) = self.edit_undo_base.take() {
            let base = base.min(self.undo.len());
            self.redo.clear();
            if self.undo.len() > base {
                self.undo.truncate(base + 1);
            } else {
                self.redo = outer;
            }
        }
    }

    fn after_history(&mut self) {
        let doc = &self.doc;
        self.selection.retain(|s| doc.get(s).is_some());
        if self.edit_shape.as_ref().is_some_and(|e| doc.get(e).is_none()) {
            self.stop_editing();
        }
        self.cursor = None;
        self.dirty = true;
        self.rerender();
    }

    /// The primary (most recently selected) shape.
    pub fn primary(&self) -> Option<String> {
        self.selection.last().cloned()
    }

    /// The only selected shape, if exactly one is selected.
    pub fn single(&self) -> Option<String> {
        match self.selection.as_slice() {
            [one] => Some(one.clone()),
            _ => None,
        }
    }

    pub fn is_selected(&self, id: &str) -> bool {
        self.selection.iter().any(|s| s == id)
    }

    pub fn select_only(&mut self, id: Option<String>) {
        self.selection = id.into_iter().collect();
    }

    /// Add `id` to the selection, or remove it if already selected.
    pub fn toggle_selected(&mut self, id: &str) {
        if self.is_selected(id) {
            self.selection.retain(|s| s != id);
        } else {
            self.selection.push(id.to_string());
        }
    }

    /// Union of the selected shapes' frames.
    pub fn selection_frame(&self) -> Option<Rect> {
        let scene = self.scene();
        self.selection.iter().filter_map(|s| scene.frame(s)).reduce(|a, b| a.union(&b))
    }

    /// Selected shapes without those whose ancestor is also selected (so moves apply once).
    pub fn selection_roots(&self) -> Vec<String> {
        let scene = self.scene();
        self.selection
            .iter()
            .filter(|id| {
                let mut cur = scene.parent(id);
                while let Some(p) = cur {
                    if self.is_selected(p) {
                        return false;
                    }
                    cur = scene.parent(p);
                }
                true
            })
            .cloned()
            .collect()
    }

    pub fn title(&self) -> String {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".into());
        format!("{}{} — solodraw", if self.dirty { "• " } else { "" }, name)
    }

    pub fn flag(&self, id: &str, key: &str) -> bool {
        self.doc.get(id).is_some_and(|o| o.boolean(key))
    }

    /// Toggle `locked` / `hidden` on a shape (with undo).
    pub fn toggle_flag(&mut self, id: &str, key: &str) {
        let v = !self.flag(id, key);
        self.checkpoint();
        self.doc.set_value(id, key, serde_json::json!(v));
        if v {
            self.selection.retain(|s| s != id);
        }
        self.rerender();
    }

    /// Topmost hit that is neither locked nor inside a locked ancestor.
    pub fn hit_unlocked(&self, p: Point) -> Option<String> {
        let scene = self.scene();
        let hit = scene.hit_test(p)?;
        let mut cur = Some(hit.as_str());
        while let Some(c) = cur {
            if self.flag(c, "locked") {
                return None;
            }
            cur = scene.parent(c);
        }
        Some(hit)
    }

    /// Translation of the canvas-level parent where new shapes go.
    pub fn insert_parent(&self) -> (String, Point) {
        (self.canvas.clone(), self.scene().translation(&self.canvas))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A distinct document per step: the canvas name records the step.
    fn mark(app: &mut App, n: &str) {
        app.checkpoint();
        let c = app.doc.canvases()[0].clone();
        app.doc.set_value(&c, "name", serde_json::json!(n));
    }

    fn name(app: &App) -> String {
        let c = app.doc.canvases()[0].clone();
        app.doc.get(&c).and_then(|o| o.str_("name")).unwrap_or("").to_string()
    }

    /// Monodraw 1.6.1 edit-mode history (golden-gui step 07).
    #[test]
    fn edit_mode_history_folds_into_one_step() {
        let (doc, _) = Document::new_empty();
        let mut app = App::new(doc, None);
        app.checkpoint();
        let c = app.doc.canvases()[0].clone();
        let id = app.doc.add_rect(&c, Rect::new(0, 0, 10, 5), "");
        mark(&mut app, "rect");
        app.start_editing(id.clone());
        // Undo is disabled on entry.
        assert!(!app.can_undo());
        app.undo();
        assert_eq!(name(&app), "rect");
        for (i, ch) in ["A", "AB", "ABC"].into_iter().enumerate() {
            app.cursor = Some((id.clone(), Point::new(1 + i as i32, 0), 0));
            mark(&mut app, ch);
        }
        app.cursor = Some((id.clone(), Point::new(4, 0), 0));
        // One undo step per character while editing; undo returns the cursor to the undone
        // cell, redo leaves it there.
        app.undo();
        assert_eq!(name(&app), "AB");
        assert_eq!(app.cursor.as_ref().map(|c| c.1), Some(Point::new(3, 0)));
        app.redo();
        assert_eq!(name(&app), "ABC");
        assert_eq!(app.cursor.as_ref().map(|c| c.1), Some(Point::new(3, 0)));
        app.undo();
        // Leaving edit mode: one step for the session, pending in-edit redo discarded.
        app.stop_editing();
        assert!(app.redo.is_empty());
        app.undo();
        assert_eq!(name(&app), "rect");
        assert!(app.doc.get(&id).is_some());
        app.undo();
        app.undo();
        assert!(app.doc.get(&id).is_none());
    }

    #[test]
    fn edit_mode_without_edits_keeps_document_redo() {
        let (doc, _) = Document::new_empty();
        let mut app = App::new(doc, None);
        mark(&mut app, "rect");
        app.undo();
        app.start_editing("x".into());
        app.stop_editing();
        app.redo();
        assert_eq!(name(&app), "rect");
    }
}
