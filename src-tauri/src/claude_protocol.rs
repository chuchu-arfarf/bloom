//! Pure helpers for Claude Code's streaming JSON mode (`claude -p
//! --input-format stream-json --output-format stream-json`): the lines we send,
//! the lines we receive, and how tool calls are summarized for approval.
//! No IO here, so everything is unit-tested.
use serde::Serialize;
use serde_json::{json, Value};

/// A tool call, described for the panel. `target` is the file, command,
/// pattern or address the tool works on; the panel words it per tool.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct ToolCall {
    pub tool: String,
    pub target: String,
    pub detail: String,
}

/// Why a turn failed: Claude's own message, or a known code the UI translates.
#[derive(Debug, Clone, PartialEq)]
pub enum TurnError {
    Message(String),
    MaxTurns,
    Generic,
}

/// Something Claude Code told us, reduced to what the panel needs.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The session started or resumed under this id.
    SessionId(String),
    /// Text from Claude in the main conversation.
    Text(String),
    /// Claude used a tool.
    ToolUse(ToolCall),
    /// The turn finished. `error` is set when it failed.
    TurnDone {
        error: Option<TurnError>,
        session_id: Option<String>,
    },
    /// Claude wants to use a tool and is waiting for an answer.
    Permission {
        request_id: String,
        tool: String,
        input: Value,
        title: Option<String>,
    },
    /// Any other request to us; we answer with an error so it never waits.
    UnsupportedRequest { request_id: String },
    /// Claude withdrew one of its requests.
    Cancelled { request_id: String },
}

fn str_of<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

