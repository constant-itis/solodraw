//! A JSON command layer over the editing primitives: one entry point (`Session::exec`) that
//! front ends drive by name — the MCP server, scripts, and anything else that wants to build
//! or change a drawing without the GUI. Every command takes a JSON object of arguments and
//! returns a JSON result; [`COMMANDS`] describes them (name, summary, argument schema).
//!
//! Coordinates are canvas cells, x right and y down. Shapes are referred to by object id or,
//! when it is unique on the canvas, by name.

use std::path::PathBuf;

use serde_json::{json, Map, Value};

use crate::edit::{shape_display_name, LineEnd};
use crate::geom::{Point, Rect};
use crate::{canvas_to_text, Document, ObjExt, Scene, TextOptions};

type Res = std::result::Result<Value, String>;

/// An open document plus the state a front end needs around it.
pub struct Session {
    pub doc: Document,
    pub path: Option<PathBuf>,
    pub canvas: String,
    pub dirty: bool,
    undo: Vec<Document>,
    redo: Vec<Document>,
}

impl Default for Session {
    fn default() -> Self {
        let (doc, canvas) = Document::new_empty();
        Session { doc, path: None, canvas, dirty: false, undo: vec![], redo: vec![] }
    }
}

/// One command: name, one-line summary and JSON Schema of its arguments.
pub struct CommandSpec {
    pub name: &'static str,
    pub summary: &'static str,
    pub schema: fn() -> Value,
}

const SIDE_DOC: &str = "top | bottom | left | right | auto (default: the side facing the other shape)";

fn obj_schema(props: Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": props, "required": required, "additionalProperties": false})
}

fn style_props() -> Map<String, Value> {
    let v = json!({
        "name": {"type": "string", "description": "Shape name; can be used instead of the id afterwards"},
        "border": {"type": "string", "enum": ["light", "heavy", "double", "rounded", "rounded-heavy", "none", "opaque", "transparent"],
                   "description": "Border style (rounded = light with ╭╮╰╯ corners; ASCII documents draw + - |)"},
        "corners": {"type": "string", "description": "Four characters for TL TR BL BR corners, e.g. \"++++\" or \"BWH$\""},
        "fill": {"type": "string", "description": "Background: opaque (spaces, hides what is under), none (see-through), block, or a single character to fill with"},
        "align": {"type": "string", "enum": ["left", "center", "right"], "description": "Horizontal text alignment (default center)"},
        "valign": {"type": "string", "enum": ["top", "middle", "bottom"], "description": "Vertical text placement (default middle)"},
        "font": {"type": "string", "description": "FIGlet font name for big lettering (e.g. standard, small, big); empty for plain text"},
        "shadow": {"type": "boolean", "description": "Drop shadow offset (1,1)"},
        "padding": {"type": "integer", "description": "Text inset on all four sides inside the border"},
    });
    v.as_object().unwrap().clone()
}

fn with(mut base: Map<String, Value>, extra: Value) -> Value {
    for (k, v) in extra.as_object().unwrap() {
        base.insert(k.clone(), v.clone());
    }
    Value::Object(base)
}

fn int() -> Value {
    json!({"type": "integer"})
}

