//! Imports shared by every GUI module.

pub use std::cell::RefCell;
pub use std::path::PathBuf;
pub use std::rc::Rc;

pub use gtk::prelude::*;
pub use gtk::{gdk, gdk_pixbuf, gio, glib, pango};
pub use solodraw_core::edit::LineEnd;
pub use solodraw_core::export::TextOptions;
pub use solodraw_core::format::NO_COLOR;
pub use solodraw_core::geom::{Point, Rect};
pub use solodraw_core::tixel::{glyph_segments, SEG_D, SEG_DL, SEG_DR, SEG_L, SEG_R, SEG_U, SEG_UL, SEG_UR, STYLE_DOUBLE, STYLE_HEAVY};
pub use solodraw_core::{canvas_to_text, Document, ObjExt};
pub use serde_json::json;

pub use crate::actions::delete_selection;
pub use crate::canvas::*;
pub use crate::files::*;
pub use crate::input::*;
pub use crate::inspector::*;
pub use crate::state::{App, Drag, Tool};
pub use crate::sidebar::*;
pub use crate::textedit::*;
pub use crate::ui::*;
pub use crate::window::*;

pub type St = Rc<RefCell<App>>;
