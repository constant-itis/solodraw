//! Clean-room implementation of the Monodraw document model, renderer and text export,
//! written from `SPEC.md`.

pub mod b64;
pub mod command;
pub mod edit;
pub mod ellipse;
pub mod export;
pub mod figlet;
pub mod fragment;
pub mod format;
pub mod geom;
mod junction_table;
pub mod render;
pub mod route;
pub mod scene;
pub mod table;
pub mod text;
pub mod tixel;

pub use export::{canvas_to_text, TextOptions};
pub use format::{Document, Error, Obj, ObjExt};
pub use scene::Scene;
