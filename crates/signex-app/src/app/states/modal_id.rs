#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum ModalId {
    AnnotateDialog,
    AnnotateResetConfirm,
    ErcDialog,
    /// Altium-style Move-Selection dialog — numeric DeltaX / DeltaY inputs.
    MoveSelection,
    /// F5 net-color palette.
    NetColorPalette,
    /// Parameter manager — bulk parameter editor.
    ParameterManager,
    // Reserved for future draggable modals — wired in when each dialog's
    // header gets a drag hook.
    Preferences,
    FindReplace,
    /// Rename-sheet dialog (Projects-panel leaf -> Rename...).
    RenameDialog,
    /// Remove-from-project dialog (Projects-panel leaf -> Remove from Project).
    RemoveDialog,
    /// Print Preview / Export PDF unified modal (File -> Print Preview, File -> Export PDF).
    PrintPreview,
    /// BOM Export preview modal (File -> Export -> Bill of Materials...).
    BomPreview,
}
