//! `.monopic` / `.monojson` persistence (SPEC §1).
//!
//! The document is kept as the raw JSON object graph so that unknown keys survive a
//! load/save round trip. Typed access goes through [`ObjExt`], which falls back to the
//! per-type default table when a key is absent (SPEC §1.6).

use std::collections::HashMap;
use std::io::{Read, Write};

use serde_json::{json, Map, Value};

use crate::geom::{parse_point, Point};

pub const MAGIC: &[u8; 9] = b"\xffMONOPIC\x01";
pub const MODEL_VERSION: i64 = 5;
pub const NO_COLOR: u64 = 0x1_0000_0000;

pub type Obj = Map<String, Value>;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Json(serde_json::Error),
    NotMonodraw,
    NewerVersion(i64),
    Format(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Json(e) => write!(f, "invalid JSON: {e}"),
            Error::NotMonodraw => write!(f, "not a Monodraw document"),
            Error::NewerVersion(v) => {
                write!(f, "file was created by a newer version of Monodraw (model v{v})")
            }
            Error::Format(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string().to_uppercase()
}

// ---------------------------------------------------------------------------
// Default table (SPEC §1.6)

fn shape_base() -> Vec<(&'static str, Value)> {
    vec![
        ("cthrough", json!(false)),
        ("name_edited", json!(false)),
        ("origin", json!("0,0")),
        ("tag", json!(0)),
        ("hidden", json!(false)),
        ("locked", json!(false)),
        ("surface_pos_enabled", json!(true)),
        ("attachable", json!(true)),
        ("content_blended", json!(true)),
        ("content_color", json!(NO_COLOR)),
        ("shadow_enabled", json!(false)),
        ("shadow_offset", json!("0,0")),
        ("shadow_tixel", json!("")),
        ("shadow_intensity", json!(4)),
        ("shadow_color", json!(NO_COLOR)),
        ("subshape_refs", json!([])),
    ]
}

fn primitive() -> Vec<(&'static str, Value)> {
    let mut v = shape_base();
    v.extend([
        ("background_type", json!(2)),
        ("background_enabled", json!(true)),
        ("background_tixel", json!("")),
        ("rectangular_border_enabled", json!(true)),
        ("mps_bckg_color", json!(NO_COLOR)),
        ("mps_border_color", json!(NO_COLOR)),
    ]);
    v
}

fn with(mut base: Vec<(&'static str, Value)>, extra: Vec<(&'static str, Value)>) -> Map<String, Value> {
    for (k, v) in extra {
        if let Some(slot) = base.iter_mut().find(|(bk, _)| *bk == k) {
            slot.1 = v;
        } else {
            base.push((k, v));
        }
    }
    base.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

fn dash0() -> Value {
    json!({"o": 0, "p": 0, "u": 0})
}

fn build_defaults() -> HashMap<i64, Map<String, Value>> {
    let mut d = HashMap::new();
    d.insert(0, with(vec![], vec![("alphabet", json!(1))]));
    d.insert(
        1,
        with(shape_base(), vec![("attachable", json!(false)), ("position_refs", json!([])), ("shape_type", json!(6))]),
    );
    d.insert(
        3,
        with(
            shape_base(),
            vec![
                ("shape_type", json!(8)),
                ("position_refs", json!([])),
                ("attachable", json!(false)),
                ("subshape_selection", json!(false)),
            ],
        ),
    );
    d.insert(
        5,
        with(
            shape_base(),
            vec![
                ("shape_type", json!(2)),
                ("line_type", json!(3)),
                ("line_style", json!(3)),
                ("line_dash_enabled", json!(false)),
                ("line_dash", dash0()),
                ("head_notation", json!(2)),
            ],
        ),
    );
    d.insert(10, with(vec![], vec![("head_type", json!(0)), ("dirset", json!(0))]));
    d.insert(11, with(vec![], vec![("dirset", json!(0))]));
    d.insert(12, with(vec![], vec![("dirset", json!(0)), ("position_flags", json!(1))]));
    d.insert(22, with(vec![], vec![("dirset", json!(0)), ("position_flags", json!(1))]));
    d.insert(25, with(shape_base(), vec![("shape_type", json!(10))]));
    d.insert(18, with(vec![], vec![("tixels", json!({"text_content": [], "text_points": []}))]));
    let mut rect = vec![
        ("shape_type", json!(4)),
        ("rectangular_border", json!(4)),
        ("frame_offset", json!("0,0")),
        ("frame_corner", json!(1)),
        ("content_offset", json!("0,0")),
        ("border_dash", json!(false)),
        ("border_dash_enabled", dash0()),
    ];
    for k in ["border_tixel_tl", "border_tixel_tr", "border_tixel_bl", "border_tixel_br", "border_tixel_ts", "border_tixel_bs", "border_tixel_ls", "border_tixel_rs"] {
        rect.push((k, json!("")));
    }
    d.insert(20, with(primitive(), rect));
    let mut dia = vec![("shape_type", json!(9)), ("anchor_pos", json!(1))];
    for k in ["bt_tp", "bt_bp", "bt_lp", "bt_rp", "btd_ul", "btd_ur", "btd_ll", "btd_lr"] {
        dia.push((k, json!("")));
    }
    d.insert(21, with(primitive(), dia));
    d.insert(
        27,
        with(
            vec![],
            vec![
                ("enabled", json!(true)),
                ("sweep", json!(0)),
                ("movement", json!(0)),
                ("alignment", json!(1)),
                ("position", json!(1)),
                ("inside_border", json!(true)),
                ("color", json!(NO_COLOR)),
                ("inset_top", json!(0)),
                ("inset_bottom", json!(0)),
                ("inset_left", json!(0)),
                ("inset_right", json!(0)),
            ],
        ),
    );
    d.insert(
        28,
        with(
            primitive(),
            vec![("shape_type", json!(12)), ("border_tixel", json!("")), ("text_container_control", json!(0.25))],
        ),
    );
    d
}

pub fn defaults(type_id: i64) -> Option<&'static Map<String, Value>> {
    use std::sync::OnceLock;
    static D: OnceLock<HashMap<i64, Map<String, Value>>> = OnceLock::new();
    D.get_or_init(build_defaults).get(&type_id)
}

// ---------------------------------------------------------------------------
// Typed access

pub trait ObjExt {
    fn type_id(&self) -> i64;
    fn val(&self, key: &str) -> Option<&Value>;
    fn int(&self, key: &str) -> Option<i64> {
        self.val(key).and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)).or_else(|| v.as_bool().map(|b| b as i64)))
    }
    fn int_or(&self, key: &str, d: i64) -> i64 {
        self.int(key).unwrap_or(d)
    }
    fn uint(&self, key: &str) -> Option<u64> {
        self.val(key).and_then(|v| v.as_u64())
    }
    fn float(&self, key: &str) -> Option<f64> {
        self.val(key).and_then(|v| v.as_f64())
    }
    fn boolean(&self, key: &str) -> bool {
        self.val(key).map(|v| v.as_bool().unwrap_or_else(|| v.as_i64().unwrap_or(0) != 0)).unwrap_or(false)
    }
    fn str_(&self, key: &str) -> Option<&str> {
        self.val(key).and_then(|v| v.as_str())
    }
    fn point(&self, key: &str) -> Option<Point> {
        self.str_(key).and_then(parse_point)
    }
    /// A one-character tixel string; `""` (or missing) = unset.
    fn tixel_char(&self, key: &str) -> Option<char> {
        self.str_(key).and_then(|s| s.chars().next())
    }
    fn refs(&self, key: &str) -> Vec<&str> {
        match self.val(key) {
            Some(Value::Array(a)) => a.iter().filter_map(|v| v.as_str()).collect(),
            _ => vec![],
        }
    }
}

