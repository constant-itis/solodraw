//! Command socket: each window listens on `$XDG_RUNTIME_DIR/solodraw/<pid>-<n>.sock` so other
//! programs (`solodraw-mcp`'s `attach`) can run `solodraw_core::command` commands on the open
//! document. Edits land as ordinary undoable steps and show up immediately.
//!
//! Protocol: one JSON object per line, `{"cmd": "...", "args": {...}}`; each gets one reply line,
//! `{"ok": true, "result": ...}` or `{"ok": false, "error": "..."}`.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::rc::Weak;
use std::sync::mpsc;

use serde_json::Value;
use solodraw_core::command::{is_read_only, Session};

use crate::prelude::*;

thread_local! {
    static WINDOWS: RefCell<HashMap<u32, (Weak<Ui>, Weak<RefCell<App>>)>> = RefCell::new(HashMap::new());
}

pub fn socket_dir() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("solodraw")
}

/// Start listening for `ui`'s window. The socket is removed when the window closes.
pub fn listen(ui: &U, st: &St) {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
    let key = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = socket_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join(format!("{}-{key}.sock", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("solodraw: no command socket at {}: {e}", path.display());
            return;
        }
    };
    WINDOWS.with(|w| w.borrow_mut().insert(key, (Rc::downgrade(ui), Rc::downgrade(st))));
    ui.window.connect_destroy(move |_| {
        WINDOWS.with(|w| w.borrow_mut().remove(&key));
    });
    let p = path.clone();
    ui.window.connect_close_request(move |_| {
        let _ = std::fs::remove_file(&p);
        glib::Propagation::Proceed
    });
    std::thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            std::thread::spawn(move || serve(conn, key));
        }
    });
}

fn serve(conn: UnixStream, key: u32) {
    let Ok(mut out) = conn.try_clone() else { return };
    for line in BufReader::new(conn).lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let (tx, rx) = mpsc::channel();
        glib::MainContext::default().invoke(move || {
            let _ = tx.send(handle(key, &line).to_string());
        });
        let reply = rx.recv().unwrap_or_else(|_| json!({"ok": false, "error": "window closed"}).to_string());
        if writeln!(out, "{reply}").is_err() {
            break;
        }
    }
}

/// Runs on the GTK main thread.
fn handle(key: u32, line: &str) -> Value {
    let found = WINDOWS.with(|w| w.borrow().get(&key).and_then(|(u, s)| Some((u.upgrade()?, s.upgrade()?))));
    let Some((ui, st)) = found else { return json!({"ok": false, "error": "window closed"}) };
    let req: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return json!({"ok": false, "error": format!("bad request: {e}")}),
    };
    let cmd = req.get("cmd").and_then(Value::as_str).unwrap_or("");
    let args = req.get("args").cloned().unwrap_or(Value::Null);
    match run(&ui, &st, cmd, &args) {
        Ok(v) => json!({"ok": true, "result": v}),
        Err(e) => json!({"ok": false, "error": e}),
    }
}

fn run(ui: &U, st: &St, cmd: &str, args: &Value) -> Result<Value, String> {
    // Don't change the document under an in-progress drag or text edit.
    let busy = {
        let s = st.borrow();
        s.drag.is_some() || s.edit_shape.is_some()
    };
    match cmd {
        "info" => {
            let s = st.borrow();
            return Ok(json!({
                "path": s.path, "title": ui.window.title().map(|t| t.to_string()),
                "canvas": s.canvas, "selection": s.selection, "dirty": s.dirty,
            }));
        }
        "new" | "open" => return Err(format!("'{cmd}' is not available while attached to an editor window; detach first")),
        "undo" | "redo" => {
            if busy {
                return Err("the user is editing; try again in a moment".into());
            }
            {
                let mut s = st.borrow_mut();
                let before = s.undo.len();
                if cmd == "undo" { s.undo() } else { s.redo() }
                if s.undo.len() == before {
                    return Err(format!("nothing to {cmd}"));
                }
            }
            sync(ui, st);
            return Ok(json!({"ok": true}));
        }
        "save" => {
            if let Some(p) = args.get("path").and_then(Value::as_str) {
                st.borrow_mut().path = Some(PathBuf::from(p));
            }
            if st.borrow().path.is_none() {
                return Err("the document has no file yet: pass 'path'".into());
            }
            save(ui, st, false);
            let s = st.borrow();
            return if s.dirty { Err("save failed (see the editor window)".into()) } else { Ok(json!({"saved": s.path})) };
        }
        _ => {}
    }
    if !is_read_only(cmd) && busy {
        return Err("the user is editing; try again in a moment".into());
    }
    let (doc, canvas) = {
        let s = st.borrow();
        (s.doc.clone(), s.canvas.clone())
    };
    let mut session = Session::from_doc(doc, canvas);
    let result = session.exec(cmd, args)?;
    if !is_read_only(cmd) {
        {
            let mut s = st.borrow_mut();
            s.checkpoint();
            s.doc = session.doc;
            let doc = &s.doc;
            let keep: Vec<String> = s.selection.iter().filter(|id| doc.get(id).is_some()).cloned().collect();
            s.selection = keep;
            s.rerender();
        }
        sync(ui, st);
    }
    Ok(result)
}
