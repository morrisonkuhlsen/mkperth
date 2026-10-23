//! The server driven end to end over its pipes, hostile lines among the good ones.

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::Value;

#[test]
fn a_session_over_stdio_survives_hostile_lines() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mkperth-mcp")).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut input: Vec<u8> = Vec::new();
    input.extend_from_slice(br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}"#);
    input.push(b'\n');
    input.extend_from_slice(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n");
    // Not UTF-8, not JSON, an empty line, a deep nest: each answered or skipped, never fatal.
    input.extend_from_slice(b"\xff\xfe\xfd\n{{{{\n\n");
    input.extend_from_slice("[".repeat(100_000).as_bytes());
    input.push(b'\n');
    input.extend_from_slice(br#"[{"jsonrpc":"2.0","id":2,"method":"tools/list"},{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"run_command","arguments":{"command":"activity.add","params":{"name":"Survey","duration":2}}}}]"#);
    input.push(b'\n');
    input.extend_from_slice(br#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"get_schedule","arguments":{"format":"csv"}}}"#);
    input.push(b'\n');
    child.stdin.take().unwrap().write_all(&input).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let answers: Vec<Value> = String::from_utf8(out.stdout).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    // initialize; not UTF-8; not JSON; too deep; the batch; the schedule.
    assert_eq!(answers.len(), 6, "{answers:#?}");
    assert_eq!(answers[0]["result"]["serverInfo"]["name"], "mkperth");
    for a in &answers[1..4] {
        assert_eq!(a["error"]["code"], -32700, "{a}");
    }
    let batch = answers[4].as_array().unwrap();
    assert_eq!(batch.len(), 2);
    assert_eq!(batch[1]["result"]["isError"], false, "{}", batch[1]);
    assert!(answers[5]["result"]["content"][0]["text"].as_str().unwrap().contains(",Survey,"));
}