impl ObjExt for Obj {
    fn type_id(&self) -> i64 {
        self.get("type_id").and_then(|v| v.as_i64()).unwrap_or(-1)
    }
    fn val(&self, key: &str) -> Option<&Value> {
        self.get(key).or_else(|| defaults(self.type_id()).and_then(|d| d.get(key)))
    }
}

// ---------------------------------------------------------------------------
// Document

#[derive(Clone, Debug, Default)]
pub struct Document {
    /// object_id → object, in file order.
    pub objects: Map<String, Value>,
}

impl Document {
    pub fn get(&self, id: &str) -> Option<&Obj> {
        self.objects.get(id).and_then(|v| v.as_object())
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Obj> {
        self.objects.get_mut(id).and_then(|v| v.as_object_mut())
    }

    pub fn insert(&mut self, obj: Obj) -> String {
        let id = obj.get("object_id").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_else(new_id);
        let mut obj = obj;
        obj.insert("object_id".into(), json!(id));
        self.objects.insert(id.clone(), Value::Object(obj));
        id
    }

    pub fn root(&self) -> Option<(&str, &Obj)> {
        self.objects.iter().find_map(|(k, v)| {
            let o = v.as_object()?;
            (o.type_id() == 0).then_some((k.as_str(), o))
        })
    }

    pub fn canvases(&self) -> Vec<String> {
        self.root().map(|(_, r)| r.refs("canvas_refs").into_iter().map(str::to_string).collect()).unwrap_or_default()
    }

    /// true = Unicode alphabet (1), false = ASCII (2).
    pub fn unicode(&self) -> bool {
        self.root().map(|(_, r)| r.int_or("alphabet", 1) != 2).unwrap_or(true)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let json: Vec<u8> = if bytes.starts_with(MAGIC) {
            let mut out = Vec::new();
            flate2::read::GzDecoder::new(&bytes[MAGIC.len()..]).read_to_end(&mut out)?;
            out
        } else if bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{') {
            bytes.to_vec()
        } else {
            return Err(Error::NotMonodraw);
        };
        let v: Value = serde_json::from_slice(&json)?;
        Self::from_json(v)
    }

    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        Self::from_bytes(&std::fs::read(path)?)
    }