/// Parses one stdout line. Unknown or malformed lines yield nothing.
pub fn parse_line(line: &str) -> Vec<Event> {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return Vec::new();
    };
    match str_of(&value, "type") {
        Some("system") if str_of(&value, "subtype") == Some("init") => str_of(&value, "session_id")
            .map(|id| vec![Event::SessionId(id.to_string())])
            .unwrap_or_default(),
        Some("assistant") => parse_assistant(&value),
        Some("result") => {
            let failed = value.get("is_error").and_then(Value::as_bool) == Some(true)
                || str_of(&value, "subtype").is_some_and(|s| s != "success");
            let error = failed.then(|| {
                match str_of(&value, "result")
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                {
                    Some(text) => TurnError::Message(text.to_string()),
                    None if str_of(&value, "subtype") == Some("error_max_turns") => {
                        TurnError::MaxTurns
                    }
                    None => TurnError::Generic,
                }
            });
            vec![Event::TurnDone {
                error,
                session_id: str_of(&value, "session_id").map(str::to_string),
            }]
        }
        Some("control_request") => {
            let request_id = str_of(&value, "request_id").unwrap_or_default().to_string();
            let Some(request) = value.get("request") else {
                return Vec::new();
            };
            if str_of(request, "subtype") == Some("can_use_tool") {
                vec![Event::Permission {
                    request_id,
                    tool: str_of(request, "tool_name").unwrap_or("tool").to_string(),
                    input: request.get("input").cloned().unwrap_or(Value::Null),
                    title: str_of(request, "title").map(str::to_string),
                }]
            } else {
                vec![Event::UnsupportedRequest { request_id }]
            }
        }
        Some("control_cancel_request") => str_of(&value, "request_id")
            .map(|id| {
                vec![Event::Cancelled {
                    request_id: id.to_string(),
                }]
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn parse_assistant(value: &Value) -> Vec<Event> {
    // Sub-agent messages carry their parent tool call; show only the main thread.
    if value
        .get("parent_tool_use_id")
        .and_then(Value::as_str)
        .is_some()
    {
        return Vec::new();
    }
    let Some(content) = value.pointer("/message/content").and_then(Value::as_array) else {
        return Vec::new();
    };
    content
        .iter()
        .filter_map(|block| match str_of(block, "type") {
            Some("text") => str_of(block, "text")
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(|text| Event::Text(text.to_string())),
            Some("tool_use") => Some(Event::ToolUse(describe_tool(
                str_of(block, "name").unwrap_or("tool"),
                block.get("input").unwrap_or(&Value::Null),
            ))),
            _ => None,
        })
        .collect()
}

// ── Lines we send ──────────────────────────────────────────────────────────

pub fn initialize_request(request_id: &str) -> String {
    json!({"type": "control_request", "request_id": request_id, "request": {"subtype": "initialize"}})
        .to_string()
}

pub fn usage_request(request_id: &str) -> String {
    json!({
        "type": "control_request",
        "request_id": request_id,
        "request": {"subtype": "get_usage", "skip_behaviors": true}
    })
    .to_string()
}

pub fn interrupt_request(request_id: &str) -> String {
    json!({"type": "control_request", "request_id": request_id, "request": {"subtype": "interrupt"}})
        .to_string()
}

pub fn user_message(text: &str) -> String {
    json!({
        "type": "user",
        "message": {"role": "user", "content": text},
        "parent_tool_use_id": null
    })
    .to_string()
}

pub fn permission_response(request_id: &str, allow: bool, input: &Value) -> String {
    let decision = if allow {
        let input = if input.is_object() {
            input.clone()
        } else {
            json!({})
        };
        json!({"behavior": "allow", "updatedInput": input})
    } else {
        json!({"behavior": "deny", "message": "The user declined this action in Bloom."})
    };
    json!({
        "type": "control_response",
        "response": {"subtype": "success", "request_id": request_id, "response": decision}
    })
    .to_string()
}

pub fn unsupported_response(request_id: &str) -> String {
    json!({
        "type": "control_response",
        "response": {"subtype": "error", "request_id": request_id, "error": "Not supported by Bloom"}
    })
    .to_string()
}

// ── Tool summaries ─────────────────────────────────────────────────────────

fn clip(text: &str, max: usize) -> String {
    let text = text.replace('\r', "");
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut clipped: String = text.chars().take(max).collect();
    clipped.push('…');
    clipped
}

fn file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

/// Describes a tool call: what it works on and the full detail. The words
/// around them ("Edit …", "Run …") come from the UI's translations.
pub fn describe_tool(tool: &str, input: &Value) -> ToolCall {
    let get = |key: &str| str_of(input, key).unwrap_or_default();
    let path = [get("file_path"), get("notebook_path"), get("path")]
        .into_iter()
        .find(|p| !p.is_empty())
        .unwrap_or_default();
    let (target, detail) = match tool {
        "Bash" | "PowerShell" => {
            let command = get("command");
            let description = get("description");
            let detail = if description.is_empty() {
                command.to_string()
            } else {
                format!("{description}\n\n{command}")
            };
            (clip(command, 70), detail)
        }
        "Edit" | "MultiEdit" => (
            file_name(path).to_string(),
            format!(
                "{path}\n\n- {}\n+ {}",
                clip(get("old_string"), 400),
                clip(get("new_string"), 400)
            ),
        ),
        "Write" => (
            file_name(path).to_string(),
            format!("{path}\n\n{}", clip(get("content"), 600)),
        ),
        "NotebookEdit" | "Read" => (file_name(path).to_string(), path.to_string()),
        "Glob" | "Grep" => (clip(get("pattern"), 50), get("pattern").to_string()),
        "WebFetch" => (clip(get("url"), 60), get("url").to_string()),
        "WebSearch" => (clip(get("query"), 60), get("query").to_string()),
        "Task" | "Agent" => (clip(get("description"), 60), clip(get("prompt"), 600)),
        _ => (
            String::new(),
            serde_json::to_string_pretty(input).unwrap_or_default(),
        ),
    };
    ToolCall {
        tool: tool.to_string(),
        target,
        detail,
    }
}

// ── Usage ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct UsageWindow {
    /// Stable id: "five_hour", "seven_day", "seven_day_opus", "seven_day_sonnet" or "model".
    pub id: String,
    /// Model bucket name for per-model weekly windows (from Claude; not translated).
    pub model: Option<String>,
    pub percent: f64,
    /// ISO 8601 reset time, as Claude reports it.
    pub resets_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct UsageInfo {
    pub plan: Option<String>,
    pub available: bool,
    pub windows: Vec<UsageWindow>,
    /// Stable error code for the UI: "not_installed", "no_answer", "no_plan" or "failed".
    pub error: Option<String>,
    /// Unix ms of the check.
    pub checked_at: u64,
}

/// Finds the `get_usage` answer for `request_id` in a stdout line.
pub fn parse_usage_line(line: &str, request_id: &str) -> Option<UsageInfo> {
    let value: Value = serde_json::from_str(line).ok()?;
    if str_of(&value, "type") != Some("control_response") {
        return None;
    }
    let response = value.get("response")?;
    if str_of(response, "request_id") != Some(request_id) {
        return None;
    }
    if str_of(response, "subtype") != Some("success") {
        return Some(UsageInfo {
            error: Some("failed".into()),
            ..Default::default()
        });
    }
    let data = response.get("response").unwrap_or(&Value::Null);
    let available = data.get("rate_limits_available").and_then(Value::as_bool) == Some(true);
    let limits = data.get("rate_limits").unwrap_or(&Value::Null);

    let mut windows = Vec::new();
    for id in [
        "five_hour",
        "seven_day",
        "seven_day_opus",
        "seven_day_sonnet",
    ] {
        if let Some(window) = limits.get(id).and_then(|w| read_window(w, id, None)) {
            windows.push(window);
        }
    }
    if let Some(scoped) = limits.get("model_scoped").and_then(Value::as_array) {
        for item in scoped {
            let model = str_of(item, "display_name").map(str::to_string);
            if model.is_some() {
                if let Some(window) = read_window(item, "model", model) {
                    windows.push(window);
                }
            }
        }
    }

    Some(UsageInfo {
        plan: str_of(data, "subscription_type").map(str::to_string),
        available,
        windows,
        error: (!available).then(|| "no_plan".to_string()),
        checked_at: 0,
    })
}

fn read_window(value: &Value, id: &str, model: Option<String>) -> Option<UsageWindow> {
    Some(UsageWindow {
        id: id.to_string(),
        model,
        percent: value.get("utilization")?.as_f64()?,
        resets_at: str_of(value, "resets_at").map(str::to_string),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_line_gives_session_id() {
        let line = r#"{"type":"system","subtype":"init","session_id":"abc","tools":[]}"#;
        assert_eq!(parse_line(line), vec![Event::SessionId("abc".into())]);
    }

    #[test]
    fn assistant_text_and_tools_are_read_in_order() {
        let line = r#"{"type":"assistant","parent_tool_use_id":null,"message":{"content":[
            {"type":"text","text":"  Looking now. "},
            {"type":"tool_use","name":"Read","input":{"file_path":"C:\\proj\\src\\main.rs"}}]}}"#;
        assert_eq!(
            parse_line(line),
            vec![
                Event::Text("Looking now.".into()),
                Event::ToolUse(ToolCall {
                    tool: "Read".into(),
                    target: "main.rs".into(),
                    detail: "C:\\proj\\src\\main.rs".into()
                })
            ]
        );
    }

    #[test]
    fn sub_agent_messages_are_hidden() {
        let line = r#"{"type":"assistant","parent_tool_use_id":"t1","message":{"content":[{"type":"text","text":"x"}]}}"#;
        assert!(parse_line(line).is_empty());
    }

    #[test]
    fn permission_request_is_parsed() {
        let line = r#"{"type":"control_request","request_id":"r1","request":{"subtype":"can_use_tool","tool_name":"Bash","input":{"command":"npm test"}}}"#;
        match &parse_line(line)[0] {
            Event::Permission {
                request_id,
                tool,
                input,
                title,
            } => {
                assert_eq!(request_id, "r1");
                assert_eq!(tool, "Bash");
                assert_eq!(input["command"], "npm test");
                assert!(title.is_none());
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn other_requests_are_unsupported() {
        let line =
            r#"{"type":"control_request","request_id":"h1","request":{"subtype":"hook_callback"}}"#;
        assert_eq!(
            parse_line(line),
            vec![Event::UnsupportedRequest {
                request_id: "h1".into()
            }]
        );
    }

    #[test]
    fn failed_result_carries_its_message() {
        let line = r#"{"type":"result","subtype":"success","is_error":true,"result":"Not logged in","session_id":"s"}"#;
        assert_eq!(
            parse_line(line),
            vec![Event::TurnDone {
                error: Some(TurnError::Message("Not logged in".into())),
                session_id: Some("s".into())
            }]
        );
    }

    #[test]
    fn successful_result_has_no_error() {
        let line = r#"{"type":"result","subtype":"success","is_error":false,"result":"done"}"#;
        assert_eq!(
            parse_line(line),
            vec![Event::TurnDone {
                error: None,
                session_id: None
            }]
        );
    }

    #[test]
    fn garbage_is_ignored() {
        assert!(parse_line("not json").is_empty());
        assert!(parse_line(r#"{"type":"stream_event"}"#).is_empty());
    }

    #[test]
    fn allow_echoes_the_input_and_deny_explains() {
        let input = json!({"command": "ls"});
        let allow: Value = serde_json::from_str(&permission_response("r", true, &input)).unwrap();
        assert_eq!(allow["response"]["response"]["behavior"], "allow");
        assert_eq!(
            allow["response"]["response"]["updatedInput"]["command"],
            "ls"
        );
        let deny: Value = serde_json::from_str(&permission_response("r", false, &input)).unwrap();
        assert_eq!(deny["response"]["response"]["behavior"], "deny");
        assert_eq!(deny["response"]["request_id"], "r");
    }

    #[test]
    fn user_message_shape() {
        let v: Value = serde_json::from_str(&user_message("hi")).unwrap();
        assert_eq!(v["type"], "user");
        assert_eq!(v["message"]["content"], "hi");
        assert!(v["parent_tool_use_id"].is_null());
    }

    #[test]
    fn tool_summaries() {
        let call = describe_tool(
            "Bash",
            &json!({"command": "cargo test", "description": "Run tests"}),
        );
        assert_eq!(call.target, "cargo test");
        assert!(call.detail.starts_with("Run tests"));
        assert_eq!(
            describe_tool("Write", &json!({"file_path": "/a/b/c.txt", "content": "x"})).target,
            "c.txt"
        );
        assert_eq!(describe_tool("Mystery", &json!({})).target, "");
    }

    #[test]
    fn max_turns_has_its_own_code() {
        let line = r#"{"type":"result","subtype":"error_max_turns","is_error":true}"#;
        assert_eq!(
            parse_line(line),
            vec![Event::TurnDone {
                error: Some(TurnError::MaxTurns),
                session_id: None
            }]
        );
    }

    #[test]
    fn clip_keeps_short_text_and_marks_long_text() {
        assert_eq!(clip("short", 10), "short");
        assert_eq!(clip("abcdefghijkl", 5), "abcde…");
    }

    #[test]
    fn usage_is_parsed() {
        let line = r#"{"type":"control_response","response":{"subtype":"success","request_id":"u","response":{
            "subscription_type":"max","rate_limits_available":true,
            "rate_limits":{"five_hour":{"utilization":62.5,"resets_at":"2026-11-23T03:10:00Z"},
                           "seven_day":{"utilization":38,"resets_at":null},
                           "model_scoped":[{"display_name":"Fable","utilization":12,"resets_at":null}]}}}}"#;
        let usage = parse_usage_line(line, "u").unwrap();
        assert!(usage.available);
        assert_eq!(usage.plan.as_deref(), Some("max"));
        assert_eq!(usage.windows.len(), 3);
        assert_eq!(usage.windows[0].id, "five_hour");
        assert_eq!(usage.windows[0].percent, 62.5);
        assert_eq!(usage.windows[2].model.as_deref(), Some("Fable"));
        assert!(usage.error.is_none());
    }

    #[test]
    fn usage_without_a_plan_says_so() {
        let line = r#"{"type":"control_response","response":{"subtype":"success","request_id":"u","response":{
            "subscription_type":null,"rate_limits_available":false,"rate_limits":null}}}"#;
        let usage = parse_usage_line(line, "u").unwrap();
        assert!(!usage.available);
        assert_eq!(usage.error.as_deref(), Some("no_plan"));
        assert!(parse_usage_line(line, "other").is_none());
    }
}
