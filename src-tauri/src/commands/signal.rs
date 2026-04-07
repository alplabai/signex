use serde::{Deserialize, Serialize};
use std::sync::Mutex;

// Claude API configuration
static API_KEY: std::sync::LazyLock<Mutex<Option<String>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,    // "user" | "assistant"
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectedComponent {
    pub reference: String,
    pub value: String,
    pub footprint: String,
    pub lib_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<ClaudeMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContent>,
    #[allow(dead_code)]
    model: String,
    #[allow(dead_code)]
    stop_reason: Option<String>,
    usage: ClaudeUsage,
}

#[derive(Debug, Deserialize)]
struct ClaudeContent {
    #[allow(dead_code)]
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClaudeUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignalResponse {
    pub message: String,
    pub usage: ClaudeUsage,
}

fn build_system_prompt(context: &SchematicContext) -> String {
    let mut system = String::from(
        "You are Signal, an AI design assistant integrated into Signex EDA (Electronic Design Automation). \
         You help hardware engineers with schematic design, component selection, ERC troubleshooting, \
         and circuit analysis. You are knowledgeable about electronics, PCB design, and KiCad file formats.\n\n\
         Guidelines:\n\
         - Be concise and technical. Hardware engineers don't need hand-holding.\n\
         - When suggesting components, include specific part numbers and values.\n\
         - When explaining circuits, use standard EE terminology.\n\
         - Reference designators (R1, C1, U1) when discussing specific components.\n\
         - If asked about ERC violations, explain the root cause and suggest fixes.\n\
         - Format responses with markdown for readability.\n\n"
    );

    system.push_str("Current schematic context:\n");
    system.push_str(&format!("- Title: {}\n", if context.title.is_empty() { "Untitled" } else { &context.title }));
    system.push_str(&format!("- Paper: {}\n", context.paper_size));
    system.push_str(&format!("- Components: {}, Wires: {}, Nets: {}\n",
        context.component_count, context.wire_count, context.net_count));

    if context.erc_errors > 0 || context.erc_warnings > 0 {
        system.push_str(&format!("- ERC: {} errors, {} warnings\n",
            context.erc_errors, context.erc_warnings));
    }

    if !context.selected_components.is_empty() {
        system.push_str("\nSelected components:\n");
        for comp in &context.selected_components {
            system.push_str(&format!("- {} = {} ({})\n",
                comp.reference, comp.value, comp.footprint));
        }
    }

    system
}

/// Set the Claude API key
#[tauri::command]
pub fn set_api_key(key: String) -> Result<(), String> {
    let mut api_key = API_KEY.lock().unwrap_or_else(|e| e.into_inner());
    *api_key = if key.is_empty() { None } else { Some(key) };
    Ok(())
}

/// Check if API key is configured
#[tauri::command]
pub fn has_api_key() -> bool {
    let api_key = API_KEY.lock().unwrap_or_else(|e| e.into_inner());
    api_key.is_some()
}

/// Send a message to Claude and get a response
#[tauri::command]
pub async fn signal_chat(
    messages: Vec<ChatMessage>,
    context: SchematicContext,
) -> Result<SignalResponse, String> {
    let api_key = {
        let guard = API_KEY.lock().unwrap_or_else(|e| e.into_inner());
        guard.clone().ok_or_else(|| "API key not configured. Set it in Preferences > Signal AI.".to_string())?
    };

    let system = build_system_prompt(&context);

    let claude_messages: Vec<ClaudeMessage> = messages
        .iter()
        .map(|m| ClaudeMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();

    let request = ClaudeRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        max_tokens: 4096,
        system,
        messages: claude_messages,
    };

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

    let claude_response: ClaudeResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let message = claude_response
        .content
        .iter()
        .filter_map(|c| c.text.as_ref())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");

    Ok(SignalResponse {
        message,
        usage: claude_response.usage,
    })
}

/// Quick analysis: send schematic context and get design review
#[tauri::command]
pub async fn signal_review(
    context: SchematicContext,
) -> Result<SignalResponse, String> {
    let review_message = ChatMessage {
        role: "user".to_string(),
        content: "Review this schematic design. Check for common issues like:\n\
            - Missing bypass capacitors\n\
            - Incorrect pull-up/pull-down values\n\
            - Power rail issues\n\
            - Signal integrity concerns\n\
            - Component selection suggestions\n\
            Provide a brief, actionable review.".to_string(),
    };

    signal_chat(vec![review_message], context).await
}

/// Quick fix: suggest fix for a specific ERC violation
#[tauri::command]
pub async fn signal_fix_erc(
    violation_message: String,
    context: SchematicContext,
) -> Result<SignalResponse, String> {
    let fix_message = ChatMessage {
        role: "user".to_string(),
        content: format!(
            "I have this ERC violation: \"{}\"\n\nExplain what's wrong and suggest a specific fix. \
             Be concise — just the cause and the fix.",
            violation_message
        ),
    };

    signal_chat(vec![fix_message], context).await
}
