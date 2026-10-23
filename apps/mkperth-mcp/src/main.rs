//! MK Perth as a Model Context Protocol server over stdio: an agent opens a project, reads its
//! schedule, changes it with the engine's commands, asks how a date was reached and saves it,
//! through the same `Session` the app and the CLI use. One session lives as long as the
//! connection.
//!
//! `mkperth-mcp` reads JSON-RPC messages, one per line, on stdin (a batch as a JSON array) and
//! answers on stdout. Register it with an MCP client: `claude mcp add mkperth -- mkperth-mcp`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::io::{BufRead, Write};
use std::path::Path;

use mkperth_engine::{COMMANDS, Session};
use serde_json::{Value, json};

const PROTOCOL: &str = "2025-06-18";
/// The longest line read, in bytes: a message past it is refused, not held in memory.
const MAX_LINE: usize = 16 * 1024 * 1024;

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// The tools, with their arguments as JSON Schema.
fn tools() -> Value {
    let commands = COMMANDS.join(", ");
    json!([
        {
            "name": "open_project",
            "description": "Open a project file and schedule it: Primavera P6 XML (.xml) or Perth.jl's project JSON (anything else). Replaces the open project; returns what reading it said and its schedule.",
            "inputSchema": {"type": "object", "properties": {"path": {"type": "string", "description": "Path to the file"}}, "required": ["path"]}
        },
        {
            "name": "get_schedule",
            "description": "The open project's schedule: every activity's early and late start and finish, total and free float in working days, whether it is critical, its progress status, and the project's finish. As a table, JSON or CSV.",
            "inputSchema": {"type": "object", "properties": {"format": {"type": "string", "enum": ["table", "json", "csv"], "default": "table"}}}
        },
        {
            "name": "run_command",
            "description": format!("Run one of the engine's commands on the open project, then reschedule: {commands}. Parameters are a JSON object, e.g. activity.add {{\"name\": \"Survey\", \"duration\": 5, \"after\": \"A10\"}}, activity.edit {{\"id\": \"A10\", \"duration\": 7, \"constraint\": {{\"type\": \"start-on-or-after\", \"date\": \"2026-11-09\"}}}}, relationship.add {{\"predecessor\": \"A10\", \"successor\": \"A20\", \"kind\": \"FS\", \"lag\": 2}}, project.options {{\"data_date\": \"2026-11-10\"}}, band.add {{\"name\": \"Rains\", \"from\": \"2026-12-01\", \"to\": \"2027-01-31\"}}. Edits can be undone with edit.undo. A change that would make a loop is refused and the project stays as it was."),
            "inputSchema": {
                "type": "object",
                "properties": {"command": {"type": "string", "enum": COMMANDS}, "params": {"type": "object"}},
                "required": ["command"]
            }
        },
        {
            "name": "explain_activity",
            "description": "How the scheduler reached an activity's dates and floats: each bound of the forward and backward pass, the one that wins, the calendar's placing, the free float's slacks.",
            "inputSchema": {"type": "object", "properties": {"id": {"type": "string", "description": "The activity's ID"}}, "required": ["id"]}
        },
        {
            "name": "save_project",
            "description": "Write the open project, all or nothing: P6 XML when the path ends in .xml, Perth.jl's JSON otherwise. Returns what the format couldn't hold.",
            "inputSchema": {"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}
        },
        {
            "name": "export_latex",
            "description": "Write an activity's calculation as a LaTeX document that pdflatex compiles.",
            "inputSchema": {"type": "object", "properties": {"id": {"type": "string"}, "path": {"type": "string"}}, "required": ["id", "path"]}
        }
    ])
}

struct Server {
    session: Session,
}

/// A tool's answer: its text, and whether it is an error.
type Answer = (String, bool);

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

impl Server {
    fn new() -> Self {
        Self { session: Session::default() }
    }

    /// The answer to a line: one message, or a batch as an array; `None` when nothing needs one
    /// (notifications, a batch of them).
    fn handle_line(&mut self, line: &str) -> Option<String> {
        let message: Value = match serde_json::from_str(line) {
            Ok(m) => m,
            Err(e) => return Some(error(Value::Null, PARSE_ERROR, &format!("not JSON: {e}")).to_string()),
        };
        match message {
            Value::Array(items) if items.is_empty() => Some(error(Value::Null, INVALID_REQUEST, "an empty batch").to_string()),
            Value::Array(items) => {
                let answers: Vec<Value> = items.iter().filter_map(|m| self.handle(m)).collect();
                (!answers.is_empty()).then(|| Value::Array(answers).to_string())
            }
            m => self.handle(&m).map(|a| a.to_string()),
        }
    }

    /// The answer to one message; `None` for a notification.
    fn handle(&mut self, message: &Value) -> Option<Value> {
        let Value::Object(m) = message else { return Some(error(Value::Null, INVALID_REQUEST, "a message is an object")) };
        let id = m.get("id").cloned();
        let Some(method) = m.get("method").and_then(Value::as_str) else {
            // A response sent to the server, or nonsense: nothing to do but say so if it has an id.
            return id.map(|id| error(id, INVALID_REQUEST, "no method"));
        };
        // Notifications (no id) get no answer.
        let id = id?;
        if !matches!(id, Value::String(_) | Value::Number(_)) {
            return Some(error(Value::Null, INVALID_REQUEST, "an id is a string or a number"));
        }
        let params = m.get("params").cloned().unwrap_or(Value::Null);
        let result = match method {
            "initialize" => json!({
                "protocolVersion": PROTOCOL,
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "mkperth", "version": env!("CARGO_PKG_VERSION")},
                "instructions": "MK Perth: critical-path scheduling, Primavera P6 style. Open a project with open_project (or work on the sample that is open), read it with get_schedule, change it with run_command, ask how a date came about with explain_activity, then save_project."
            }),
            "ping" => json!({}),
            "tools/list" => json!({"tools": tools()}),
            "tools/call" => {
                let Some(name) = params.get("name").and_then(Value::as_str) else {
                    return Some(error(id, INVALID_PARAMS, "tools/call needs a tool name"));
                };
                let args = params.get("arguments").cloned().unwrap_or(Value::Null);
                if !matches!(args, Value::Object(_) | Value::Null) {
                    return Some(error(id, INVALID_PARAMS, "a tool's arguments are an object"));
                }
                let Some((text, is_error)) = self.call(name, &args) else {
                    return Some(error(id, INVALID_PARAMS, &format!("there is no tool {name}")));
                };
                json!({"content": [{"type": "text", "text": text}], "isError": is_error})
            }
            other => return Some(error(id, METHOD_NOT_FOUND, &format!("no method {other}"))),
        };
        Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
    }

    /// A tool's answer; `None` when there is no such tool.
    fn call(&mut self, name: &str, args: &Value) -> Option<Answer> {
        let text = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
        let missing = |key: &str| (format!("{name} needs {key}."), true);
        Some(match name {
            "open_project" if text("path").is_empty() => missing("path"),
            "open_project" => match self.session.open(Path::new(&text("path"))) {
                Ok(()) => {
                    let said = self.session.warnings.join("\n");
                    let schedule = self.schedule("table");
                    (if said.is_empty() { schedule.0 } else { format!("{said}\n\n{}", schedule.0) }, schedule.1)
                }
                Err(e) => (e.to_string(), true),
            },
            "get_schedule" => self.schedule(&text("format")),
            "run_command" if text("command").is_empty() => missing("command"),
            "run_command" => {
                let params = args.get("params").cloned().unwrap_or(Value::Null);
                match self.session.run(&text("command"), &params) {
                    Ok(()) => match &self.session.schedule {
                        Ok(s) => (format!("Done. The project finishes {}; {} activities.", s.finish, s.activities.len()), false),
                        Err(e) => (format!("Done, but the project doesn't schedule: {e}"), true),
                    },
                    Err(e) => (e.to_string(), true),
                }
            }
            "explain_activity" if text("id").is_empty() => missing("id"),
            "explain_activity" => match self.session.explain(&text("id")) {
                Ok(e) => (e.lines(&|d| d.to_string()).join("\n"), false),
                Err(e) => (e.to_string(), true),
            },
            "save_project" if text("path").is_empty() => missing("path"),
            "save_project" => match self.session.save(Path::new(&text("path"))) {
                Ok(()) if self.session.warnings.is_empty() => (format!("Wrote {}", text("path")), false),
                Ok(()) => (format!("Wrote {}\n{}", text("path"), self.session.warnings.join("\n")), false),
                Err(e) => (e.to_string(), true),
            },
            "export_latex" if text("id").is_empty() => missing("id"),
            "export_latex" if text("path").is_empty() => missing("path"),
            "export_latex" => match self.session.export_latex(&text("id"), Path::new(&text("path")), &|d| d.to_string()) {
                Ok(()) => (format!("Wrote {}", text("path")), false),
                Err(e) => (e.to_string(), true),
            },
            _ => return None,
        })
    }

    fn schedule(&self, format: &str) -> Answer {
        let out = match format {
            "json" => self.session.schedule_json().map(|v| v.to_string()),
            "csv" => self.session.schedule_csv(),
            _ => self.session.schedule_table(&|d| d.to_string()),
        };
        match out {
            Ok(text) => (text, false),
            Err(e) => (e.to_string(), true),
        }
    }
}

