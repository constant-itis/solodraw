use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

/// Run the server over `requests` (one JSON-RPC message each) and return its replies.
fn session(runtime_dir: &std::path::Path, requests: &[Value]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_solodraw-mcp"))
        .env("XDG_RUNTIME_DIR", runtime_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for r in requests {
        writeln!(stdin, "{r}").unwrap();
    }
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    String::from_utf8(out.stdout).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

fn call(id: i64, name: &str, args: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": args}})
}

fn text(reply: &Value) -> &str {
    reply["result"]["content"][0]["text"].as_str().unwrap()
}

fn short_dir(tag: &str) -> std::path::PathBuf {
    // Unix socket paths are limited to ~108 bytes, so stay out of long temp paths.
    let d = std::path::PathBuf::from(format!("/tmp/sdmcp-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn initialize_list_and_draw() {
    let dir = short_dir("a");
    let r = session(
        &dir,
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            call(3, "add_box", json!({"x": 0, "y": 0, "w": 6, "h": 3, "text": "hi"})),
            call(4, "render", json!({"ruler": false})),
            call(5, "connect", json!({"from": "nope", "to": "nope"})),
        ],
    );
    assert_eq!(r.len(), 5, "the notification gets no reply");
    assert_eq!(r[0]["result"]["serverInfo"]["name"], "solodraw");
    let names: Vec<&str> = r[1]["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    for n in ["render", "add_box", "connect", "attach", "detach", "windows"] {
        assert!(names.contains(&n), "{n} missing from tools/list");
    }
    assert_eq!(text(&r[3]), "┌────┐\n│ hi │\n└────┘");
    assert_eq!(r[4]["result"]["isError"], true);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn attach_forwards_to_an_editor_socket() {
    let dir = short_dir("b");
    std::fs::create_dir_all(dir.join("solodraw")).unwrap();
    let sock = dir.join("solodraw/1-1.sock");
    let listener = UnixListener::bind(&sock).unwrap();
    // A stand-in editor: answers `info`, echoes every other command.
    std::thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let mut out = conn.try_clone().unwrap();
                for line in BufReader::new(conn).lines().map_while(Result::ok) {
                    let req: Value = serde_json::from_str(&line).unwrap();
                    let reply = match req["cmd"].as_str().unwrap() {
                        "info" => json!({"ok": true, "result": {"path": "/x.monopic"}}),
                        "delete" => json!({"ok": false, "error": "refused"}),
                        cmd => json!({"ok": true, "result": {"echo": cmd, "args": req["args"]}}),
                    };
                    writeln!(out, "{reply}").unwrap();
                }
            });
        }
    });
    let r = session(
        &dir,
        &[
            call(1, "windows", json!({})),
            call(2, "attach", json!({})),
            call(3, "add_box", json!({"x": 1, "y": 2, "w": 3, "h": 4})),
            call(4, "delete", json!({"ids": ["a"]})),
            call(5, "detach", json!({})),
            call(6, "render", json!({"ruler": false})),
        ],
    );
    let windows: Value = serde_json::from_str(text(&r[0])).unwrap();
    assert_eq!(windows[0]["path"], "/x.monopic");
    assert!(text(&r[1]).contains("1-1.sock"));
    let echoed: Value = serde_json::from_str(text(&r[2])).unwrap();
    assert_eq!(echoed, json!({"echo": "add_box", "args": {"x": 1, "y": 2, "w": 3, "h": 4}}));
    assert_eq!(text(&r[3]), "refused");
    assert_eq!(r[3]["result"]["isError"], true);
    assert_eq!(text(&r[5]), "", "after detach, commands go to the (empty) headless document");
    std::fs::remove_dir_all(dir).ok();
}
