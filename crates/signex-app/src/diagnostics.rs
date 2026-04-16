use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use simplelog::{ColorChoice, CombinedLogger, Config, LevelFilter, SharedLogger, TermLogger, TerminalMode};

pub fn init_logging() -> Result<()> {
    let loggers: Vec<Box<dyn SharedLogger>> = vec![TermLogger::new(
        LevelFilter::Debug,
        Config::default(),
        TerminalMode::Stderr,
        ColorChoice::Auto,
    )];

    CombinedLogger::init(loggers).context("initialize application logger")?;
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