    pub fn from_json(mut doc: Value) -> Result<Self> {
        let header_v = doc.get("header").and_then(|h| h.get("v")).and_then(|v| v.as_i64());
        let mut v = header_v.ok_or_else(|| Error::Format("missing header.v".into()))?;
        if v > MODEL_VERSION {
            return Err(Error::NewerVersion(v));
        }
        let top = doc.as_object_mut().ok_or(Error::NotMonodraw)?;
        if v < 3 {
            let objs = top
                .get_mut("objects")
                .and_then(|o| o.as_object_mut())
                .ok_or_else(|| Error::Format("missing objects".into()))?;
            while v < 3 {
                match v {
                    0 => migrate_0_1(objs),
                    1 => migrate_1_2(objs),
                    _ => {}
                }
                v += 1;
            }
        }
        if v == 3 {
            migrate_3_4(top)?;
            v = 4;
        }
        let list = match top.remove("object_list") {
            Some(Value::Array(a)) => a,
            _ => return Err(Error::Format("missing object_list".into())),
        };
        let mut objects = Map::new();
        for o in list {
            let Value::Object(mut o) = o else { continue };
            if v == 5 {
                expand(&mut o);
            } else {
                migrate_4_5(&mut o);
            }
            let id = o.get("object_id").and_then(|v| v.as_str()).map(str::to_string);
            if let Some(id) = id {
                objects.insert(id, Value::Object(o));
            }
        }
        Ok(Document { objects })
    }

    pub fn to_json(&self, minify: bool) -> Value {
        let list: Vec<Value> = self
            .objects
            .values()
            .map(|o| match (minify, o.as_object()) {
                (true, Some(obj)) => Value::Object(elide(obj)),
                _ => o.clone(),
            })
            .collect();
        json!({"header": {"v": MODEL_VERSION}, "object_list": list})
    }

    /// `.monopic` bytes: magic + gzip(level 2) of compact JSON.
    pub fn to_monopic(&self, minify: bool) -> Vec<u8> {
        let body = serde_json::to_vec(&self.to_json(minify)).expect("serialize");
        let mut out = MAGIC.to_vec();
        let mut enc = flate2::write::GzEncoder::new(&mut out, flate2::Compression::new(2));
        enc.write_all(&body).expect("gzip");
        enc.finish().expect("gzip");
        out
    }

    /// `.monojson` bytes: pretty-printed JSON, no magic.
    pub fn to_monojson(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(&self.to_json(false)).expect("serialize")
    }

    pub fn save(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let path = path.as_ref();
        let bytes = if path.extension().is_some_and(|e| e == "monojson") { self.to_monojson() } else { self.to_monopic(false) };
        std::fs::write(path, bytes)?;
        Ok(())
    }
}

fn expand(o: &mut Obj) {
    if let Some(d) = defaults(o.type_id()) {
        for (k, v) in d {
            if !o.contains_key(k) {
                o.insert(k.clone(), v.clone());
            }
        }
    }
}

fn elide(o: &Obj) -> Obj {
    match defaults(o.type_id()) {
        Some(d) => o.iter().filter(|(k, v)| d.get(*k) != Some(v)).map(|(k, v)| (k.clone(), v.clone())).collect(),
        None => o.clone(),
    }
}

