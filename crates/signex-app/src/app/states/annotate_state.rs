pub struct AnnotateState {
    /// Annotate dialog open flag. When true, the Annotate-Schematics modal
    /// covers the canvas with its preview + confirm-apply UI.
    pub dialog_open: bool,
    /// Annotate dialog: order-of-processing choice. Controls the iteration
    /// order used to assign sequential numbers.
    pub order: super::AnnotateOrder,
    /// Reset-and-renumber confirmation modal. When true, the Design ->
    /// Reset menu item shows a confirm before discarding every number.
    pub reset_confirm: bool,
    /// Symbols whose designator the user locked against reannotation.
    /// Exposed as per-row checkboxes in the Annotate dialog; the engine
    /// skips these uuids in `annotate_with_seed_and_locks`.
    pub locked: std::collections::HashSet<uuid::Uuid>,
}