pub const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        name: "new",
        summary: "Start a new empty document (one canvas). Optionally give the path it will be saved to.",
        schema: || obj_schema(json!({"path": {"type": "string"}, "ascii": {"type": "boolean", "description": "Use the ASCII alphabet (+-|) instead of Unicode box drawing"}}), &[]),
    },
    CommandSpec {
        name: "open",
        summary: "Open a .monopic file; it becomes the current document.",
        schema: || obj_schema(json!({"path": {"type": "string"}}), &["path"]),
    },
    CommandSpec {
        name: "save",
        summary: "Save the current document (to its path, or to `path`, which then becomes its path).",
        schema: || obj_schema(json!({"path": {"type": "string"}}), &[]),
    },
    CommandSpec {
        name: "render",
        summary: "Render the current canvas as text, starting at canvas cell (0,0). With ruler (default true) the \
                  rows and columns are numbered so you can read coordinates off the drawing. Check your work with this after edits.",
        schema: || obj_schema(json!({
            "ruler": {"type": "boolean"},
            "unicode": {"type": "boolean", "description": "Override the document alphabet"},
            "region": {"type": "array", "items": int(), "minItems": 4, "maxItems": 4, "description": "[x, y, w, h] to render only part of the canvas"},
        }), &[]),
    },
    CommandSpec {
        name: "list_shapes",
        summary: "List the shapes on the current canvas (top of the z-order first) with id, name, kind, frame [x,y,w,h], text and children.",
        schema: || obj_schema(json!({}), &[]),
    },
    CommandSpec {
        name: "add_box",
        summary: "Add a rectangle with optional text inside (wrapped and centred by default). Returns its id.",
        schema: || obj_schema(with(style_props(), json!({
            "x": int(), "y": int(), "w": int(), "h": int(), "text": {"type": "string"},
        })), &["x", "y", "w", "h"]),
    },
    CommandSpec {
        name: "add_text",
        summary: "Add borderless text with its top-left at (x, y). Size defaults to fit the text; left aligned. Use font for FIGlet lettering.",
        schema: || obj_schema(with(style_props(), json!({
            "x": int(), "y": int(), "w": int(), "h": int(), "text": {"type": "string"},
        })), &["x", "y", "text"]),
    },
    CommandSpec {
        name: "add_art",
        summary: "Place literal multi-line character art with its top-left at (x, y), one cell per character. \
                  Spaces are see-through. Use this for hand-made drawings, portraits and glyph patterns.",
        schema: || obj_schema(json!({"x": int(), "y": int(), "art": {"type": "string"}, "name": {"type": "string"}}), &["x", "y", "art"]),
    },
    CommandSpec {
        name: "add_ellipse",
        summary: "Add an ellipse bounded by (x, y, w, h).",
        schema: || obj_schema(json!({"x": int(), "y": int(), "w": int(), "h": int(), "name": {"type": "string"}}), &["x", "y", "w", "h"]),
    },
    CommandSpec {
        name: "add_diamond",
        summary: "Add a diamond centred on (cx, cy) with the given radius.",
        schema: || obj_schema(json!({"cx": int(), "cy": int(), "radius": int(), "name": {"type": "string"}}), &["cx", "cy", "radius"]),
    },
    CommandSpec {
        name: "connect",
        summary: "Draw a connector from one shape to another. It is routed around shapes and follows them when they move.",
        schema: || obj_schema(json!({
            "from": {"type": "string"}, "to": {"type": "string"},
            "from_side": {"type": "string", "description": SIDE_DOC}, "to_side": {"type": "string", "description": SIDE_DOC},
            "head": {"type": "string", "enum": ["arrow", "open-arrow", "none", "both"], "description": "Arrow heads (default arrow at the end)"},
            "style": {"type": "string", "enum": ["light", "heavy", "double"]},
            "label": {"type": "string", "description": "Text placed on the middle of the line"},
        }), &["from", "to"]),
    },
    CommandSpec {
        name: "add_line",
        summary: "Draw a free orthogonal line through two or more points [[x,y], ...].",
        schema: || obj_schema(json!({
            "points": {"type": "array", "items": {"type": "array", "items": int(), "minItems": 2, "maxItems": 2}, "minItems": 2},
            "head": {"type": "string", "enum": ["arrow", "open-arrow", "none", "both"], "description": "default none"},
            "style": {"type": "string", "enum": ["light", "heavy", "double"]},
            "name": {"type": "string"},
        }), &["points"]),
    },
    CommandSpec {
        name: "update",
        summary: "Change a shape: text, position (x, y = new top-left), size (w, h), or any style option.",
        schema: || obj_schema(with(style_props(), json!({
            "id": {"type": "string"}, "text": {"type": "string"},
            "x": int(), "y": int(), "w": int(), "h": int(),
        })), &["id"]),
    },
    CommandSpec {
        name: "move",
        summary: "Move shapes by (dx, dy). Attached connectors follow.",
        schema: || obj_schema(json!({"ids": {"type": "array", "items": {"type": "string"}}, "dx": int(), "dy": int()}), &["ids", "dx", "dy"]),
    },
    CommandSpec {
        name: "delete",
        summary: "Delete shapes.",
        schema: || obj_schema(json!({"ids": {"type": "array", "items": {"type": "string"}}}), &["ids"]),
    },
    CommandSpec {
        name: "group",
        summary: "Group top-level shapes so they move together. Returns the group id.",
        schema: || obj_schema(json!({"ids": {"type": "array", "items": {"type": "string"}}, "name": {"type": "string"}}), &["ids"]),
    },
    CommandSpec {
        name: "ungroup",
        summary: "Dissolve a group, keeping its shapes.",
        schema: || obj_schema(json!({"id": {"type": "string"}}), &["id"]),
    },
    CommandSpec {
        name: "duplicate",
        summary: "Copy shapes (with everything inside them) offset by (dx, dy). Returns the new ids.",
        schema: || obj_schema(json!({"ids": {"type": "array", "items": {"type": "string"}}, "dx": int(), "dy": int()}), &["ids", "dx", "dy"]),
    },
    CommandSpec {
        name: "undo",
        summary: "Undo the last change.",
        schema: || obj_schema(json!({}), &[]),
    },
    CommandSpec {
        name: "redo",
        summary: "Redo the last undone change.",
        schema: || obj_schema(json!({}), &[]),
    },
];

