use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::Emitter;

// Claude API configuration — supports two modes:
// 1. User's own API key (direct to Anthropic)
// 2. Signex backend proxy (future: managed billing via signex.pro)
//
// SECURITY: API key held as plaintext String in process memory.
// For distribution, use OS keychain (tauri-plugin-stronghold).
static API_KEY: std::sync::LazyLock<Mutex<Option<String>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

// Signex Pro backend URL (when using managed API key)
static SIGNEX_BACKEND: std::sync::LazyLock<Mutex<Option<String>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

// Shared HTTP client — reuses connection pool, TLS sessions, DNS cache
static HTTP_CLIENT: std::sync::LazyLock<reqwest::Client> =
    std::sync::LazyLock::new(reqwest::Client::new);

const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";
const MAX_DESIGN_BRIEF_CHARS: usize = 500;
const MAX_VIOLATION_MSG_CHARS: usize = 500;
const MAX_TOOL_INPUT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "user" | "assistant"
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

#[derive(Debug, Serialize)]
struct ClaudeApiMessage {
    role: String,
    content: serde_json::Value,
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

// --- Tool definitions (cached) ---

static TOOL_DEFINITIONS: std::sync::LazyLock<Vec<ToolDef>> =
    std::sync::LazyLock::new(|| {
        vec![
            ToolDef {
                name: "add_component".into(),
                description: "Add a component to the schematic at a specific position".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "reference_prefix": { "type": "string", "description": "Component prefix (R, C, U, L, D, Q)" },
                        "value": { "type": "string", "description": "Component value (10k, 100nF, STM32F4)" },
                        "x": { "type": "number", "description": "X position in mm" },
                        "y": { "type": "number", "description": "Y position in mm" }
                    },
                    "required": ["reference_prefix", "value", "x", "y"]
                }),
            },
            ToolDef {
                name: "add_wire".into(),
                description: "Draw a wire between two points".into(),
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
                name: "set_component_value".into(),
                description: "Change the value of an existing component by reference".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "reference": { "type": "string" },
                        "value": { "type": "string" }
                    },
                    "required": ["reference", "value"]
                }),
            },
            ToolDef {
                name: "add_net_label".into(),
                description: "Place a net label at a position".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string" },
                        "x": { "type": "number" }, "y": { "type": "number" }
                    },
                    "required": ["text", "x", "y"]
                }),
            },
            ToolDef {
                name: "run_erc".into(),
                description: "Run Electrical Rules Check and return results".into(),
                input_schema: serde_json::json!({ "type": "object", "properties": {} }),
            },
        ]
    });

fn build_system_prompt(context: &SchematicContext) -> String {
    let mut system = String::from(
        "You are Signal, an expert AI hardware design assistant integrated into Signex EDA. \
         You are an electronics engineering expert with deep knowledge of:\n\
         - Analog and digital circuit design\n\
         - Power supply design (LDO, buck/boost, SEPIC, charge pumps)\n\
         - Microcontroller systems (STM32, ESP32, nRF, PIC, AVR, RISC-V)\n\
         - Communication interfaces (UART, SPI, I2C, CAN, USB, Ethernet, PCIe)\n\
         - RF and high-speed design (impedance matching, termination, signal integrity)\n\
         - Sensor integration (ADC, DAC, op-amp signal conditioning)\n\
         - PCB design best practices (stackup, routing, EMC/EMI)\n\
         - Manufacturing constraints (DFM, DFA, component availability)\n\
         - Component selection (Digi-Key, Mouser, LCSC, JLCPCB catalogs)\n\n\
         Guidelines:\n\
         - Be concise, technical, and actionable. Hardware engineers don't need hand-holding.\n\
         - Always suggest specific part numbers (e.g., 'TPS63020DSJR' not just 'buck converter').\n\
         - Include critical design values: exact resistor values with tolerance, capacitor ESR, inductor DCR.\n\
         - Use standard reference designators (R1, C1, U1, L1, D1, Q1, J1, TP1).\n\
         - When reviewing designs, prioritize: power integrity > signal integrity > EMC > manufacturability.\n\
         - For bypass caps: 100nF ceramic (0402/0603) per power pin, 10uF bulk per rail, place within 2mm of pin.\n\
         - For pull-ups: I2C standard = 4.7k for 100kHz / 2.2k for 400kHz. GPIO = 10k-100k.\n\
         - For ESD protection: TVS diodes on all external connectors, series resistors on sensitive inputs.\n\
         - Always consider: input voltage range, output current, thermal dissipation, package availability.\n\
         - Format responses with markdown. Use tables for component comparisons.\n\
         - When asked to create or modify circuits, use the available tools.\n\n"
    );

    // Design brief (persistent context, length-limited)
    if let Some(brief) = &context.design_brief {
        if !brief.is_empty() {
            let truncated: String = brief.chars().take(MAX_DESIGN_BRIEF_CHARS).collect();
            system.push_str(&format!("Design intent: {}\n\n", truncated));
        }
    }

    // Schematic data section — clearly delimited as untrusted data
    system.push_str("--- BEGIN SCHEMATIC DATA ---\n");
    system.push_str(&format!(
        "Title: {}\nPaper: {}, Components: {}, Wires: {}, Nets: {}\n",
        if context.title.is_empty() { "Untitled" } else { &context.title },
        context.paper_size, context.component_count, context.wire_count, context.net_count
    ));

    if context.erc_errors > 0 || context.erc_warnings > 0 {
        system.push_str(&format!(
            "ERC: {} errors, {} warnings\n",
            context.erc_errors, context.erc_warnings
        ));
    }

    if !context.selected_components.is_empty() {
        system.push_str("Selected:\n");
        for comp in &context.selected_components {
            system.push_str(&format!(
                "  {} = {} ({})\n",
                comp.reference, comp.value, comp.footprint
            ));
        }
    }

    if let Some(detail) = &context.detailed_context {
        if !detail.is_empty() {
            system.push('\n');
            system.push_str(detail);
            system.push('\n');
        }
    }

    system.push_str("--- END SCHEMATIC DATA ---\n");
    system
}

