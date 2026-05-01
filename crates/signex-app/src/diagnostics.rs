use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use anyhow::Result;
use log::{Level, LevelFilter, Log, Metadata, Record, debug, error, info, warn};

const MAX_DIAGNOSTIC_ENTRIES: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Info,
    Debug,
    Trace,
}

impl DiagnosticLevel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warning => "WARN",
            Self::Info => "INFO",
            Self::Debug => "DEBUG",
            Self::Trace => "TRACE",
        }
    }
}

impl From<Level> for DiagnosticLevel {
    fn from(level: Level) -> Self {
        match level {
            Level::Error => Self::Error,
            Level::Warn => Self::Warning,
            Level::Info => Self::Info,
            Level::Debug => Self::Debug,
            Level::Trace => Self::Trace,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiagnosticEntry {
    pub id: u64,
    pub level: DiagnosticLevel,
    pub code: String,
    pub message: String,
}

struct SignexLogger {
    level: LevelFilter,
}

pub fn init_logging() -> Result<()> {
    let level = configured_level();
    log::set_boxed_logger(Box::new(SignexLogger { level }))
        .map_err(|error| anyhow::anyhow!("initialize application logger: {error}"))?;
    log::set_max_level(level);
    info!("Signex logging initialized");
    Ok(())
}

pub fn log_debug(message: impl AsRef<str>) {
    debug!("{}", message.as_ref());
}

pub fn log_info(message: impl AsRef<str>) {
    info!("{}", message.as_ref());
}

pub fn log_warning(message: impl AsRef<str>) {
    warn!("{}", message.as_ref());
}

pub fn log_error(context: &str, error: &anyhow::Error) {
    error!("{context}: {error:#}");
}

pub fn recent_entries() -> Vec<DiagnosticEntry> {
    entries()
        .lock()
        .expect("diagnostics mutex poisoned")
        .iter()
        .cloned()
        .collect()
}

pub fn configured_level() -> LevelFilter {
    *CONFIGURED_LEVEL.get_or_init(resolve_configured_level)
}

pub fn configured_level_label() -> &'static str {
    match configured_level() {
        LevelFilter::Off => "off",
        LevelFilter::Error => "error",
        LevelFilter::Warn => "warn",
        LevelFilter::Info => "info",
        LevelFilter::Debug => "debug",
        LevelFilter::Trace => "trace",
    }
}

impl Log for SignexLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        self.level >= metadata.level().to_level_filter()
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let rendered = format!("{}", record.args());
        let (code, message) = summarize_record(record.target(), &rendered);
        push_entry(DiagnosticEntry {
            id: NEXT_ENTRY_ID.fetch_add(1, Ordering::Relaxed),
            level: DiagnosticLevel::from(record.level()),
            code,
            message,
        });

        eprintln!("[{}] {rendered}", record.level());
    }

    fn flush(&self) {}
}

static CONFIGURED_LEVEL: OnceLock<LevelFilter> = OnceLock::new();
static DIAGNOSTIC_ENTRIES: OnceLock<Mutex<VecDeque<DiagnosticEntry>>> = OnceLock::new();
static NEXT_ENTRY_ID: AtomicU64 = AtomicU64::new(1);

fn entries() -> &'static Mutex<VecDeque<DiagnosticEntry>> {
    DIAGNOSTIC_ENTRIES.get_or_init(|| Mutex::new(VecDeque::with_capacity(MAX_DIAGNOSTIC_ENTRIES)))
}

fn push_entry(entry: DiagnosticEntry) {
    let mut entries = entries().lock().expect("diagnostics mutex poisoned");
    if entries.len() == MAX_DIAGNOSTIC_ENTRIES {
        entries.pop_front();
    }
    entries.push_back(entry);
}

fn resolve_configured_level() -> LevelFilter {
    ["SIGNEX_LOG", "RUST_LOG"]
        .into_iter()
        .find_map(|key| {
            std::env::var(key)
                .ok()
                .and_then(|value| parse_level_filter(&value))
        })
        .unwrap_or(LevelFilter::Info)
}