/// Reads one line of at most `MAX_LINE` bytes into `buf`; `Ok(false)` at the end. A longer line is
/// read to its end and dropped, `buf` left over the limit so the caller refuses it.
fn read_line(input: &mut impl BufRead, buf: &mut Vec<u8>) -> std::io::Result<bool> {
    buf.clear();
    loop {
        let chunk = input.fill_buf()?;
        if chunk.is_empty() {
            return Ok(!buf.is_empty());
        }
        let (take, done) = match chunk.iter().position(|b| *b == b'\n') {
            Some(at) => (at + 1, true),
            None => (chunk.len(), false),
        };
        if buf.len() <= MAX_LINE {
            buf.extend_from_slice(chunk.get(..take).unwrap_or(chunk));
        }
        input.consume(take);
        if done {
            return Ok(true);
        }
    }
}

fn main() {
    let mut server = Server::new();
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut stdout = std::io::stdout();
    let mut buf = Vec::new();
    while let Ok(true) = read_line(&mut input, &mut buf) {
        let answer = if buf.len() > MAX_LINE {
            Some(error(Value::Null, INVALID_REQUEST, "the message is too long").to_string())
        } else {
            match std::str::from_utf8(&buf) {
                Ok(line) if line.trim().is_empty() => None,
                Ok(line) => server.handle_line(line),
                Err(_) => Some(error(Value::Null, PARSE_ERROR, "the message isn't UTF-8").to_string()),
            }
        };
        if let Some(answer) = answer
            && writeln!(stdout, "{answer}").and_then(|()| stdout.flush()).is_err()
        {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(server: &mut Server, id: u32, method: &str, params: Value) -> Value {
        let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string();
        serde_json::from_str(&server.handle_line(&line).unwrap()).unwrap()
    }

    fn tool(server: &mut Server, name: &str, arguments: Value) -> (String, bool) {
        let answer = call(server, 9, "tools/call", json!({"name": name, "arguments": arguments}));
        let result = &answer["result"];
        (result["content"][0]["text"].as_str().unwrap().to_string(), result["isError"].as_bool().unwrap())
    }

    #[test]
    fn the_protocol_and_its_errors() {
        let mut s = Server::new();
        let init =
            call(&mut s, 1, "initialize", json!({"protocolVersion": PROTOCOL, "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}));
        assert_eq!(init["result"]["serverInfo"]["name"], "mkperth");
        assert_eq!(s.handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#), None);
        let list = call(&mut s, 2, "tools/list", json!({}));
        let names: Vec<&str> = list["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["open_project", "get_schedule", "run_command", "explain_activity", "save_project", "export_latex"]);
        assert_eq!(call(&mut s, 3, "nope", json!({}))["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(call(&mut s, 4, "tools/call", json!({}))["error"]["code"], INVALID_PARAMS);
        assert_eq!(call(&mut s, 5, "tools/call", json!({"name": "nope"}))["error"]["code"], INVALID_PARAMS);
        assert_eq!(call(&mut s, 6, "tools/call", json!({"name": "get_schedule", "arguments": [1]}))["error"]["code"], INVALID_PARAMS);
        let code = |line: &str, s: &mut Server| serde_json::from_str::<Value>(&s.handle_line(line).unwrap()).unwrap()["error"]["code"].clone();
        assert_eq!(code("{oops", &mut s), PARSE_ERROR);
        assert_eq!(code("[]", &mut s), INVALID_REQUEST);
        assert_eq!(code("42", &mut s), INVALID_REQUEST);
        assert_eq!(code(r#"{"jsonrpc":"2.0","id":{"x":1},"method":"ping"}"#, &mut s), INVALID_REQUEST);
        // A batch: an answer for each request, none for its notification.
        let batch = s
            .handle_line(
                r#"[{"jsonrpc":"2.0","id":1,"method":"ping"},{"jsonrpc":"2.0","method":"notifications/x"},{"jsonrpc":"2.0","id":2,"method":"nope"}]"#,
            )
            .unwrap();
        let batch: Value = serde_json::from_str(&batch).unwrap();
        assert_eq!(batch.as_array().map(Vec::len), Some(2));
        assert_eq!(batch[1]["error"]["code"], METHOD_NOT_FOUND);
    }

    #[test]
    fn an_agent_edits_the_sample() {
        let mut s = Server::new();
        let (text, err) = tool(&mut s, "get_schedule", json!({"format": "json"}));
        assert!(!err);
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["finish"], "2027-04-20");
        let (text, err) = tool(&mut s, "run_command", json!({"command": "activity.edit", "params": {"id": "C30", "duration": 40}}));
        assert!(!err && text.starts_with("Done."), "{text}");
        let (text, err) = tool(&mut s, "run_command", json!({"command": "relationship.add", "params": {"predecessor": "C30", "successor": "A10"}}));
        assert!(err && text.contains("loop"), "{text}");
        let (text, err) = tool(&mut s, "explain_activity", json!({"id": "A20"}));
        assert!(!err && text.contains("Forward pass"), "{text}");
        assert!(tool(&mut s, "explain_activity", json!({"id": "zz"})).1);
        assert!(tool(&mut s, "open_project", json!({"path": "/no/such/file.json"})).1);
        assert!(tool(&mut s, "save_project", json!({})).1);
        let dir = std::env::temp_dir().join(format!("mkperth-mcp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.xml");
        let (text, err) = tool(&mut s, "save_project", json!({"path": path.display().to_string()}));
        assert!(!err, "{text}");
        let (text, err) = tool(&mut s, "open_project", json!({"path": path.display().to_string()}));
        assert!(!err && text.contains("C30"), "{text}");
        let tex = dir.join("a20.tex");
        assert!(!tool(&mut s, "export_latex", json!({"id": "A20", "path": tex.display().to_string()})).1);
        assert!(std::fs::read_to_string(&tex).unwrap().contains("\\documentclass"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn long_lines_are_cut_off_and_lines_are_read_whole() {
        let mut input = std::io::Cursor::new(b"abc\n\xff\xfe\nlast".to_vec());
        let mut buf = Vec::new();
        assert!(read_line(&mut input, &mut buf).unwrap());
        assert_eq!(buf, b"abc\n");
        assert!(read_line(&mut input, &mut buf).unwrap());
        assert!(std::str::from_utf8(&buf).is_err());
        assert!(read_line(&mut input, &mut buf).unwrap());
        assert_eq!(buf, b"last");
        assert!(!read_line(&mut input, &mut buf).unwrap());
    }
}