/// Get API endpoint and key — supports direct Anthropic or Signex proxy
fn get_api_config() -> Result<(String, String), String> {
    // Check Signex backend first (managed mode)
    let backend = SIGNEX_BACKEND.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(url) = backend.as_ref() {
        // Signex backend handles auth — use a session token or empty key
        return Ok((format!("{}/v1/messages", url), String::new()));
    }

    // Direct Anthropic API with user's key
    let guard = API_KEY.lock().unwrap_or_else(|e| e.into_inner());
    let key = guard
        .clone()
        .ok_or_else(|| "API key not configured. Set it in Signal panel or connect to Signex Pro.".to_string())?;
    Ok(("https://api.anthropic.com/v1/messages".to_string(), key))
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
            let is_last_user = i == messages.len() - 1 && m.role == "user";
            if is_last_user {
                if let Some(img) = image_base64 {
                    return ClaudeApiMessage {
                        role: m.role.clone(),
                        content: serde_json::json!([
                            {
                                "type": "image",
                                "source": {
                                    "type": "base64",
                                    "media_type": "image/png",
                                    "data": img
                                }
                            },
                            { "type": "text", "text": m.content }
                        ]),
                    };
                }
            }
            ClaudeApiMessage {
                role: m.role.clone(),
                content: serde_json::Value::String(m.content.clone()),
            }
        })
        .collect();

    ClaudeRequest {
        model: model.unwrap_or(DEFAULT_MODEL).to_string(),
        max_tokens: 4096,
        system,
        messages: claude_messages,
        stream: if stream { Some(true) } else { None },
        tools: if enable_tools {
            Some(TOOL_DEFINITIONS.iter().map(|t| ToolDef {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.input_schema.clone(),
            }).collect())
        } else {
            None
        },
    }
}

// --- Commands ---

#[tauri::command]
pub fn set_api_key(key: String) -> Result<(), String> {
    let mut api_key = API_KEY.lock().unwrap_or_else(|e| e.into_inner());
    *api_key = if key.is_empty() { None } else { Some(key) };
    Ok(())
}

/// Set the Signex Pro backend URL for managed API access
#[tauri::command]
pub fn set_signex_backend(url: String) -> Result<(), String> {
    let mut backend = SIGNEX_BACKEND.lock().unwrap_or_else(|e| e.into_inner());
    *backend = if url.is_empty() { None } else { Some(url) };
    Ok(())
}

#[tauri::command]
pub fn has_api_key() -> bool {
    let api_key = API_KEY.lock().unwrap_or_else(|e| e.into_inner());
    let backend = SIGNEX_BACKEND.lock().unwrap_or_else(|e| e.into_inner());
    api_key.is_some() || backend.is_some()
}

/// Get the API mode: "user_key", "signex_backend", or "none"
#[tauri::command]
pub fn get_api_mode() -> String {
    let backend = SIGNEX_BACKEND.lock().unwrap_or_else(|e| e.into_inner());
    if backend.is_some() { return "signex_backend".to_string(); }
    let api_key = API_KEY.lock().unwrap_or_else(|e| e.into_inner());
    if api_key.is_some() { return "user_key".to_string(); }
    "none".to_string()
}

/// Non-streaming chat
#[tauri::command]
pub async fn signal_chat(
    messages: Vec<ChatMessage>,
    context: SchematicContext,
    model: Option<String>,
    image_base64: Option<String>,
) -> Result<SignalResponse, String> {
    let (api_url, api_key) = get_api_config()?;
    let request = build_request(
        &messages,
        &context,
        model.as_deref(),
        false,
        true,
        image_base64.as_deref(),
    );

    let response = HTTP_CLIENT
        .post(&api_url)
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

    let claude_response: ClaudeResponse = response
        .json()
        .await
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
        tool_calls: if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        },
        stop_reason: claude_response.stop_reason,
    })
}

