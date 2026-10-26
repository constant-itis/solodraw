//! Debug aid: every object in draw order with its type, frame, and how many cells it wrote.
use solodraw_core::format::{Document, ObjExt};
use solodraw_core::render::draw_shape;
use solodraw_core::scene::Scene;
use solodraw_core::tixel::Buffer;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let doc = Document::open(&a[1]).unwrap();
    let canvas = doc.canvases()[0].clone();
    let scene = Scene::new(&doc, &canvas, true);
    for id in scene.draw_order() {
        let o = scene.obj(&id).unwrap();
        let mut b = Buffer::new();
        draw_shape(&scene, &mut b, &id);
        let n = b.cells.values().filter(|c| !c.t.is_null()).count();
        if std::env::var("DUMPFRAME").is_ok_and(|p| format!("{:?}", scene.frame(&id)).contains(&p)) {
            println!("{}", serde_json::to_string(o).unwrap());
            for k in ["model_text", "content_tixels", "border_tixels"] {
                if let Some(t) = o.str_(k).and_then(|t| scene.obj(t)) {
                    println!("  {k}: {}", serde_json::to_string(t).unwrap());
                }
            }
        }
        println!("{} type={} st={} hidden={} frame={:?} cells={} content={:?}", &id[..8], o.type_id(), o.int_or("shape_type", -1), o.boolean("hidden"), scene.frame(&id), n, b.content_box());
    }
    let mut types = std::collections::BTreeMap::new();
    for (_, o) in doc.objects.iter() { if let Some(o) = o.as_object() { *types.entry(o.type_id()).or_insert(0) += 1; } }
    println!("all types: {types:?}");
}
