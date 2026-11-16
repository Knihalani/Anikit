//! Token budget of "make a launch video" over MCP, end to end: every request
//! the agent sends and every response it reads. Run with `--nocapture` to see
//! the table; the test fails if the session grows past its budget.

use anikit_video::mcp::Server;
use serde_json::{json, Value};

#[test]
fn launch_video_session_budget() {
    let mut s = Server::new();
    s.workdir = std::env::temp_dir().join(format!("anikit-budget-{}", std::process::id()));
    let steps: Vec<(&str, Value)> = vec![
        ("initialize", json!({ "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "agent", "version": "1" } } })),
        ("tools/list", json!({ "method": "tools/list" })),
        ("list_templates", json!({ "method": "tools/call", "params": { "name": "list_templates", "arguments": {} } })),
        ("get_template_schema", json!({ "method": "tools/call", "params": { "name": "get_template_schema", "arguments": { "name": "launch" } } })),
        ("create_from_template", json!({ "method": "tools/call", "params": { "name": "create_from_template", "arguments": {
            "template": "launch", "format": "16:9", "out": s.workdir.join("launch.json").display().to_string(),
            "brand": { "name": "Lumen", "colors": { "primary": "#0F766E" }, "voice": { "cta": "Try Lumen free", "url": "lumen.app" } },
            "params": { "product": "Lumen", "tagline": "Your notes, searchable in seconds.", "features": [ { "title": "Instant search" }, { "title": "Offline first" }, { "title": "Shared spaces" } ] }
        } } })),
        ("render_contact_sheet", json!({ "method": "tools/call", "params": { "name": "render_contact_sheet", "arguments": { "path": s.workdir.join("launch.json").display().to_string() } } })),
    ];
    let mut total = 0;
    println!("{:<22} {:>8} {:>8} {:>8}", "step", "sent", "read", "≈tokens");
    for (i, (name, mut msg)) in steps.into_iter().enumerate() {
        msg["jsonrpc"] = json!("2.0");
        msg["id"] = json!(i + 1);
        let req = msg.to_string();
        let resp = s.handle_line(&req).unwrap();
        let v: Value = serde_json::from_str(&resp).unwrap();
        assert!(v.get("error").is_none() && v["result"]["isError"] != true, "{name}: {resp}");
        let n = req.len() + resp.len();
        total += n;
        println!("{:<22} {:>8} {:>8} {:>8}", name, req.len(), resp.len(), n / 4);
    }
    println!("{:<22} {:>26}", "total (chars / ≈tokens)", format!("{total} / {}", total / 4));
    // Render of the final video (one more short call) is not counted: it needs the pipeline binary.
    assert!(total < 9_000, "the launch-video MCP session costs {total} chars; keep it under 9000");
}
