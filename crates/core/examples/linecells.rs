//! Debug aid: the rasterized cells of every line whose frame matches LINEFRAME (substring).
use solodraw_core::format::{Document, ObjExt};
use solodraw_core::scene::Scene;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let doc = Document::open(&a[1]).unwrap();
    let canvas = doc.canvases()[0].clone();
    let scene = Scene::new(&doc, &canvas, true);
    let pat = std::env::var("LINEFRAME").unwrap_or_default();
    for id in scene.draw_order() {
        let o = scene.obj(&id).unwrap();
        if o.type_id() != 5 || !format!("{:?}", scene.frame(&id)).contains(&pat) {
            continue;
        }
        let Some(p) = scene.line_path(&id) else { continue };
        println!("{} line_type={} corners={:?}", &id[..8], o.int_or("line_type", 3), p.corners);
        for e in o.refs("segment_end_refs") {
            println!("  end {}", serde_json::to_string(scene.obj(e).unwrap()).unwrap());
        }
        let cells: Vec<String> = p.cells.iter().enumerate().map(|(i, c)| format!("{i}:{},{}", c.x, c.y)).collect();
        println!("  {}", cells.join(" "));
    }
}
