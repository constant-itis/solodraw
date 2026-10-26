//! `solodraw` — prints the rendered canvas of a .monopic file (SPEC §8.4).

use std::process::ExitCode;

use solodraw_core::{canvas_to_text, Document, TextOptions};

const USAGE: &str = "usage: solodraw [OPTIONS] file

Options:
  --help               Show this help
  --version            Show the version
  --unicode            Use Unicode as the alphabet (--no-unicode forces ASCII;
                       default: the document's alphabet)
  --trim-whitespace    Trims trailing whitespace for each line
  --trim-padding       Trim blank rows/columns around the drawing (default on,
                       as in Monodraw's CLI)
  --skip-newline       Do not print the trailing newline character at the end
  --json               Print structured output in JSON format

Every option can be negated with --no-<option>.";

fn main() -> ExitCode {
    let mut unicode: Option<bool> = None;
    let mut trim = false;
    let mut trim_padding = true;
    let mut skip_newline = false;
    let mut json = false;
    let mut file: Option<String> = None;
    for arg in std::env::args().skip(1) {
        let (neg, name) = match arg.strip_prefix("--no-") {
            Some(n) => (true, n.to_string()),
            None => (false, arg.trim_start_matches("--").to_string()),
        };
        if !arg.starts_with("--") {
            if file.is_some() {
                eprintln!("error: only a single file argument is supported");
                return ExitCode::from(2);
            }
            file = Some(arg);
            continue;
        }
        match name.as_str() {
            "help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "version" => {
                println!("solodraw {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            "unicode" => unicode = Some(!neg),
            "trim-whitespace" => trim = !neg,
            "trim-padding" => trim_padding = !neg,
            "skip-newline" => skip_newline = !neg,
            "json" => json = !neg,
            _ => {
                eprintln!("error: unknown option '{arg}'\n\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(file) = file else {
        eprintln!("error: you need to provide a filename\n\n{USAGE}");
        return ExitCode::from(2);
    };
    let doc = match Document::open(&file) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: could not read file '{file}': {e}");
            return ExitCode::FAILURE;
        }
    };
    let opts = TextOptions { anchor_origin: true, trim_padding, trim_trailing: trim, unicode };
    let out = canvas_to_text(&doc, None, &opts);
    let mut text = match (&out, json) {
        (Some(o), true) => serde_json::json!({ "output": o }).to_string(),
        (Some(o), false) => o.clone(),
        // Monodraw's CLI prints a nil string for an empty drawing, with no newline.
        (None, false) => "(null)".to_string(),
        (None, true) => serde_json::json!({ "output": null }).to_string(),
    };
    if !skip_newline && !(out.is_none() && !json) {
        text.push('\n');
    }
    // A closed pipe (e.g. `| head`) is not an error.
    use std::io::Write;
    let _ = std::io::stdout().lock().write_all(text.as_bytes());
    ExitCode::SUCCESS
}
