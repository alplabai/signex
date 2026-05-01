pub struct ErcState {
    /// ERC results for the currently-visible sheet. Driven by the
    /// per-sheet cache below — switching tabs repoints this at the
    /// cached violations for that sheet, so markers and the Messages
    /// panel always match what's on the canvas.
    pub violations: Vec<signex_erc::Violation>,
    /// Per-sheet ERC violation cache, keyed by the sheet's on-disk
    /// file path. Run ERC populates this for every sheet in the
    /// project; tab switches point `violations` at the matching
    /// entry without rerunning the analysis.
    pub violations_by_path:
        std::collections::HashMap<std::path::PathBuf, Vec<signex_erc::Violation>>,
    /// Global cursor into the flattened ERC diagnostics list spanning all
    /// sheets in `violations_by_path`. Used by next/prev navigation.
    pub focus_global_index: Option<usize>,
    /// Per-rule severity override — if empty, the rule's default is used.
    pub severity_override:
        std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
    /// ERC dialog open flag — opens the full severity-matrix + pin-matrix UI.
    pub dialog_open: bool,
    /// Pin-connection matrix overrides — sparse map keyed by (row, col)
    /// pin-type index. Any entry present replaces the default severity
    /// for that pair; missing entries fall back to the hard-coded
    /// baseline in `pin_matrix_view`. Persisted alongside the ERC
    /// severity map.
    pub pin_matrix_overrides: std::collections::HashMap<(u8, u8), signex_erc::Severity>,
}