fn migrate_0_1(objs: &mut Map<String, Value>) {
    for o in objs.values_mut().filter_map(|v| v.as_object_mut()) {
        let t = o.type_id();
        let (new_type, shape_type) = match t {
            1 => (None, 6),
            3 => (None, 8),
            4 => (Some(20), 1),
            5 => (None, 2),
            8 => (Some(20), 5),
            14 => (Some(20), 4),
            _ => continue,
        };
        if let Some(nt) = new_type {
            o.insert("type_id".into(), json!(nt));
        }
        o.insert("shape_type".into(), json!(shape_type));
        if t == 14 {
            o.insert("text_enabled".into(), json!(true));
        }
        if o.get("rectangular_border").and_then(|v| v.as_i64()) == Some(1) {
            o.insert("rectangular_border".into(), json!(3));
        } else {
            o.insert("rectangular_border_enabled".into(), json!(true));
        }
        if let Some(bt) = o.get("background_type").and_then(|v| v.as_i64()) {
            if (5..=10).contains(&bt) {
                o.insert("background_enabled".into(), json!(true));
                o.insert("background_type".into(), json!(4));
            } else if (2..=4).contains(&bt) {
                o.insert("background_enabled".into(), json!(true));
            } else if bt == 1 {
                o.insert("background_type".into(), json!(2));
            }
        }
    }
}

const LEGACY_TEXT: [(&str, &str); 9] = [
    ("text_enabled", "enabled"),
    ("text", "text"),
    ("text_sweep", "sweep"),
    ("text_movement", "movement"),
    ("text_alignment", "alignment"),
    ("text_position", "position"),
    ("text_inside", "inside_border"),
    ("lf_id", "letter_font_id"),
    ("lf_name", "letter_font_name"),
];

fn migrate_1_2(objs: &mut Map<String, Value>) {
    let ids: Vec<String> = objs.keys().cloned().collect();
    for id in ids {
        let Some(o) = objs.get_mut(&id).and_then(|v| v.as_object_mut()) else { continue };
        if o.type_id() != 20 {
            continue;
        }
        let mut t = Obj::new();
        t.insert("type_id".into(), json!(27));
        if let Some(d) = o.get("doc_ref") {
            t.insert("doc_ref".into(), d.clone());
        }
        for (old, new) in LEGACY_TEXT {
            if let Some(v) = o.get(old) {
                t.insert(new.into(), v.clone());
            }
        }
        let nid = new_id();
        o.insert("model_text".into(), json!(nid));
        objs.insert(nid, Value::Object(t));
    }
}

pub(crate) fn rewrite(v: Value, m: &HashMap<String, String>) -> Value {
    match v {
        Value::String(s) => Value::String(m.get(&s).cloned().unwrap_or(s)),
        Value::Array(a) => Value::Array(a.into_iter().map(|x| rewrite(x, m)).collect()),
        Value::Object(o) => Value::Object(
            o.into_iter().map(|(k, x)| (m.get(&k).cloned().unwrap_or(k), rewrite(x, m))).collect(),
        ),
        other => other,
    }
}

fn migrate_3_4(top: &mut Map<String, Value>) -> Result<()> {
    let objs = top.remove("objects").ok_or_else(|| Error::Format("missing objects".into()))?;
    let idmap: HashMap<String, String> = objs
        .as_object()
        .map(|o| o.keys().map(|k| (k.clone(), new_id())).collect())
        .unwrap_or_default();
    let Value::Object(objs) = rewrite(objs, &idmap) else { return Err(Error::Format("objects".into())) };
    let mut list = Vec::with_capacity(objs.len());
    for (k, mut o) in objs {
        let Some(obj) = o.as_object_mut() else { continue };
        if obj.contains_key("object_id") {
            return Err(Error::Format("object_id already present in legacy object".into()));
        }
        obj.insert("object_id".into(), json!(k));
        list.push(o);
    }
    top.insert("object_list".into(), Value::Array(list));
    Ok(())
}

const COLOR_KEYS: [&str; 5] = ["content_color", "shadow_color", "mps_bckg_color", "mps_border_color", "color"];

fn migrate_4_5(o: &mut Obj) {
    for k in COLOR_KEYS {
        if o.get(k).and_then(|v| v.as_u64()) == Some(0xFF00_0000) {
            o.insert(k.into(), json!(NO_COLOR));
        }
    }
}
