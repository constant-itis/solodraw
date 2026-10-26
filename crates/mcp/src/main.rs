//! `solodraw-mcp` — a Model Context Protocol server (stdio) that lets an AI build and edit
//! solodraw documents headlessly. Each tool is one command of `solodraw_core::command`.
//!
//! usage: solodraw-mcp [--autosave] [FILE.monopic]

use std::io::{BufRead, Write};

use serde_json::{json, Value};
use solodraw_core::command::{Session, COMMANDS};

const PROTOCOL: &str = "2025-06-18";

const INSTRUCTIONS: &str = "solodraw draws box-and-line text diagrams (Monodraw-compatible .monopic files). \
Coordinates are character cells: x = column, y = row, (0,0) top-left. Boxes include their border, so a box \
with w=10 has 8 columns inside. Work in small steps and call `render` after edits to check the result \
(its ruler numbers the rows and columns). Refer to shapes by the id returned when they were created, or by \
the `name` you gave them. Use `add_art` for literal character drawings. Changes stay in memory until `save`.";

fn tools() -> Value {
    Value::Array(
        COMMANDS
            .iter()
            .map(|c| json!({"name": c.name, "description": c.summary, "inputSchema": (c.schema)()}))
            .collect(),
    )
}

fn call(session: &mut Session, autosave: bool, params: &Value) -> Value {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(Value::Null);
    let edits = !matches!(name, "render" | "list_shapes" | "save" | "open" | "new");
    match session.exec(name, &args) {
        Ok(v) => {
            let mut text = match v {
                Value::String(s) => s,
                v => v.to_string(),
            };
            if autosave && edits && session.path.is_some() {
                if let Err(e) = session.exec("save", &Value::Null) {
                    text.push_str(&format!("\n(autosave failed: {e})"));
                }
            }
            json!({"content": [{"type": "text", "text": text}]})
        }
        Err(e) => json!({"content": [{"type": "text", "text": e}], "isError": true}),
    }
}

fn main() {
    let mut autosave = false;
    let mut session = Session::default();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--autosave" => autosave = true,
            "--help" | "-h" => {
                println!("usage: solodraw-mcp [--autosave] [FILE.monopic]\n\nServes MCP over stdio. FILE is opened (or created on first save).");
                return;
            }
            path => {
                session = if std::path::Path::new(path).exists() {
                    match Session::open(path) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("solodraw-mcp: {e}");
                            std::process::exit(1);
                        }
                    }
                } else {
                    let mut s = Session::default();
                    s.path = Some(path.into());
                    s
                };
            }
        }
    }

    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let r = json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": e.to_string()}});
                let _ = writeln!(out, "{r}");
                let _ = out.flush();
                continue;
            }
        };
        // Notifications (no id) get no reply.
        let Some(id) = msg.get("id").cloned() else { continue };
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL),
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "solodraw", "version": env!("CARGO_PKG_VERSION")},
                "instructions": INSTRUCTIONS,
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": tools()})),
            "tools/call" => Ok(call(&mut session, autosave, &params)),
            _ => Err(json!({"code": -32601, "message": format!("method not found: {method}")})),
        };
        let reply = match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": e}),
        };
        let _ = writeln!(out, "{reply}");
        let _ = out.flush();
    }
}
