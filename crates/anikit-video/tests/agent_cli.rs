//! End-to-end tests of the agent-facing commands through the real binary.

use std::io::Write;
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_anikit-video");
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

fn run(args: &[&str], cwd: &std::path::Path) -> (bool, String, String) {
    let o = Command::new(BIN).args(args).current_dir(cwd).output().unwrap();
    (o.status.success(), String::from_utf8_lossy(&o.stdout).into(), String::from_utf8_lossy(&o.stderr).into())
}

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("anikit-video-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn new_scaffolds_a_project_from_a_readme() {
    let d = tmp("new");
    std::fs::write(d.join("README.md"), "# Lumen\n\nLumen turns your notes into a searchable second brain.\n\n## Features\n\n- Instant search: find anything in 20 ms\n- Offline first — your data stays on device\n").unwrap();
    let (ok, out, err) = run(&["new", "launch", "--from", "README.md", "--format", "9:16"], &d);
    assert!(ok, "{out}{err}");
    for f in ["params.json", "brand.json", "spec.json", "NEXT.md"] {
        assert!(d.join("launch").join(f).exists(), "missing {f}");
    }
    let params: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(d.join("launch/params.json")).unwrap()).unwrap();
    assert_eq!(params["product"], "Lumen");
    assert_eq!(params["features"][0]["title"], "Instant search");
    assert!(out.contains("still example copy"), "{out}");
    // Running it again refuses to overwrite.
    let (ok, _, err) = run(&["new", "launch"], &d);
    assert!(!ok && err.contains("--force"), "{err}");
}

#[test]
fn expand_examples_and_critique() {
    let d = tmp("expand");
    let params = format!("{ROOT}/examples/video/launch/params.json");
    let brand = format!("{ROOT}/examples/video/brand.json");
    for f in ["16:9", "9:16", "1:1"] {
        let out = d.join("spec.json");
        let (ok, _, err) = run(&["templates", "expand", "launch", &params, "--brand", &brand, "--format", f, "-o", out.to_str().unwrap()], &d);
        assert!(ok, "{err}");
        // Templates are honest quick starts: flat 2D (BAR #5) and slower than their intent; nothing else.
        let issues: Vec<&str> = err.lines().filter(|l| l.starts_with("warning") || l.starts_with("error")).collect();
        assert!(issues.iter().all(|l| l.contains("BAR #5") || l.contains("intent") || l.contains("nothing moves")), "{f}: {err}");
        assert!(err.contains("BAR #5") && err.contains("critique "), "{f}: {err}");
        let (ok, json, _) = run(&["critique", out.to_str().unwrap(), "--json"], &d);
        let r: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(ok && r["score"] == 94, "{f}: {json}");
    }
    let (_, text, _) = run(&["critique", &format!("{ROOT}/examples/video/hello.json")], &d);
    assert!(text.starts_with("score 100/100"), "{text}");
}

#[test]
fn errors_are_actionable() {
    let d = tmp("errors");
    let (ok, _, err) = run(&["templates", "expand", "lanch"], &d);
    assert!(!ok && err.contains("did you mean 'launch'"), "{err}");
    std::fs::write(d.join("p.json"), r#"{"hok": "x", "tagline": "y", "features": []}"#).unwrap();
    let (ok, _, err) = run(&["templates", "expand", "launch", "p.json"], &d);
    assert!(!ok, "{err}");
    assert!(err.contains("did you mean 'hook'"), "{err}");
    assert!(err.contains("needs at least 2 items"), "{err}");
    let (_, _, err) = run(&["templates", "expand", "launch", "--formt", "1:1"], &d);
    assert!(err.contains("did you mean --format"), "{err}");
    let (_, _, err) = run(&["prompt", "critc"], &d);
    assert!(err.contains("did you mean 'critic'"), "{err}");
    std::fs::write(d.join("bad.json"), "{\"scenes\": [}").unwrap();
    let (_, _, err) = run(&["critique", "bad.json"], &d);
    assert!(err.contains("line 1"), "{err}");
}

#[test]
fn mcp_over_stdio() {
    let mut child = Command::new(BIN).arg("mcp").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let msgs = [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}"#,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_template_schema","arguments":{"name":"teaser"}}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"prompts/list"}"#,
    ];
    {
        let mut stdin = child.stdin.take().unwrap();
        for m in msgs {
            writeln!(stdin, "{m}").unwrap();
        }
    }
    let out = child.wait_with_output().unwrap();
    let lines: Vec<serde_json::Value> = String::from_utf8_lossy(&out.stdout).lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines.len(), 3, "one response per request, none for the notification");
    assert_eq!(lines[0]["id"], 1);
    assert_eq!(lines[1]["result"]["isError"], false);
    assert!(lines[2]["result"]["prompts"].as_array().unwrap().iter().any(|p| p["name"] == "launch-video-generator"));
}
