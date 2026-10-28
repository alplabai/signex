use crate::command::CommandKind;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("engine document path is not set")]
    MissingPath,
    #[error("command {0:?} is not implemented yet")]
    UnsupportedCommand(CommandKind),
    #[error("a sheet symbol needs a target filename")]
    EmptyChildSheetFilename,
    #[error(
        "library symbol update refused, connected pins would be lost or moved: {}; remove those wires or no-connects first",
        pins.join(", ")
    )]
    LibSymbolPinsConnected { lib_id: String, pins: Vec<String> },
    #[error(
        "library symbol update refused, the library symbol no longer has the unit placed as: {}; delete or re-unit those parts first",
        parts.join(", ")
    )]
    LibSymbolUnitRemoved { lib_id: String, parts: Vec<String> },
    #[error("a sheet symbol needs a name")]
    EmptyChildSheetName,
    #[error("sheet file '{0}' must end in .snxsch")]
    InvalidChildSheetFilename(String),
    #[error("another sheet symbol is already named '{0}'")]
    DuplicateChildSheetName(String),
    #[error("another sheet symbol already points at '{0}'")]
    DuplicateChildSheetFile(String),
    #[error("a sheet entry needs a name")]
    EmptySheetEntryName,
    #[error("this sheet symbol already has an entry named '{0}'")]
    DuplicateSheetEntryName(String),
    #[error("failed to open schematic: {0}")]
    OpenFailed(#[source] anyhow::Error),
    #[error("designator counter for prefix {prefix:?} would pass u32::MAX; annotate refused")]
    DesignatorOverflow { prefix: String },
    #[error("failed to save schematic: {0}")]
    SaveFailed(#[source] std::io::Error),
    #[error("this symbol has no stored {0} text, so it cannot take a font family")]
    NoTextToStyle(&'static str),
    #[error(
        "rotate / mirror refused, the selection holds {what}, which cannot be rotated or mirrored yet; deselect it and try again"
    )]
    SelectionTransformUnsupported { what: String },
    #[error("{}", locked_message(references, action))]
    SymbolLocked {
        references: Vec<String>,
        action: &'static str,
    },
    #[error("an unrecorded edit is only allowed on a freshly loaded sheet with no undo history")]
    UnrecordedEditRefused,
}

fn locked_message(references: &[String], action: &str) -> String {
    match references {
        [one] => format!("{one} is locked and cannot be {action}; unlock it in Properties first"),
        many => format!(
            "{} are locked and cannot be {action}; unlock them in Properties first",
            many.join(", ")
        ),
    }
}
