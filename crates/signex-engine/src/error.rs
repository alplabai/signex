use crate::command::CommandKind;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("engine document path is not set")]
    MissingPath,
    #[error("command {0:?} is not implemented yet")]
    UnsupportedCommand(CommandKind),
    #[error("failed to open schematic: {0}")]
    OpenFailed(#[source] anyhow::Error),
    #[error("failed to save schematic: {0}")]
    SaveFailed(#[source] std::io::Error),
}