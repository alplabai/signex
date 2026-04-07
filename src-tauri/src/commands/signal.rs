use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::Emitter;

// Claude API configuration
static API_KEY: std::sync::LazyLock<Mutex<Option<String>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchematicContext {
    pub component_count: usize,
    pub wire_count: usize,
    pub net_count: usize,
    pub selected_components: Vec<SelectedComponent>,
    pub erc_errors: usize,
    pub erc_warnings: usize,
    pub paper_size: String,
    pub title: String,
    #[serde(default)]
    pub detailed_context: Option<String>,
    #[serde(default)]
    pub design_brief: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectedComponent {
    pub reference: String,
    pub value: String,
    pub footprint: String,
    pub lib_id: String,
}

// --- Claude API types ---

#[derive(Debug, Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<ClaudeApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolDef>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ClaudeApiMessage {
    role: String,
    content: serde_json::Value, // String or array of content blocks
}

#[derive(Debug, Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContent>,
    #[allow(dead_code)]
    model: String,
    stop_reason: Option<String>,
    usage: ClaudeUsage,
}

#[derive(Debug, Deserialize)]
struct ClaudeContent {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
    // Tool use fields
    id: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignalResponse {
    pub message: String,
    pub usage: ClaudeUsage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct ToolDef {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

// --- Streaming event payloads ---

#[derive(Debug, Clone, Serialize)]
struct StreamDelta {
    text: String,
    message_id: String,
}

#[derive(Debug, Clone, Serialize)]
struct StreamDone {
    message_id: String,
    usage: ClaudeUsage,
    tool_calls: Vec<ToolCall>,
    stop_reason: String,
}

#[derive(Debug, Clone, Serialize)]
struct StreamError {
    message_id: String,
    error: String,
}

// --- Tool definitions ---

fn get_tool_definitions() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "add_component".to_string(),
            description: "Add a component to the schematic at a specific position".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "reference_prefix": { "type": "string", "description": "Component prefix (R, C, U, L, D, Q, etc.)" },
                    "value": { "type": "string", "description": "Component value (10k, 100nF, STM32F4, etc.)" },
                    "x": { "type": "number", "description": "X position in mm" },
                    "y": { "type": "number", "description": "Y position in mm" }
                },
                "required": ["reference_prefix", "value", "x", "y"]
            }),
        },
        ToolDef {
            name: "add_wire".to_string(),
            description: "Draw a wire between two points on the schematic".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "start_x": { "type": "number" }, "start_y": { "type": "number" },
                    "end_x": { "type": "number" }, "end_y": { "type": "number" }
                },
                "required": ["start_x", "start_y", "end_x", "end_y"]
            }),
        },
        ToolDef {
            name: "set_component_value".to_string(),
            description: "Change the value of an existing component by its reference designator".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "reference": { "type": "string", "description": "Component reference (R1, C2, U3)" },
                    "value": { "type": "string", "description": "New value" }
                },
                "required": ["reference", "value"]
            }),
        },
        ToolDef {
            name: "add_net_label".to_string(),
            description: "Place a net label at a position on the schematic".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "Net name (VCC, GND, SDA, etc.)" },
                    "x": { "type": "number" }, "y": { "type": "number" }
                },
                "required": ["text", "x", "y"]
            }),
        },
        ToolDef {
            name: "run_erc".to_string(),
            description: "Run Electrical Rules Check and return the results".to_string(),
            input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        },
    ]
}

fn build_system_prompt(context: &SchematicContext) -> String {
    let mut system = String::from(
        "You are Signal, an AI design assistant integrated into Signex EDA. \
         You help hardware engineers with schematic design, component selection, ERC troubleshooting, \
         and circuit analysis.\n\n\
         Guidelines:\n\
         - Be concise and technical.\n\
         - Use specific part numbers and values when suggesting components.\n\
         - Use standard EE terminology and reference designators (R1, C1, U1).\n\
         - Format responses with markdown.\n\
         - When asked to create or modify circuits, use the available tools.\n\n"
    );

    // Design brief (persistent context)
    if let Some(brief) = &context.design_brief {
        if !brief.is_empty() {
            system.push_str(&format!("Design intent: {}\n\n", brief));
        }
    }

    system.push_str("Current schematic:\n");
    system.push_str(&format!("- Title: {}\n", if context.title.is_empty() { "Untitled" } else { &context.title }));
    system.push_str(&format!("- Paper: {}, Components: {}, Wires: {}, Nets: {}\n",
        context.paper_size, context.component_count, context.wire_count, context.net_count));

    if context.erc_errors > 0 || context.erc_warnings > 0 {
        system.push_str(&format!("- ERC: {} errors, {} warnings\n", context.erc_errors, context.erc_warnings));
    }

    if !context.selected_components.is_empty() {
        system.push_str("\nSelected:\n");
        for comp in &context.selected_components {
            system.push_str(&format!("- {} = {} ({})\n", comp.reference, comp.value, comp.footprint));
        }
    }

    // Rich detailed context (net connectivity, component list, ERC details)
    if let Some(detail) = &context.detailed_context {
        if !detail.is_empty() {
            system.push_str("\n");
            system.push_str(detail);
        }
    }

    system
}

