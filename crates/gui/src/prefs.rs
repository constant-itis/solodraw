//! Persistent preferences (`~/.config/solodraw/prefs.json`). Keys follow Monodraw's
//! `monodraw.prefs.*` names without the prefix (docs/UI_INVENTORY.md §5.3).
//! The file is read once and cached; [`set`] writes through.

use serde_json::{Map, Value};

use crate::prelude::*;

fn path() -> PathBuf {
    glib::user_config_dir().join("solodraw").join("prefs.json")
}

thread_local! {
    static CACHE: RefCell<Option<Map<String, Value>>> = const { RefCell::new(None) };
}

fn with<T>(f: impl FnOnce(&Map<String, Value>) -> T) -> T {
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        let m = c.get_or_insert_with(|| {
            std::fs::read_to_string(path())
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .and_then(|v| v.as_object().cloned())
                .unwrap_or_default()
        });
        f(m)
    })
}

pub fn get_bool(key: &str, default: bool) -> bool {
    with(|m| m.get(key).and_then(Value::as_bool).unwrap_or(default))
}

pub fn get_int(key: &str, default: i64) -> i64 {
    with(|m| m.get(key).and_then(Value::as_i64).unwrap_or(default))
}

pub fn get_u64(key: &str, default: u64) -> u64 {
    with(|m| m.get(key).and_then(Value::as_u64).unwrap_or(default))
}

pub fn get_str(key: &str, default: &str) -> String {
    with(|m| m.get(key).and_then(Value::as_str).unwrap_or(default).to_string())
}

pub fn get_value(key: &str) -> Option<Value> {
    with(|m| m.get(key).cloned())
}

pub fn set(key: &str, v: Value) {
    let mut m = with(|m| m.clone());
    m.insert(key.to_string(), v);
    let p = path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, serde_json::to_string_pretty(&Value::Object(m.clone())).unwrap_or_default());
    CACHE.with(|c| *c.borrow_mut() = Some(m));
}

pub const WELCOME_AT_LAUNCH: &str = "welcome.show-at-launch";

