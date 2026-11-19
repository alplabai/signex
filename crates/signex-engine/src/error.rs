use crate::command::CommandKind;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("engine document path is not set")]
    MissingPath,
    #[error("command {0:?} is not implemented yet")]
    UnsupportedCommand(CommandKind),
    #[error("{0}")]
    LibSymbolBodyCollision(#[from] signex_types::schematic::BodyRefCollision),
    #[error("a sheet symbol needs a target filename")]
    EmptyChildSheetFilename,
    #[error(
        "library symbol update refused, connected pins would be lost or moved: {}; remove those wires or no-connects first",
        pins.join(", ")
    )]
    LibSymbolPinsConnected { lib_id: String, pins: Vec<String> },
    #[error(
        "body switch refused for {reference}, connected pins would be lost, moved or land on a connection: {}; remove those wires or no-connects first",
        pins.join(", ")
    )]
    SymbolBodyPinsConnected {
        reference: String,
        pins: Vec<String>,
    },
    #[error(
        "library symbol update refused, the library symbol no longer has the unit placed as: {}; delete or re-unit those parts first",
        parts.join(", ")
    )]
    LibSymbolUnitRemoved { lib_id: String, parts: Vec<String> },
    #[error(
        "component change refused, the new part has no drawing for: {}; pick a part that has those units",
        parts.join(", ")
    )]
    ChangeComponentUnitMissing { parts: Vec<String> },
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
    #[error(
        "a sheet symbol's width and height must each be a finite number of at least {min_mm} mm, got {width} x {height}"
    )]
    InvalidChildSheetSize {
        width: f64,
        height: f64,
        min_mm: f64,
    },
    #[error(
        "sheet symbol '{sheet}' cannot take that size, these entries would fall off its border: {}; move them first or choose a larger size",
        entries.join(", ")
    )]
    ChildSheetEntriesOffBorder { sheet: String, entries: Vec<String> },
    #[error(
        "sheet symbol '{sheet}' cannot take that size, these entries are connected or would land on a connection: {}; remove those wires first",
        entries.join(", ")
    )]
    ChildSheetEntriesConnected { sheet: String, entries: Vec<String> },
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
    #[error("the selection holds a coordinate that is not a finite number; it was not copied")]
    NonFiniteCoordinate,
    #[error("a wire width must be a finite number of at least 0 mm, got {0}")]
    InvalidWireWidth(f64),
    #[error(
        "the wire cannot be broken there: a pin, bus end or another wire's end sits on that spot, and a new wire end would join it"
    )]
    BreakWireBlocked,
    #[error("an object with id {0} is already in the sheet; the new object needs a fresh id")]
    IdInUse(uuid::Uuid),
    #[error("{}", locked_message(references, action))]
    ItemLocked {
        references: Vec<String>,
        action: &'static str,
    },
    #[error("an unrecorded edit is only allowed on a freshly loaded sheet with no undo history")]
    UnrecordedEditRefused,
    #[error("this sheet was written by a newer Signex and is read-only; edit it in that version")]
    ReadOnly,
    #[error("a transaction or batch is already running on this sheet")]
    TransactionOpen,
    #[error("no transaction is open on this sheet")]
    NoTransaction,
    #[error("a command inside the transaction failed, so it can only be rolled back")]
    TransactionFailed,
    #[error("the clipboard fragment could not be converted: {0}")]
    ClipboardFragment(String),
    #[error("no {kind} directive {id} is on this sheet")]
    DirectiveNotFound { kind: &'static str, id: uuid::Uuid },
    #[error("a directive region needs two different corners with finite coordinates")]
    InvalidDirectiveRegion,
    #[error("no harness {kind} {id} is on this sheet")]
    HarnessNotFound { kind: &'static str, id: uuid::Uuid },
    #[error(
        "a harness connector needs a type name, a finite place, a width and entry pitch above 0 mm and a quarter turn"
    )]
    InvalidHarnessConnector,
    #[error("a harness wire needs two different ends with finite coordinates")]
    InvalidHarnessWire,
    #[error("a harness entry needs a name")]
    EmptyHarnessEntryName,
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