fn get_api_key() -> Result<String, String> {
    let guard = API_KEY.lock().unwrap_or_else(|e| e.into_inner());
    guard.clone().ok_or_else(|| "API key not configured. Set it in Signal panel.".to_string())
}

fn build_request(
    messages: &[ChatMessage],
    context: &SchematicContext,
    model: Option<&str>,
    stream: bool,
    enable_tools: bool,
    image_base64: Option<&str>,
) -> ClaudeRequest {
    let system = build_system_prompt(context);

    let claude_messages: Vec<ClaudeApiMessage> = messages
        .iter()
        .enumerate()
        .map(|(i, m)| {
            // If this is the last user message and we have an image, send as multi-content
            if i == messages.len() - 1 && m.role == "user" && image_base64.is_some() {
                ClaudeApiMessage {
                    role: m.role.clone(),
                    content: serde_json::json!([
                        {
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/png",
                                "data": image_base64.unwrap()
                            }
                        },
                        { "type": "text", "text": m.content }
                    ]),
                }
            } else {
                ClaudeApiMessage {
                    role: m.role.clone(),
                    content: serde_json::Value::String(m.content.clone()),
                }
            }
        })
        .collect();

    ClaudeRequest {
        model: model.unwrap_or(DEFAULT_MODEL).to_string(),
        max_tokens: 4096,
        system,
        messages: claude_messages,
        stream: if stream { Some(true) } else { None },
        tools: if enable_tools { Some(get_tool_definitions()) } else { None },
    }
}

// --- Commands ---

#[tauri::command]
pub fn set_api_key(key: String) -> Result<(), String> {
    let mut api_key = API_KEY.lock().unwrap_or_else(|e| e.into_inner());
    *api_key = if key.is_empty() { None } else { Some(key) };
    Ok(())
}

#[tauri::command]
pub fn has_api_key() -> bool {
    let api_key = API_KEY.lock().unwrap_or_else(|e| e.into_inner());
    api_key.is_some()
}

/// Non-streaming chat (kept for backwards compat and simple queries)
#[tauri::command]
pub async fn signal_chat(
    messages: Vec<ChatMessage>,
    context: SchematicContext,
    model: Option<String>,
    image_base64: Option<String>,
) -> Result<SignalResponse, String> {
    let api_key = get_api_key()?;
    let request = build_request(
        &messages, &context,
        model.as_deref(), false, true,
        image_base64.as_deref(),
    );

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Claude API error ({}): {}", status, body));
    }

    let claude_response: ClaudeResponse = response.json().await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let mut message = String::new();
    let mut tool_calls = Vec::new();

    for c in &claude_response.content {
        match c.content_type.as_str() {
            "text" => {
                if let Some(text) = &c.text {
                    message.push_str(text);
                }
            }
            "tool_use" => {
                if let (Some(id), Some(name), Some(input)) = (&c.id, &c.name, &c.input) {
                    tool_calls.push(ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    });
                }
            }
            _ => {}
        }
    }

    Ok(SignalResponse {
        message,
        usage: claude_response.usage,
        tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
        stop_reason: claude_response.stop_reason,
    })
}