// Text
pub const FONT_NAME: &str = "canvas.text.font-name";
pub const ANTI_ALIAS: &str = "canvas.text.anti-alias";
pub const SUBPIXEL_ANTI_ALIAS: &str = "canvas.text.subpixel-anti-alias";
/// 1 Unicode, 2 ASCII (new documents).
pub const ALPHABET: &str = "document.alphabet";
// Clipboard
pub const CLIP_COMMENT: &str = "pasteboard.comment-spec.style";
pub const CLIP_PADDING: &str = "pasteboard.comment-spec.padding";
pub const CLIP_TRIM_TRAILING: &str = "pasteboard.whitespace.trim-trailing";
// Canvas (defaults for new documents)
pub const VGRID: &str = "canvas.grid.vertical-lines";
pub const HGRID: &str = "canvas.grid.horizontal-lines";
pub const PAGE_GUIDE: &str = "canvas.grid.page-guide.enabled";
pub const PAGE_GUIDE_COLUMN: &str = "canvas.grid.page-guide.column";
pub const LINE_NUMBERS: &str = "canvas.grid.ruler.line-numbers";
pub const COLUMN_NUMBERS: &str = "canvas.grid.ruler.line-columns";
pub const GUIDES: &str = "canvas.guide.movement";
// Interface
/// Snippets tab source filter: 0 All, 1 User, 2 Standard.
pub const DEFAULT_SNIPPET_LIST: &str = "document.snippet-list";
/// 0 System, 1 Light, 2 Dark.
pub const APPEARANCE: &str = "ui.appearance";
/// Opt-in Table tool (Monodraw's `feature-flags.table-shape`, Window ▸ Developer ▸ Table Shape).
pub const TABLE_SHAPE: &str = "feature-flags.table-shape";
/// "Animate sidebar": shape-tree rows animate on incremental changes.
pub const SIDEBAR_ANIMATED: &str = "shape-tree.animated";
/// "Animate assistant": switching assistant tabs slides (0.2 s, linear).
pub const ASSISTANT_ANIMATED: &str = "assistant.animated";
/// "Apply selection shape style on create" (off by default).
pub const APPLY_STYLE_ON_CREATE: &str = "canvas.create.shape-style.apply";
/// "Use dark icon in dark mode" (on by default).
pub const DARK_ICON: &str = "ui.use-dark-icon-in-dark-mode";
pub const CHARS_FOR_PENCIL: &str = "assistant.char-palette.auto-select-for-pencil";
// Export
pub const EXPORT_PANE: &str = "export.format-pane";
pub const EXPORT_COMMENT: &str = "export.comment-spec.style";
pub const EXPORT_PADDING: &str = "export.comment-spec.padding";
pub const EXPORT_TRIM_PADDING: &str = "export.whitespace.trim-padding";
pub const EXPORT_TRIM_TRAILING: &str = "export.whitespace.trim-trailing";
pub const EXPORT_FOOTER: &str = "export.watermark.enabled";
pub const EXPORT_WRAP: &str = "export.html-wrap.enabled";
pub const EXPORT_WRAP_TAG: &str = "export.html-wrap.tag";
pub const PNG_FOOTER: &str = "export.watermark.png.enabled";
pub const PNG_TRIM: &str = "export.png.whitespace.trim-padding";
pub const PNG_VGRID: &str = "export.png.grid.vertical-lines";
pub const PNG_HGRID: &str = "export.png.grid.horizontal-lines";
pub const PNG_TEXT: &str = "export.png.color.text";
pub const PNG_BACKGROUND: &str = "export.png.color.background";
pub const PNG_BACKGROUND_ON: &str = "export.png.background.enabled";
pub const PNG_GRID: &str = "export.png.color.grid";
pub const PNG_SCALE: &str = "export.png.scale";
pub const SVG_FOOTER: &str = "export.watermark.svg.enabled";
pub const SVG_TRIM: &str = "export.svg.whitespace.trim-padding";
pub const SVG_VGRID: &str = "export.svg.grid.vertical-lines";
pub const SVG_HGRID: &str = "export.svg.grid.horizontal-lines";
pub const SVG_TEXT: &str = "export.svg.color.text";
pub const SVG_GRID: &str = "export.svg.color.grid";

pub const DEFAULT_FONT: &str = "Monospace 11";

/// Canvas font family from the font preference (size comes from the zoom level).
pub fn font_family() -> String {
    let d = pango::FontDescription::from_string(&get_str(FONT_NAME, DEFAULT_FONT));
    d.family().map(|f| f.to_string()).unwrap_or_else(|| "Monospace".into())
}

thread_local! {
    /// The desktop's own dark preference, captured before we first change it.
    static SYSTEM_DARK: std::cell::OnceCell<bool> = const { std::cell::OnceCell::new() };
}

/// Apply the Appearance preference to GTK.
pub fn apply_appearance() {
    let Some(s) = gtk::Settings::default() else { return };
    let system = SYSTEM_DARK.with(|c| *c.get_or_init(|| s.is_gtk_application_prefer_dark_theme()));
    s.set_gtk_application_prefer_dark_theme(match get_int(APPEARANCE, 0) {
        1 => false,
        2 => true,
        _ => system,
    });
}

/// Text antialiasing for the canvas from the anti-alias preferences.
pub fn font_options() -> gtk::cairo::FontOptions {
    let mut o = gtk::cairo::FontOptions::new().expect("font options");
    o.set_antialias(if !get_bool(ANTI_ALIAS, true) {
        gtk::cairo::Antialias::None
    } else if get_bool(SUBPIXEL_ANTI_ALIAS, false) {
        gtk::cairo::Antialias::Subpixel
    } else {
        gtk::cairo::Antialias::Gray
    });
    o
}