fn get_i(a: &Value, k: &str) -> Option<i32> {
    a.get(k).and_then(Value::as_i64).map(|v| v as i32)
}

fn need_i(a: &Value, k: &str) -> std::result::Result<i32, String> {
    get_i(a, k).ok_or_else(|| format!("missing integer argument '{k}'"))
}

fn get_s<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Value::as_str)
}

fn ids_arg(a: &Value) -> std::result::Result<Vec<String>, String> {
    let v = a.get("ids").and_then(Value::as_array).ok_or("missing array argument 'ids'")?;
    Ok(v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
}

fn head_types(head: Option<&str>, default_end: bool) -> std::result::Result<(i64, i64), String> {
    Ok(match head {
        None if default_end => (0, 1),
        None | Some("none") => (0, 0),
        Some("arrow") => (0, 1),
        Some("open-arrow") => (0, 2),
        Some("both") => (1, 1),
        Some(h) => return Err(format!("unknown head '{h}'")),
    })
}

fn line_style(s: &str) -> std::result::Result<i64, String> {
    Ok(match s {
        "light" => 3,
        "heavy" => 4,
        "double" => 5,
        _ => return Err(format!("unknown line style '{s}'")),
    })
}

/// Commands that only read the document.
pub fn is_read_only(name: &str) -> bool {
    matches!(name, "render" | "list_shapes")
}

impl Session {
    /// A session over an existing document (e.g. a copy of the editor's), on `canvas`.
    pub fn from_doc(doc: Document, canvas: String) -> Session {
        Session { doc, path: None, canvas, dirty: false, undo: vec![], redo: vec![] }
    }

    pub fn open(path: impl Into<PathBuf>) -> std::result::Result<Session, String> {
        let path = path.into();
        let doc = Document::open(&path).map_err(|e| format!("could not open {}: {e}", path.display()))?;
        let canvas = doc.canvases().first().cloned().ok_or("document has no canvas")?;
        Ok(Session { doc, path: Some(path), canvas, dirty: false, undo: vec![], redo: vec![] })
    }

    /// Run command `name` with JSON arguments `args`.
    pub fn exec(&mut self, name: &str, args: &Value) -> Res {
        let empty = json!({});
        let a = if args.is_null() { &empty } else { args };
        match name {
            "new" => {
                *self = Session::default();
                self.path = get_s(a, "path").map(PathBuf::from);
                if a.get("ascii").and_then(Value::as_bool) == Some(true) {
                    let root = self.doc.root().map(|(k, _)| k.to_string()).unwrap_or_default();
                    self.doc.set_value(&root, "alphabet", json!(2));
                }
                Ok(json!({"canvas": self.canvas, "path": self.path}))
            }
            "open" => {
                let path = get_s(a, "path").ok_or("missing 'path'")?;
                *self = Session::open(path)?;
                Ok(json!({"canvas": self.canvas, "shapes": self.shape_count()}))
            }
            "save" => {
                if let Some(p) = get_s(a, "path") {
                    self.path = Some(PathBuf::from(p));
                }
                let path = self.path.clone().ok_or("no path: pass 'path'")?;
                self.doc.save(&path).map_err(|e| format!("save failed: {e}"))?;
                self.dirty = false;
                Ok(json!({"saved": path}))
            }
            "render" => Ok(json!(self.render(a))),
            "list_shapes" => Ok(self.list_shapes()),
            "undo" | "redo" => {
                let (from, to) = if name == "undo" { (&mut self.undo, &mut self.redo) } else { (&mut self.redo, &mut self.undo) };
                let Some(prev) = from.pop() else { return Err(format!("nothing to {name}")) };
                to.push(std::mem::replace(&mut self.doc, prev));
                self.dirty = true;
                Ok(json!({"ok": true}))
            }
            _ => {
                if !COMMANDS.iter().any(|c| c.name == name) {
                    return Err(format!("unknown command '{name}'"));
                }
                let before = self.doc.clone();
                match self.edit(name, a) {
                    Ok(v) => {
                        self.undo.push(before);
                        if self.undo.len() > 200 {
                            self.undo.remove(0);
                        }
                        self.redo.clear();
                        self.dirty = true;
                        Ok(v)
                    }
                    Err(e) => {
                        // Leave the document as it was: no half-applied edits.
                        self.doc = before;
                        Err(e)
                    }
                }
            }
        }
    }

    fn shape_count(&self) -> usize {
        Scene::new(&self.doc, &self.canvas, true).draw_order().len()
    }

    fn render(&self, a: &Value) -> String {
        let ruler = a.get("ruler").and_then(Value::as_bool).unwrap_or(true);
        let opts = TextOptions {
            anchor_origin: true,
            trim_padding: false,
            trim_trailing: true,
            unicode: a.get("unicode").and_then(Value::as_bool),
        };
        let text = canvas_to_text(&self.doc, Some(&self.canvas), &opts).unwrap_or_default();
        let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
        let (mut ox, mut oy, mut width) = (0, 0, None);
        if let Some(r) = a.get("region").and_then(Value::as_array) {
            let r: Vec<i64> = r.iter().filter_map(Value::as_i64).collect();
            if r.len() == 4 {
                let (x, y, w, h) = (r[0].max(0) as usize, r[1].max(0) as usize, r[2].max(0) as usize, r[3].max(0) as usize);
                lines = (y..y + h)
                    .map(|i| lines.get(i).map(|l| l.chars().skip(x).take(w).collect::<String>().trim_end().to_string()).unwrap_or_default())
                    .collect();
                (ox, oy, width) = (x, y, Some(w));
            }
        }
        if !ruler {
            return lines.join("\n");
        }
        let width = width.unwrap_or_else(|| lines.iter().map(|l| l.chars().count()).max().unwrap_or(0).max(10));
        let gutter = (oy + lines.len()).to_string().len().max(2);
        let pad = " ".repeat(gutter + 1);
        let tens: String = (ox..ox + width).map(|x| if x % 10 == 0 || x == ox { char::from_digit((x / 10 % 10) as u32, 10).unwrap() } else { ' ' }).collect();
        let ones: String = (ox..ox + width).map(|x| char::from_digit((x % 10) as u32, 10).unwrap()).collect();
        let mut out = vec![format!("{pad}{}", tens.trim_end()), format!("{pad}{ones}")];
        for (i, l) in lines.iter().enumerate() {
            out.push(format!("{:>gutter$}|{l}", oy + i));
        }
        out.join("\n")
    }

    fn describe(&self, scene: &Scene, id: &str) -> Value {
        let o = scene.obj(id).unwrap();
        let st = o.int("shape_type").unwrap_or(0);
        let kind = match (o.type_id(), st) {
            (20, 4) => "text",
            (20, 5) => "art",
            (20, _) => "box",
            (3, _) => "group",
            _ => shape_display_name(st),
        }
        .to_lowercase();
        let mut d = json!({"id": id, "name": o.str_("name").unwrap_or(""), "kind": kind});
        if let Some(f) = scene.frame(id) {
            d["frame"] = json!([f.x, f.y, f.w, f.h]);
        }
        if let Some(t) = self.doc.text_of(id).and_then(|t| self.doc.get(&t)).and_then(|t| t.str_("text")) {
            if !t.is_empty() {
                d["text"] = json!(t);
            }
        }
        let kids = o.refs("subshape_refs");
        if !kids.is_empty() {
            d["children"] = Value::Array(kids.iter().map(|k| self.describe(scene, k)).collect());
        }
        d
    }

    fn list_shapes(&self) -> Value {
        let scene = Scene::new(&self.doc, &self.canvas, true);
        let top = self.doc.get(&self.canvas).map(|c| c.refs("subshape_refs")).unwrap_or_default();
        json!({"canvas": self.canvas, "shapes": top.iter().map(|id| self.describe(&scene, id)).collect::<Vec<_>>()})
    }

    /// Resolve an id or a unique shape name on the current canvas.
    pub fn resolve(&self, key: &str) -> std::result::Result<String, String> {
        let scene = Scene::new(&self.doc, &self.canvas, true);
        let all = scene.draw_order();
        let mut groups = Vec::new();
        let mut stack: Vec<String> = self.doc.get(&self.canvas).map(|c| c.refs("subshape_refs").into_iter().map(str::to_string).collect()).unwrap_or_default();
        while let Some(s) = stack.pop() {
            if let Some(o) = self.doc.get(&s) {
                stack.extend(o.refs("subshape_refs").into_iter().map(str::to_string));
            }
            groups.push(s);
        }
        if groups.iter().any(|s| s == key) || all.iter().any(|s| s == key) {
            return Ok(key.to_string());
        }
        let hits: Vec<&String> = groups.iter().filter(|s| self.doc.get(s).and_then(|o| o.str_("name")) == Some(key)).collect();
        match hits.len() {
            1 => Ok(hits[0].clone()),
            0 => Err(format!("no shape with id or name '{key}'")),
            n => Err(format!("{n} shapes are named '{key}'; use the id")),
        }
    }

    fn frame_of(&self, id: &str) -> Option<Rect> {
        Scene::new(&self.doc, &self.canvas, true).frame(id)
    }

    fn edit(&mut self, name: &str, a: &Value) -> Res {
        let canvas = self.canvas.clone();
        match name {
            "add_box" | "add_text" => {
                let (x, y) = (need_i(a, "x")?, need_i(a, "y")?);
                let text = get_s(a, "text").unwrap_or("");
                let st = if name == "add_box" { 1 } else { 4 };
                let (w, h) = if name == "add_box" {
                    (need_i(a, "w")?, need_i(a, "h")?)
                } else {
                    let tw = text.lines().map(|l| l.chars().count()).max().unwrap_or(0) as i32;
                    (get_i(a, "w").unwrap_or(tw.max(1)), get_i(a, "h").unwrap_or(text.lines().count().max(1) as i32))
                };
                let id = self.doc.add_rect_typed(&canvas, Rect::new(x, y, w.max(1), h.max(1)), text, st);
                if name == "add_text" {
                    self.doc.set_value(&id, "rectangular_border_enabled", json!(false));
                    self.doc.set_value(&id, "background_enabled", json!(false));
                    if let Some(t) = self.doc.text_of(&id) {
                        self.doc.set_value(&t, "alignment", json!(0));
                        self.doc.set_value(&t, "position", json!(0));
                    }
                    if get_s(a, "font").is_some_and(|f| !f.is_empty()) && (get_i(a, "w").is_none() || get_i(a, "h").is_none()) {
                        self.fit_figlet(&id, get_s(a, "font").unwrap(), text, get_i(a, "w"), get_i(a, "h"));
                    }
                }
                self.apply_style(&id, a)?;
                Ok(self.created(&id))
            }
            "add_art" => {
                let (x, y) = (need_i(a, "x")?, need_i(a, "y")?);
                let art = get_s(a, "art").ok_or("missing 'art'")?;
                let id = self.doc.add_text_surface(&canvas, Point::new(x, y), art).ok_or("art has no visible characters")?;
                self.set_name(&id, a);
                Ok(self.created(&id))
            }
            "add_ellipse" => {
                let r = Rect::new(need_i(a, "x")?, need_i(a, "y")?, need_i(a, "w")?.max(2), need_i(a, "h")?.max(2));
                let id = self.doc.add_ellipse(&canvas, r);
                self.set_name(&id, a);
                Ok(self.created(&id))
            }
            "add_diamond" => {
                let id = self.doc.add_diamond(&canvas, Point::new(need_i(a, "cx")?, need_i(a, "cy")?), need_i(a, "radius")?);
                self.set_name(&id, a);
                Ok(self.created(&id))
            }
            "connect" => {
                let from = self.resolve(get_s(a, "from").ok_or("missing 'from'")?)?;
                let to = self.resolve(get_s(a, "to").ok_or("missing 'to'")?)?;
                let (fa, ta) = (self.frame_of(&from).ok_or("'from' has no frame")?, self.frame_of(&to).ok_or("'to' has no frame")?);
                let fs = side_position(get_s(a, "from_side"), fa, ta)?;
                let ts = side_position(get_s(a, "to_side"), ta, fa)?;
                let (sh, eh) = head_types(get_s(a, "head"), true)?;
                let at = |s: &str, p| LineEnd::Attached { shape: s.to_string(), rect_position: p };
                let line = self.doc.add_line(&canvas, Point::default(), at(&from, fs), at(&to, ts), eh);
                self.finish_line(&line, sh, a)?;
                let mut out = self.created(&line);
                if let Some(l) = get_s(a, "label").filter(|l| !l.is_empty()) {
                    let label = self.doc.add_line_label(&canvas, &line, l).ok_or("label failed")?;
                    out["label_id"] = json!(label);
                }
                Ok(out)
            }
            "add_line" => {
                let pts: Vec<Point> = a
                    .get("points")
                    .and_then(Value::as_array)
                    .ok_or("missing 'points'")?
                    .iter()
                    .filter_map(|p| Some(Point::new(p.get(0)?.as_i64()? as i32, p.get(1)?.as_i64()? as i32)))
                    .collect();
                if pts.len() < 2 {
                    return Err("need at least two [x, y] points".into());
                }
                let (sh, eh) = head_types(get_s(a, "head"), false)?;
                let line = self.doc.add_line(&canvas, Point::default(), LineEnd::Free(pts[0]), LineEnd::Free(*pts.last().unwrap()), eh);
                // Free lines take their path from the points, horizontal leg first.
                self.doc.set_value(&line, "line_type", json!(2));
                for (i, p) in pts[1..pts.len() - 1].iter().enumerate() {
                    self.doc.insert_line_point(&line, i, *p, Point::default());
                }
                self.finish_line(&line, sh, a)?;
                self.set_name(&line, a);
                Ok(self.created(&line))
            }
            "update" => {
                let id = self.resolve(get_s(a, "id").ok_or("missing 'id'")?)?;
                if let Some(t) = get_s(a, "text") {
                    let tid = self.doc.text_of(&id).ok_or("this shape has no text")?;
                    self.doc.set_value(&tid, "text", json!(t));
                }
                if get_i(a, "x").is_some() || get_i(a, "y").is_some() {
                    let f = self.frame_of(&id).ok_or("shape has no frame")?;
                    let d = Point::new(get_i(a, "x").unwrap_or(f.x) - f.x, get_i(a, "y").unwrap_or(f.y) - f.y);
                    self.doc.move_shape(&id, d);
                }
                if get_i(a, "w").is_some() || get_i(a, "h").is_some() {
                    let f = self.frame_of(&id).ok_or("shape has no frame")?;
                    let (w, h) = (get_i(a, "w").unwrap_or(f.w), get_i(a, "h").unwrap_or(f.h));
                    match self.doc.get(&id).map(|o| o.type_id()) {
                        Some(20 | 28 | 25) => self.doc.resize_shape(&id, w, h),
                        Some(21) => self.doc.set_value(&id, "radius", json!((w.max(h) / 2).max(1))),
                        _ => return Err("this shape cannot be resized".into()),
                    }
                }
                self.apply_style(&id, a)?;
                Ok(json!({"id": id, "frame": self.frame_of(&id).map(|f| [f.x, f.y, f.w, f.h])}))
            }
            "move" => {
                let d = Point::new(need_i(a, "dx")?, need_i(a, "dy")?);
                for k in ids_arg(a)? {
                    let id = self.resolve(&k)?;
                    self.doc.move_shape(&id, d);
                }
                Ok(json!({"ok": true}))
            }
            "delete" => {
                for k in ids_arg(a)? {
                    let id = self.resolve(&k)?;
                    self.doc.delete_shape(&canvas, &id);
                }
                Ok(json!({"ok": true}))
            }
            "group" => {
                let ids = ids_arg(a)?.iter().map(|k| self.resolve(k)).collect::<std::result::Result<Vec<_>, _>>()?;
                let top = self.doc.get(&canvas).map(|c| c.refs("subshape_refs").into_iter().map(str::to_string).collect::<Vec<_>>()).unwrap_or_default();
                if let Some(bad) = ids.iter().find(|i| !top.contains(i)) {
                    return Err(format!("{bad} is not a top-level shape (ungroup its group first)"));
                }
                let g = self.doc.group_shapes(&canvas, &ids).ok_or("group failed")?;
                self.set_name(&g, a);
                Ok(self.created(&g))
            }
            "ungroup" => {
                let id = self.resolve(get_s(a, "id").ok_or("missing 'id'")?)?;
                let kids = self.doc.ungroup(&canvas, &id);
                if kids.is_empty() {
                    return Err("not a group".into());
                }
                Ok(json!({"ids": kids}))
            }
            "duplicate" => {
                let ids = ids_arg(a)?.iter().map(|k| self.resolve(k)).collect::<std::result::Result<Vec<_>, _>>()?;
                let frag = self.doc.extract_fragment(&canvas, &ids);
                let d = Point::new(need_i(a, "dx")?, need_i(a, "dy")?);
                let new = self.doc.paste_fragment(&frag, &canvas, Point::default(), d).map_err(|e| e.to_string())?;
                Ok(json!({"ids": new}))
            }
            _ => Err(format!("unknown command '{name}'")),
        }
    }

    fn created(&self, id: &str) -> Value {
        let mut v = json!({"id": id, "name": self.doc.get(id).and_then(|o| o.str_("name")).unwrap_or("")});
        if let Some(f) = self.frame_of(id) {
            v["frame"] = json!([f.x, f.y, f.w, f.h]);
        }
        v
    }

    fn set_name(&mut self, id: &str, a: &Value) {
        if let Some(n) = get_s(a, "name") {
            self.doc.set_value(id, "name", json!(n));
        }
    }

    fn finish_line(&mut self, line: &str, start_head: i64, a: &Value) -> std::result::Result<(), String> {
        if start_head != 0 {
            let e = self.doc.get(line).and_then(|o| o.refs("segment_end_refs").first().map(|s| s.to_string()));
            if let Some(e) = e {
                self.doc.set_value(&e, "head_type", json!(start_head));
            }
        }
        if let Some(s) = get_s(a, "style") {
            self.doc.set_value(line, "line_style", json!(line_style(s)?));
        }
        Ok(())
    }

    /// Size a FIGlet text shape to its rendered lettering.
    fn fit_figlet(&mut self, id: &str, font: &str, text: &str, w: Option<i32>, h: Option<i32>) {
        let Some(f) = crate::figlet::load_font(font) else { return };
        let rows = f.render(text, usize::MAX);
        let lw = rows.iter().flatten().map(|r| r.chars().count()).max().unwrap_or(1) as i32;
        let lh = rows.iter().map(|b| b.len()).sum::<usize>().max(1) as i32;
        self.doc.resize_shape(id, w.unwrap_or(lw), h.unwrap_or(lh));
    }

    fn apply_style(&mut self, id: &str, a: &Value) -> std::result::Result<(), String> {
        self.set_name(id, a);
        let is_rect = self.doc.get(id).is_some_and(|o| o.type_id() == 20);
        let needs_rect = ["border", "corners", "fill", "shadow"];
        if !is_rect {
            if let Some(k) = needs_rect.iter().find(|k| a.get(**k).is_some()) {
                return Err(format!("'{k}' applies to boxes and text only"));
            }
        }
        if let Some(b) = get_s(a, "border") {
            let (enabled, v, round) = match b {
                "light" => (true, 4, None),
                "heavy" => (true, 5, None),
                "double" => (true, 6, None),
                "rounded" => (true, 4, Some("╭╮╰╯")),
                "rounded-heavy" => (true, 5, Some("╭╮╰╯")),
                "opaque" => (true, 3, None),
                "transparent" => (true, 2, None),
                "none" => (false, 4, None),
                _ => return Err(format!("unknown border '{b}'")),
            };
            self.doc.set_value(id, "rectangular_border_enabled", json!(enabled));
            self.doc.set_value(id, "rectangular_border", json!(v));
            self.set_corners(id, round.unwrap_or("\0\0\0\0"));
        }
        if let Some(c) = get_s(a, "corners") {
            if c.chars().count() != 4 {
                return Err("'corners' needs exactly four characters (TL TR BL BR)".into());
            }
            self.set_corners(id, c);
        }
        if let Some(f) = get_s(a, "fill") {
            let (on, ty, ch) = match f {
                "opaque" => (true, 2, None),
                "none" => (false, 2, None),
                "block" => (true, 4, None),
                s if s.chars().count() == 1 => (true, 3, Some(s)),
                _ => return Err(format!("unknown fill '{f}'")),
            };
            self.doc.set_value(id, "background_enabled", json!(on));
            self.doc.set_value(id, "background_type", json!(ty));
            if let Some(c) = ch {
                self.doc.set_value(id, "background_tixel", json!(c));
            }
        }
        if let Some(s) = a.get("shadow").and_then(Value::as_bool) {
            self.doc.set_value(id, "shadow_enabled", json!(s));
            if s {
                self.doc.set_value(id, "shadow_offset", json!("1,1"));
                self.doc.set_value(id, "shadow_intensity", json!(1));
            }
        }
        let text_keys = ["align", "valign", "font", "padding"];
        if text_keys.iter().any(|k| a.get(*k).is_some()) {
            let tid = self.doc.text_of(id).ok_or("this shape has no text")?;
            if let Some(v) = get_s(a, "align") {
                let n = match v {
                    "left" => 0,
                    "center" | "centre" => 1,
                    "right" => 2,
                    _ => return Err(format!("unknown align '{v}'")),
                };
                self.doc.set_value(&tid, "alignment", json!(n));
            }
            if let Some(v) = get_s(a, "valign") {
                let n = match v {
                    "top" => 0,
                    "middle" | "center" => 1,
                    "bottom" => 2,
                    _ => return Err(format!("unknown valign '{v}'")),
                };
                self.doc.set_value(&tid, "position", json!(n));
            }
            if let Some(f) = get_s(a, "font") {
                if f.is_empty() {
                    if let Some(o) = self.doc.get_mut(&tid) {
                        o.remove("letter_font_id");
                    }
                } else {
                    if crate::figlet::load_font(f).is_none() {
                        return Err(format!("FIGlet font '{f}' not found (looked in {:?})", crate::figlet::font_dirs()));
                    }
                    self.doc.set_value(&tid, "letter_font_id", json!(format!("com.helftone.monodraw.figlet-font.{f}")));
                }
            }
            if let Some(p) = get_i(a, "padding") {
                for k in ["inset_top", "inset_bottom", "inset_left", "inset_right"] {
                    self.doc.set_value(&tid, k, json!(p.max(0)));
                }
            }
        }
        Ok(())
    }

    fn set_corners(&mut self, id: &str, chars: &str) {
        for (k, c) in ["tl", "tr", "bl", "br"].iter().zip(chars.chars()) {
            let v = if c == '\0' { String::new() } else { c.to_string() };
            self.doc.set_value(id, &format!("border_tixel_{k}"), json!(v));
        }
    }
}

/// Keypad attach position for a named side of `own`; `auto` picks the side facing `other`.
fn side_position(side: Option<&str>, own: Rect, other: Rect) -> std::result::Result<i64, String> {
    let side = match side.unwrap_or("auto") {
        "auto" => {
            let (cx, cy) = (own.x * 2 + own.w, own.y * 2 + own.h);
            let (ox, oy) = (other.x * 2 + other.w, other.y * 2 + other.h);
            let (dx, dy) = (ox - cx, oy - cy);
            // Cells are about twice as tall as wide, so weigh vertical distance double.
            if dx.abs() >= dy.abs() * 2 {
                if dx >= 0 { "right" } else { "left" }
            } else if dy >= 0 {
                "bottom"
            } else {
                "top"
            }
        }
        s => s,
    };
    Ok(match side {
        "top" => 12,
        "left" => 15,
        "right" => 16,
        "bottom" => 19,
        s => return Err(format!("unknown side '{s}' ({SIDE_DOC})")),
    })
}
