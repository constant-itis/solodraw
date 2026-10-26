//! Debug aid: every shape whose frame hangs off an anchor position, with the anchor, the
//! shape that owns the anchor, and how we resolve it.
use solodraw_core::format::{Document, ObjExt};
use solodraw_core::scene::Scene;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let doc = Document::open(&a[1]).unwrap();
    let canvas = doc.canvases()[0].clone();
    let scene = Scene::new(&doc, &canvas, true);
    let mut owner = std::collections::HashMap::new();
    for (id, v) in &doc.objects {
        if let Some(o) = v.as_object() {
            for p in o.refs("position_refs") {
                owner.entry(p.to_string()).or_insert_with(|| id.clone());
            }
        }
    }
    let name = |id: &str| scene.obj(id).and_then(|o| o.str_("name")).unwrap_or("").chars().take(24).collect::<String>().replace('\n', " ");
    for id in scene.draw_order() {
        let o = scene.obj(&id).unwrap();
        let Some(anc) = o.str_("frame_anchor_ref") else { continue };
        let ao = scene.obj(anc).unwrap();
        let w = owner.get(anc).map(String::as_str).unwrap_or("-");
        let wo = scene.obj(w);
        println!(
            "{} {:?} size={} corner={} coff={} origin={} foff={} frame={:?}",
            &id[..8],
            name(&id),
            o.str_("frame_size").unwrap_or(""),
            o.int_or("frame_corner", 1),
            o.str_("content_offset").unwrap_or(""),
            o.str_("origin").unwrap_or(""),
            o.str_("frame_offset").unwrap_or(""),
            scene.frame(&id)
        );
        let mut aj = serde_json::to_value(ao).unwrap();
        if let Some(m) = aj.as_object_mut() {
            m.remove("doc_ref");
            m.remove("object_id");
        }
        println!("   anchor t{} {} -> {:?}", ao.type_id(), aj, scene.resolve(anc).map(|r| r.p));
        if let Some(wo) = wo {
            println!(
                "   owner {} {:?} t{} size={} corner={} coff={} frame={:?} content_origin={:?}",
                &w[..8.min(w.len())],
                name(w),
                wo.type_id(),
                wo.str_("frame_size").unwrap_or(""),
                wo.int_or("frame_corner", 1),
                wo.str_("content_offset").unwrap_or(""),
                scene.frame(w),
                scene.content_origin(w)
            );
        }
    }
}
