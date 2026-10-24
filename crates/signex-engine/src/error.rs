use crate::command::CommandKind;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("engine document path is not set")]
    MissingPath,
    #[error("command {0:?} is not implemented yet")]
    UnsupportedCommand(CommandKind),
    #[error("a sheet symbol needs a target filename")]
    EmptyChildSheetFilename,
    #[error("a sheet symbol needs a name")]
    EmptyChildSheetName,
    #[error("sheet file '{0}' must end in .snxsch")]
    InvalidChildSheetFilename(String),
    #[error("another sheet symbol is already named '{0}'")]
    DuplicateChildSheetName(String),
    #[error("another sheet symbol already points at '{0}'")]
    DuplicateChildSheetFile(String),
    #[error("failed to open schematic: {0}")]
    OpenFailed(#[source] anyhow::Error),
    #[error("failed to save schematic: {0}")]
    SaveFailed(#[source] std::io::Error),
}
