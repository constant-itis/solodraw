use serde_json::{json, Value};
use solodraw_core::command::{Session, COMMANDS};

fn run(s: &mut Session, name: &str, args: Value) -> Value {
    s.exec(name, &args).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn plain(s: &mut Session) -> String {
    run(s, "render", json!({"ruler": false})).as_str().unwrap().to_string()
}

#[test]
fn builds_a_card_and_connects_by_name() {
    let mut s = Session::default();
    run(&mut s, "add_box", json!({"x": 0, "y": 0, "w": 12, "h": 5, "border": "heavy", "corners": "BWH$", "name": "card"}));
    run(&mut s, "add_box", json!({"x": 20, "y": 1, "w": 8, "h": 3, "text": "Deck", "border": "double", "name": "deck"}));
    run(&mut s, "connect", json!({"from": "card", "to": "deck"}));
    assert_eq!(
        plain(&mut s),
        "B━━━━━━━━━━W\n\
         ┃          ┃        ╔══════╗\n\
         ┃          ┃───────▶║ Deck ║\n\
         ┃          ┃        ╚══════╝\n\
         H━━━━━━━━━━$"
    );
}

#[test]
fn edits_are_undoable_and_failures_change_nothing() {
    let mut s = Session::default();
    run(&mut s, "add_box", json!({"x": 0, "y": 0, "w": 3, "h": 3, "name": "a"}));
    let before = plain(&mut s);
    assert!(s.exec("move", &json!({"ids": ["a", "missing"], "dx": 5, "dy": 0})).is_err());
    assert_eq!(plain(&mut s), before, "a failed command must not half-apply");
    run(&mut s, "update", json!({"id": "a", "x": 4, "w": 5}));
    assert_eq!(plain(&mut s), "    ┌───┐\n    │   │\n    └───┘");
    run(&mut s, "undo", json!({}));
    assert_eq!(plain(&mut s), before);
    run(&mut s, "redo", json!({}));
    assert!(plain(&mut s).starts_with("    ┌"));
}

#[test]
fn art_ruler_and_region() {
    let mut s = Session::default();
    run(&mut s, "add_art", json!({"x": 2, "y": 1, "art": "( ^  ^)\n /|  |\\"}));
    let r = run(&mut s, "render", json!({"region": [2, 1, 7, 2]}));
    assert_eq!(r.as_str().unwrap(), "   0\n   2345678\n 1|( ^  ^)\n 2| /|  |\\");
}

#[test]
fn group_moves_together_and_saves() {
    let dir = std::env::temp_dir().join(format!("solodraw-cmd-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("g.monopic");
    let mut s = Session::default();
    let a = run(&mut s, "add_box", json!({"x": 0, "y": 0, "w": 3, "h": 3}))["id"].as_str().unwrap().to_string();
    run(&mut s, "add_text", json!({"x": 4, "y": 1, "text": "hi", "name": "t"}));
    run(&mut s, "group", json!({"ids": [a, "t"], "name": "g"}));
    run(&mut s, "move", json!({"ids": ["g"], "dx": 1, "dy": 1}));
    let shown = plain(&mut s);
    run(&mut s, "save", json!({"path": path.to_str().unwrap()}));
    let mut t = Session::open(&path).unwrap();
    assert_eq!(plain(&mut t), shown);
    let listed = run(&mut t, "list_shapes", json!({}));
    assert_eq!(listed["shapes"][0]["name"], "g");
    assert_eq!(listed["shapes"][0]["children"].as_array().unwrap().len(), 2);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn every_command_has_an_object_schema() {
    for c in COMMANDS {
        assert_eq!((c.schema)()["type"], "object", "{}", c.name);
    }
}
