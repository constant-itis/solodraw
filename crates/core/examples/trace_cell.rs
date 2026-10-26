//! Debug aid: which shapes write a given output cell (coords as in --no-trim-padding output).
use solodraw_core::export::{export_rect, TextOptions};
use solodraw_core::format::{Document, ObjExt};
use solodraw_core::geom::Point;
use solodraw_core::render::draw_shape;
use solodraw_core::scene::Scene;
use solodraw_core::tixel::Buffer;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let doc = Document::open(&a[1]).unwrap();
    let (cx, cy): (i32, i32) = (a[2].parse().unwrap(), a[3].parse().unwrap());
    let unicode = a.get(4).is_some_and(|s| s == "u");
    let canvas = doc.canvases()[0].clone();
    let scene = Scene::new(&doc, &canvas, unicode);
    let mut buf = Buffer::new();
    for id in scene.draw_order() {
        draw_shape(&scene, &mut buf, &id);
    }
    let r = export_rect(&buf, &TextOptions { trim_padding: false, ..Default::default() }).unwrap();
    let p = Point::new(r.x + cx, r.y + cy);
    println!("canvas point {:?}", p);
    let mut buf = Buffer::new();
    for id in scene.draw_order() {
        let before = buf.get(p).copied();
        draw_shape(&scene, &mut buf, &id);
        let after = buf.get(p).copied();
        if before != after {
            let o = scene.obj(&id).unwrap();
            println!("{id} type={} frame={:?} -> {:?}", o.type_id(), scene.frame(&id), after);
            if std::env::var("DUMP").is_ok() {
                println!("  {}", serde_json::to_string(o).unwrap());
                if let Some(a) = o.str_("frame_anchor_ref") {
                    if let Some(ao) = scene.obj(a) {
                        println!("  anchor: {} -> {:?}", serde_json::to_string(ao).unwrap(), scene.resolve(a).map(|r| (r.p, r.rect)));
                    }
                }
                if let Some(t) = o.str_("model_text").and_then(|t| scene.obj(t)) {
                    println!("  text: {}", serde_json::to_string(t).unwrap());
                }
            }
        }
    }
}