fn parse_level_filter(value: &str) -> Option<LevelFilter> {
    let mut global_level = None;

    for directive in value
        .split(',')
        .map(str::trim)
        .filter(|directive| !directive.is_empty())
    {
        if let Some((target, level)) = directive.split_once('=') {
            let target = target.trim();
            if matches!(target, "signex" | "signex_app") {
                return parse_level(level.trim());
            }
            continue;
        }

        if global_level.is_none() {
            global_level = parse_level(directive);
        }
    }

    global_level
}

fn parse_level(level: &str) -> Option<LevelFilter> {
    match level.trim().to_ascii_lowercase().as_str() {
        "off" => Some(LevelFilter::Off),
        "error" => Some(LevelFilter::Error),
        "warn" | "warning" => Some(LevelFilter::Warn),
        "info" => Some(LevelFilter::Info),
        "debug" => Some(LevelFilter::Debug),
        "trace" => Some(LevelFilter::Trace),
        _ => None,
    }
}

fn summarize_record(target: &str, rendered: &str) -> (String, String) {
    if let Some(summary) = summarize_graphics_record(rendered) {
        return summary;
    }

    let code = diagnostic_code_from_target(target);
    let message = compact_message(rendered);
    (code, message)
}

fn summarize_graphics_record(rendered: &str) -> Option<(String, String)> {
    if rendered.contains("Selected: AdapterInfo") {
        let adapter =
            extract_named_field(rendered, "name").unwrap_or_else(|| "Unknown adapter".to_string());
        let backend = extract_named_field(rendered, "backend")
            .unwrap_or_else(|| "Unknown backend".to_string());
        return Some((
            "GPU-ADAPTER-SELECTED".to_string(),
            format!("Graphics adapter selected: {adapter} ({backend})"),
        ));
    }

    if rendered.contains("Available formats:") {
        let formats = extract_bracket_items(rendered);
        let preview = join_preview(&formats, 4);
        let suffix = if formats.len() > 4 { "..." } else { "" };
        return Some((
            "GPU-SURFACE-FORMATS".to_string(),
            format!("Surface formats available: {}{}", preview, suffix),
        ));
    }

    if rendered.contains("Available alpha modes:") {
        let modes = extract_bracket_items(rendered);
        let preview = join_preview(&modes, 4);
        return Some((
            "GPU-ALPHA-MODES".to_string(),
            format!("Surface alpha modes available: {preview}"),
        ));
    }

    None
}

fn diagnostic_code_from_target(target: &str) -> String {
    let normalized = target
        .split("::")
        .flat_map(|part| part.split(':'))
        .filter(|part| !part.is_empty())
        .map(|part| part.replace(|ch: char| !ch.is_ascii_alphanumeric(), "_"))
        .map(|part| part.to_ascii_uppercase())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    if normalized.is_empty() {
        "APP-EVENT".to_string()
    } else {
        normalized.join("-")
    }
}

fn compact_message(rendered: &str) -> String {
    let compact = rendered.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.len() <= 160 {
        compact
    } else {
        format!("{}...", &compact[..157])
    }
}

fn extract_named_field(rendered: &str, field_name: &str) -> Option<String> {
    let marker = format!("{field_name}: ");
    let start = rendered.find(&marker)? + marker.len();
    let rest = &rendered[start..];
    if let Some(stripped) = rest.strip_prefix('"') {
        let end = stripped.find('"')?;
        return Some(stripped[..end].to_string());
    }
    let end = rest.find([',', '\n', '}']).unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}

fn extract_bracket_items(rendered: &str) -> Vec<String> {
    let Some(start) = rendered.find('[') else {
        return Vec::new();
    };
    let Some(end) = rendered.rfind(']') else {
        return Vec::new();
    };

    rendered[start + 1..end]
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn join_preview(items: &[String], max_items: usize) -> String {
    items
        .iter()
        .take(max_items)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ")
}
