use solodraw_core::figlet::render_in_rect;
use solodraw_core::geom::Rect;
use solodraw_core::text::TextStyle;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let text = a[2].replace("\\n", "\n");
    let h: i32 = a[3].parse().unwrap();
    let pos: i64 = a.get(4).map(|s| s.parse().unwrap()).unwrap_or(1);
    let st = TextStyle { sweep: 0, movement: 0, alignment: 1, position: pos };
    let cells = render_in_rect(&a[1], &text, Rect::new(0, 0, 20, h), st).unwrap();
    for y in 0..h {
        let row: String = (0..20).map(|x| cells.iter().find(|(p, _)| p.x == x && p.y == y).map(|c| c.1).unwrap_or('.')).collect();
        println!("{row}");
    }
}
