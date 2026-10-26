//! `solodraw-mcp` — a Model Context Protocol server (stdio) that lets an AI build and edit
//! solodraw documents headlessly. Each tool is one command of `solodraw_core::command`.
//!
//! usage: solodraw-mcp [--autosave] [FILE.monopic]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use serde_json::{json, Value};
use solodraw_core::command::{Session, COMMANDS};

const PROTOCOL: &str = "2025-06-18";

const INSTRUCTIONS: &str = "solodraw draws box-and-line text diagrams (Monodraw-compatible .monopic files). \
Coordinates are character cells: x = column, y = row, (0,0) top-left. Boxes include their border, so a box \
with w=10 has 8 columns inside. Work in small steps and call `render` after edits to check the result \
(its ruler numbers the rows and columns). Refer to shapes by the id returned when they were created, or by \
the `name` you gave them. Use `add_art` for literal character drawings. Changes stay in memory until `save`. \
To draw in the user's open solodraw editor window instead (they see each change live and can undo it), call \
`attach`; `detach` goes back to the headless document.";

/// Connection to an editor window's command socket.
struct Remote {
    path: PathBuf,
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

impl Remote {
    fn connect(path: PathBuf) -> std::io::Result<Remote> {
        let writer = UnixStream::connect(&path)?;
        writer.set_read_timeout(Some(std::time::Duration::from_secs(30)))?;
        let reader = BufReader::new(writer.try_clone()?);
        Ok(Remote { path, reader, writer })
    }

    fn call(&mut self, cmd: &str, args: &Value) -> Result<Value, String> {
        let req = json!({"cmd": cmd, "args": args});
        writeln!(self.writer, "{req}").map_err(|e| format!("editor connection lost: {e}"))?;
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) | Err(_) => return Err("editor connection lost (window closed?)".into()),
            Ok(_) => {}
        }
        let r: Value = serde_json::from_str(&line).map_err(|e| format!("bad reply from editor: {e}"))?;
        if r["ok"] == true {
            Ok(r["result"].clone())
        } else {
            Err(r["error"].as_str().unwrap_or("editor error").to_string())
        }
    }
}

fn socket_dir() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("solodraw")
}

/// Live editor windows, newest first: (socket, info).
fn windows() -> Vec<(PathBuf, Value)> {
    let mut socks: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(socket_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "sock"))
        .map(|e| (e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH), e.path()))
        .collect();
    socks.sort_by(|a, b| b.0.cmp(&a.0));
    let mut out = Vec::new();
    for (_, p) in socks {
        match Remote::connect(p.clone()).and_then(|mut r| {
            r.writer.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
            Ok(r.call("info", &Value::Null))
        }) {
            Ok(Ok(info)) => out.push((p, info)),
            // A socket nobody listens on is left over from a crashed editor.
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                let _ = std::fs::remove_file(&p);
            }
            _ => {}
        }
    }
    out
}

struct Server {
    session: Session,
    autosave: bool,
    remote: Option<Remote>,
}


fn tools() -> Value {
    let mut v: Vec<Value> =
        COMMANDS.iter().map(|c| json!({"name": c.name, "description": c.summary, "inputSchema": (c.schema)()})).collect();
    let no_args = json!({"type": "object", "properties": {}, "additionalProperties": false});
    v.push(json!({"name": "windows", "description": "List the solodraw editor windows open on this machine (newest first) with their file, canvas and the user's current selection.", "inputSchema": no_args}));
    v.push(json!({"name": "attach", "description": "Send the following commands to an open editor window (default: the newest) so the user watches the drawing happen and can undo any step. `windows` lists them.",
        "inputSchema": {"type": "object", "properties": {"socket": {"type": "string", "description": "A socket path from `windows`"}}, "additionalProperties": false}}));
    v.push(json!({"name": "detach", "description": "Stop driving the editor window; commands go to the headless document again.", "inputSchema": no_args}));
    Value::Array(v)
}

impl Server {
    fn exec(&mut self, name: &str, args: &Value) -> Result<Value, String> {
        match name {
            "windows" => {
                let w = windows();
                let attached = self.remote.as_ref().map(|r| r.path.clone());
                return Ok(json!(w.into_iter().map(|(p, mut info)| {
                    info["attached"] = json!(Some(&p) == attached.as_ref());
                    info["socket"] = json!(p);
                    info
                }).collect::<Vec<_>>()));
            }
            "attach" => {
                let path = match args.get("socket").and_then(Value::as_str) {
                    Some(p) => PathBuf::from(p),
                    None => windows().into_iter().next().map(|(p, _)| p).ok_or("no solodraw editor window is open (start solodraw-gtk)")?,
                };
                let mut r = Remote::connect(path.clone()).map_err(|e| format!("could not connect to {}: {e}", path.display()))?;
                let info = r.call("info", &Value::Null)?;
                self.remote = Some(r);
                return Ok(json!({"attached": path, "window": info}));
            }
            "detach" => {
                return Ok(json!({"detached": self.remote.take().is_some()}));
            }
            _ => {}
        }
        if let Some(r) = self.remote.as_mut() {
            let res = r.call(name, args);
            if res.as_ref().is_err_and(|e| e.starts_with("editor connection lost")) {
                self.remote = None;
                return res.map_err(|e| format!("{e}; detached, commands now go to the headless document"));
            }
            return res;
        }
        let res = self.session.exec(name, args)?;
        let edits = !matches!(name, "render" | "list_shapes" | "save" | "open" | "new");
        if self.autosave && edits && self.session.path.is_some() {
            if let Err(e) = self.session.exec("save", &Value::Null) {
                return Ok(json!({"result": res, "autosave_error": e}));
            }
        }
        Ok(res)
    }
}

fn call(server: &mut Server, params: &Value) -> Value {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(Value::Null);
    match server.exec(name, &args) {
        Ok(v) => {
            let text = match v {
                Value::String(s) => s,
                v => v.to_string(),
            };
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
                println!("usage: solodraw-mcp [--autosave] [FILE.monopic]\n\nServes MCP over stdio. FILE is opened (or created on first save).\nThe `attach` tool drives an open solodraw-gtk window instead.");
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

    let mut server = Server { session, autosave, remote: None };
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
            "tools/call" => Ok(call(&mut server, &params)),
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
