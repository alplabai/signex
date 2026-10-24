use crate::command::CommandKind;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("engine document path is not set")]
    MissingPath,
    #[error("command {0:?} is not implemented yet")]
    UnsupportedCommand(CommandKind),
    #[error("cannot place a child sheet with no target filename")]
    EmptyChildSheetFilename,
    #[error("failed to open schematic: {0}")]
    OpenFailed(#[source] anyhow::Error),
    #[error("designator counter for prefix {prefix:?} would pass u32::MAX; annotate refused")]
    DesignatorOverflow { prefix: String },
    #[error("failed to save schematic: {0}")]
    SaveFailed(#[source] std::io::Error),
}