/// Streaming chat — emits Tauri events as tokens arrive
#[tauri::command]
pub async fn signal_chat_stream(
    app: tauri::AppHandle,
    message_id: String,
    messages: Vec<ChatMessage>,
    context: SchematicContext,
    model: Option<String>,
    image_base64: Option<String>,
) -> Result<(), String> {
    let (api_url, api_key) = get_api_config()?;
    let request = build_request(
        &messages,
        &context,
        model.as_deref(),
        true,
        true,
        image_base64.as_deref(),
    );

    tokio::spawn(async move {
        let response = match HTTP_CLIENT
            .post(&api_url)
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let _ = app.emit(
                    "signal:stream-error",
                    StreamError {
                        message_id,
                        error: format!("Network error: {}", e),
                    },
                );
                return;
            }
        };

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            let _ = app.emit(
                "signal:stream-error",
                StreamError {
                    message_id,
                    error: format!("API error: {}", body),
                },
            );
            return;
        }

        // Read response body with error handling
        let bytes = match response.bytes().await {
            Ok(b) => b,
            Err(e) => {
                let _ = app.emit(
                    "signal:stream-error",
                    StreamError {
                        message_id,
                        error: format!("Failed to read stream: {}", e),
                    },
                );
                return;
            }
        };
        let text = String::from_utf8_lossy(&bytes);

        // Parse SSE events
        let mut usage = ClaudeUsage {
            input_tokens: 0,
            output_tokens: 0,
        };
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut stop_reason = String::new();
        let mut current_tool_id = String::new();
        let mut current_tool_name = String::new();
        let mut current_tool_input = String::new();

        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
                    let event_type = event
                        .get("type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("");

                    match event_type {
                        "content_block_delta" => {
                            if let Some(delta) = event.get("delta") {
                                let delta_type =
                                    delta.get("type").and_then(|t| t.as_str()).unwrap_or("");
                                if delta_type == "text_delta" {
                                    if let Some(t) = delta.get("text").and_then(|t| t.as_str()) {
                                        let _ = app.emit(
                                            "signal:stream-delta",
                                            StreamDelta {
                                                text: t.to_string(),
                                                message_id: message_id.clone(),
                                            },
                                        );
                                    }
                                } else if delta_type == "input_json_delta" {
                                    if let Some(partial) =
                                        delta.get("partial_json").and_then(|t| t.as_str())
                                    {
                                        if current_tool_input.len() + partial.len()
                                            < MAX_TOOL_INPUT_BYTES
                                        {
                                            current_tool_input.push_str(partial);
                                        }
                                    }
                                }
                            }
                        }
                        "content_block_start" => {
                            if let Some(cb) = event.get("content_block") {
                                if cb.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                                    current_tool_id = cb
                                        .get("id")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    current_tool_name = cb
                                        .get("name")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    current_tool_input.clear();
                                }
                            }
                        }
                        "content_block_stop" => {
                            if !current_tool_name.is_empty() {
                                let input = serde_json::from_str(&current_tool_input)
                                    .unwrap_or(serde_json::json!({}));
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
                                if let Some(out) = u.get("output_tokens").and_then(|t| t.as_u64())
                                {
                                    usage.output_tokens = out as u32;
                                }
                            }
                            if let Some(sr) = event
                                .get("delta")
                                .and_then(|d| d.get("stop_reason"))
                                .and_then(|s| s.as_str())
                            {
                                stop_reason = sr.to_string();
                            }
                        }
                        "message_start" => {
                            if let Some(msg) = event.get("message") {
                                if let Some(u) = msg.get("usage") {
                                    if let Some(inp) =
                                        u.get("input_tokens").and_then(|t| t.as_u64())
                                    {
                                        usage.input_tokens = inp as u32;
                                    }
                                }
                            }
                        }
                        "message_stop" => {
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }

        if stop_reason.is_empty() {
            stop_reason = "end_turn".to_string();
        }

        let _ = app.emit(
            "signal:stream-done",
            StreamDone {
                message_id,
                usage,
                tool_calls,
                stop_reason,
            },
        );
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
            Provide a brief, actionable review."
            .to_string(),
    };
    signal_chat(vec![review_message], context, model, None).await
}

#[tauri::command]
pub async fn signal_fix_erc(
    violation_message: String,
    context: SchematicContext,
    model: Option<String>,
) -> Result<SignalResponse, String> {
    let msg: String = violation_message.chars().take(MAX_VIOLATION_MSG_CHARS).collect();
    let fix_message = ChatMessage {
        role: "user".to_string(),
        content: format!(
            "ERC violation: \"{}\"\n\nExplain the cause and suggest a specific fix. Be concise.",
            msg
        ),
    };
    signal_chat(vec![fix_message], context, model, None).await
}
