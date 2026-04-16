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
        push_entry(DiagnosticEntry {
            id: NEXT_ENTRY_ID.fetch_add(1, Ordering::Relaxed),
            level: DiagnosticLevel::from(record.level()),
            message: rendered.clone(),
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
        .find_map(|key| std::env::var(key).ok().and_then(|value| parse_level_filter(&value)))
        .unwrap_or(LevelFilter::Info)
}

fn parse_level_filter(value: &str) -> Option<LevelFilter> {
    let mut global_level = None;

    for directive in value.split(',').map(str::trim).filter(|directive| !directive.is_empty()) {
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