/// Streaming chat — emits events as tokens arrive
#[tauri::command]
pub async fn signal_chat_stream(
    app: tauri::AppHandle,
    message_id: String,
    messages: Vec<ChatMessage>,
    context: SchematicContext,
    model: Option<String>,
    image_base64: Option<String>,
) -> Result<(), String> {
    let api_key = get_api_key()?;
    let request = build_request(
        &messages, &context,
        model.as_deref(), true, true,
        image_base64.as_deref(),
    );

    // Spawn the streaming task
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let response = match client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let _ = app.emit("signal:stream-error", StreamError {
                    message_id, error: format!("Network error: {}", e),
                });
                return;
            }
        };

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            let _ = app.emit("signal:stream-error", StreamError {
                message_id, error: format!("API error: {}", body),
            });
            return;
        }

        // Read SSE stream
        let mut buffer = String::new();
        let mut usage = ClaudeUsage { input_tokens: 0, output_tokens: 0 };
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut stop_reason = String::from("end_turn");
        let mut current_tool_id = String::new();
        let mut current_tool_name = String::new();
        let mut current_tool_input = String::new();

        let bytes = response.bytes().await.unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes);

        for line in text.lines() {
            if line.starts_with("data: ") {
                let data = &line[6..];
                if data == "[DONE]" { break; }

                if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
                    let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");

                    match event_type {
                        "content_block_delta" => {
                            if let Some(delta) = event.get("delta") {
                                let delta_type = delta.get("type").and_then(|t| t.as_str()).unwrap_or("");
                                if delta_type == "text_delta" {
                                    if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                        buffer.push_str(text);
                                        let _ = app.emit("signal:stream-delta", StreamDelta {
                                            text: text.to_string(),
                                            message_id: message_id.clone(),
                                        });
                                    }
                                } else if delta_type == "input_json_delta" {
                                    if let Some(partial) = delta.get("partial_json").and_then(|t| t.as_str()) {
                                        current_tool_input.push_str(partial);
                                    }
                                }
                            }
                        }
                        "content_block_start" => {
                            if let Some(cb) = event.get("content_block") {
                                let cb_type = cb.get("type").and_then(|t| t.as_str()).unwrap_or("");
                                if cb_type == "tool_use" {
                                    current_tool_id = cb.get("id").and_then(|t| t.as_str()).unwrap_or("").to_string();
                                    current_tool_name = cb.get("name").and_then(|t| t.as_str()).unwrap_or("").to_string();
                                    current_tool_input.clear();
                                }
                            }
                        }
                        "content_block_stop" => {
                            if !current_tool_name.is_empty() {
                                let input = serde_json::from_str(&current_tool_input).unwrap_or(serde_json::json!({}));
                                tool_calls.push(ToolCall {
                                    id: current_tool_id.clone(),
                                    name: current_tool_name.clone(),
                                    input,
                                });
                                current_tool_name.clear();
                                current_tool_id.clear();
                                current_tool_input.clear();
                            }
                        }
                        "message_delta" => {
                            if let Some(u) = event.get("usage") {
                                if let Some(out) = u.get("output_tokens").and_then(|t| t.as_u64()) {
                                    usage.output_tokens = out as u32;
                                }
                            }
                            if let Some(sr) = event.get("delta").and_then(|d| d.get("stop_reason")).and_then(|s| s.as_str()) {
                                stop_reason = sr.to_string();
                            }
                        }
                        "message_start" => {
                            if let Some(msg) = event.get("message") {
                                if let Some(u) = msg.get("usage") {
                                    if let Some(inp) = u.get("input_tokens").and_then(|t| t.as_u64()) {
                                        usage.input_tokens = inp as u32;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let _ = app.emit("signal:stream-done", StreamDone {
            message_id,
            usage,
            tool_calls,
            stop_reason,
        });
    });

    Ok(())
}

#[tauri::command]
pub async fn signal_review(
    context: SchematicContext,
    model: Option<String>,
) -> Result<SignalResponse, String> {
    let review_message = ChatMessage {
        role: "user".to_string(),
        content: "Review this schematic design. Check for:\n\
            - Missing bypass capacitors\n\
            - Incorrect pull-up/pull-down values\n\
            - Power rail issues\n\
            - Signal integrity concerns\n\
            Provide a brief, actionable review.".to_string(),
    };
    signal_chat(vec![review_message], context, model, None).await
}

#[tauri::command]
pub async fn signal_fix_erc(
    violation_message: String,
    context: SchematicContext,
    model: Option<String>,
) -> Result<SignalResponse, String> {
    let fix_message = ChatMessage {
        role: "user".to_string(),
        content: format!(
            "ERC violation: \"{}\"\n\nExplain the cause and suggest a specific fix. Be concise.",
            violation_message
        ),
    };
    signal_chat(vec![fix_message], context, model, None).await
}
