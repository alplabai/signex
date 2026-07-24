//! Command palette — fuzzy-search peek that fronts the chrome strip.
//!
//! VS Code-style Ctrl+Shift+P entry. Three sources feed one flat catalog:
//!
//! 1. **Commands** — every menu action + every panel open.
//! 2. **Symbols** — placed designators in the active project (zoom-to).
//! 3. **Files** — sheets/PCB/libraries in every loaded project (open).
//!
//! Result list is capped at [`MAX_RESULTS`]; a "More results" footer
//! shows when more match. Sublime-text-style non-contiguous scoring with
//! word-boundary bonuses, contiguous-run bonuses, and a length penalty.

use std::path::PathBuf;
use std::sync::LazyLock;

use iced::widget::Id;

use crate::menu_bar::MenuMessage;
use crate::panels::{ALL_PANELS, PanelKind};

/// Stable widget id for the chrome-strip palette `text_input`. Used by
/// the keyboard shortcut handler to drive
/// `iced::widget::operation::focus`.
pub static COMMAND_PALETTE_INPUT_ID: LazyLock<Id> =
    LazyLock::new(|| Id::new("signex.command_palette"));

/// Cap on rows rendered in the dropdown. Beyond this the user sees a
/// "More results — refine query" footer; matches VS Code's behaviour.
pub const MAX_RESULTS: usize = 10;

#[derive(Debug, Clone, Default)]
pub struct CommandPaletteState {
    /// Dropdown open flag. The text_input is always rendered (it's the
    /// chrome-strip search bar); this gates whether the result list
    /// shows. Open implies the input is also focused.
    pub open: bool,
    /// Live query text — echoed on every text_input change.
    pub query: String,
    /// Highlighted row in the result list (0-based, clamped to results
    /// at render time).
    pub selected_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandSource {
    Command,
    Symbol,
    File,
}

#[derive(Debug, Clone)]
pub struct CommandEntry {
    pub source: CommandSource,
    pub label: String,
    /// Secondary text shown in muted style after the label (file path,
    /// symbol value, etc.). Empty when not applicable.
    pub detail: String,
    pub action: CommandAction,
}

#[derive(Debug, Clone)]
pub enum CommandAction {
    Menu(MenuMessage),
    Panel(PanelKind),
    OpenFile(PathBuf),
    /// Focus a placed symbol on the canvas by reference designator.
    /// Resolution to world coords happens in the dispatcher because
    /// only it has the engine reference.
    FocusSymbol {
        reference: String,
    },
}

/// Build the full catalog from the live app state. Cheap: O(menu) +
/// O(panels) + O(placed symbols across active project) +
/// O(sheets across all projects). Called once per query keystroke; the
/// catalog is filtered/scored downstream.
pub fn build_catalog(app: &super::Signex) -> Vec<CommandEntry> {
    let mut out = Vec::with_capacity(256);

    // 1. Commands — menu actions.
    for (label, msg) in menu_command_table() {
        // v0.13.0 — footprint editor gated off; keep its create
        // command out of the palette so there's no dead entry.
        if !crate::feature_flags::FOOTPRINT_EDITOR_ENABLED
            && matches!(msg, MenuMessage::AddLibraryFootprint)
        {
            continue;
        }
        out.push(CommandEntry {
            source: CommandSource::Command,
            label: label.to_string(),
            detail: String::new(),
            action: CommandAction::Menu(msg.clone()),
        });
    }

    // 1b. Commands — panel opens.
    for &panel in ALL_PANELS {
        out.push(CommandEntry {
            source: CommandSource::Command,
            label: format!("Open Panel: {}", panel.label()),
            detail: String::new(),
            action: CommandAction::Panel(panel),
        });
    }

    // 2. Symbols — placed designators in the active project.
    for (reference, value, _footprint, _lib_id) in &app.document_state.panel_ctx.placed_symbols {
        if reference.is_empty() {
            continue;
        }
        out.push(CommandEntry {
            source: CommandSource::Symbol,
            label: reference.clone(),
            detail: value.clone(),
            action: CommandAction::FocusSymbol {
                reference: reference.clone(),
            },
        });
    }

    // 3. Files — every sheet, library, and PCB across loaded projects.
    for project in &app.document_state.projects {
        let project_dir = std::path::Path::new(&project.data.dir);
        if let Some(root) = &project.data.schematic_root {
            let abs = project_dir.join(root);
            out.push(CommandEntry {
                source: CommandSource::File,
                label: root.clone(),
                detail: project.data.name.clone(),
                action: CommandAction::OpenFile(abs),
            });
        }
        for sheet in &project.data.sheets {
            let abs = project_dir.join(&sheet.filename);
            out.push(CommandEntry {
                source: CommandSource::File,
                label: sheet.filename.clone(),
                detail: project.data.name.clone(),
                action: CommandAction::OpenFile(abs),
            });
        }
        if let Some(pcb) = &project.data.pcb_file {
            let abs = project_dir.join(pcb);
            out.push(CommandEntry {
                source: CommandSource::File,
                label: pcb.clone(),
                detail: project.data.name.clone(),
                action: CommandAction::OpenFile(abs),
            });
        }
        for entry in &project.data.libraries {
            let abs = project.data.resolve_library_path(entry);
            let label = entry
                .path
                .file_name()
                .and_then(|s| s.to_str())
                .map(str::to_string)
                .unwrap_or_else(|| abs.display().to_string());
            out.push(CommandEntry {
                source: CommandSource::File,
                label,
                detail: project.data.name.clone(),
                action: CommandAction::OpenFile(abs),
            });
        }
    }

    out
}

/// Filter the catalog by query and rank the survivors. Returns indices
/// into `catalog` paired with their score, descending by score (higher
/// is better). Empty query passes everything through with a baseline
/// rank that prefers Commands > Symbols > Files. The caller can
/// truncate to `MAX_RESULTS`.
pub fn rank_results(catalog: &[CommandEntry], query: &str) -> Vec<(usize, i32)> {
    if query.trim().is_empty() {
        // No query → show the catalog in source-priority order so the
        // empty palette is still useful (recently used menus near top).
        let mut ranked: Vec<(usize, i32)> = catalog
            .iter()
            .enumerate()
            .map(|(idx, entry)| {
                let base = match entry.source {
                    CommandSource::Command => 100,
                    CommandSource::Symbol => 50,
                    CommandSource::File => 25,
                };
                (idx, base)
            })
            .collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        return ranked;
    }

    let mut ranked: Vec<(usize, i32)> = catalog
        .iter()
        .enumerate()
        .filter_map(|(idx, entry)| {
            let target = if entry.detail.is_empty() {
                entry.label.clone()
            } else {
                format!("{} {}", entry.label, entry.detail)
            };
            fuzzy_score(query, &target).map(|score| {
                let source_bonus = match entry.source {
                    CommandSource::Command => 5,
                    CommandSource::Symbol => 3,
                    CommandSource::File => 1,
                };
                (idx, score + source_bonus)
            })
        })
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked
}

/// Sublime-text-style fuzzy score. Returns `None` if any query
/// character is missing from `target`. Higher is better. Bonuses:
/// word-boundary match (+10), contiguous match (+15), full-substring
/// match (+25). Penalty: target length (-len/4) so shorter labels
/// rank above longer ones with the same match quality. The contiguous
/// bonus dominates the word-boundary bonus so "save" → "Save File"
/// outranks "save" → "Set Animation Variant Edit" (acronym matches
/// rank below literal word matches, which matches VS Code's feel).
pub fn fuzzy_score(query: &str, target: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let q: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    let t: Vec<char> = target.chars().flat_map(char::to_lowercase).collect();
    let t_orig: Vec<char> = target.chars().collect();
    if q.len() > t.len() {
        return None;
    }

    let mut score: i32 = 0;
    let mut q_idx = 0usize;
    let mut prev_match: Option<usize> = None;

    for (i, &ch) in t.iter().enumerate() {
        if q_idx >= q.len() {
            break;
        }
        if ch == q[q_idx] {
            // Base hit.
            score += 1;
            // Word boundary: start of string, after a separator, or
            // start of a CamelCase word.
            let at_word_boundary = if i == 0 {
                true
            } else {
                let prev = t_orig[i - 1];
                prev.is_whitespace()
                    || matches!(
                        prev,
                        '_' | '-' | '/' | '\\' | '.' | ':' | '(' | ')' | '[' | ']'
                    )
                    || (prev.is_lowercase() && t_orig[i].is_uppercase())
            };
            if at_word_boundary {
                score += 10;
            }
            // Contiguous run with the previous match. Heavier than the
            // word-boundary bonus so a literal word match ("save" in
            // "Save File") beats an acronym ("save" via S-A-V-E in
            // "Set Animation Variant Edit").
            if let Some(p) = prev_match
                && p + 1 == i
            {
                score += 15;
            }
            prev_match = Some(i);
            q_idx += 1;
        }
    }

    if q_idx < q.len() {
        return None;
    }
    // Bonus for the entire query appearing as a contiguous lowercase
    // substring — strong signal for "the user typed a real word".
    let q_str: String = q.iter().collect();
    let t_str: String = t.iter().collect();
    if t_str.contains(&q_str) {
        score += 25;
    }
    score -= (t.len() as i32) / 4;
    Some(score)
}

/// User-facing label for every menu action that's worth surfacing in
/// the palette. Order influences the empty-query default ranking via
/// `rank_results`'s stable secondary sort on index — keep frequently-
/// used items near the top. NoOp / passive headers are excluded.
fn menu_command_table() -> &'static [(&'static str, MenuMessage)] {
    &[
        // File
        ("New Project", MenuMessage::NewProject),
        ("Open Project…", MenuMessage::OpenProject),
        ("Save", MenuMessage::Save),
        ("Save As…", MenuMessage::SaveAs),
        ("Print Preview…", MenuMessage::PrintPreview),
        ("Export PDF…", MenuMessage::ExportPdf),
        ("Export Netlist…", MenuMessage::ExportNetlist),
        ("Export Bill of Materials…", MenuMessage::ExportBom),
        ("Open Library…", MenuMessage::LibraryOpenLibrary),
        ("Place Component…", MenuMessage::LibraryPlaceComponent),
        ("Add Component Library", MenuMessage::AddComponentLibrary),
        ("Add New Component", MenuMessage::AddLibraryComponent),
        ("Add New Symbol", MenuMessage::AddLibrarySymbol),
        ("Add New Footprint", MenuMessage::AddLibraryFootprint),
        // Edit
        ("Undo", MenuMessage::Undo),
        ("Redo", MenuMessage::Redo),
        ("Cut", MenuMessage::Cut),
        ("Copy", MenuMessage::Copy),
        ("Paste", MenuMessage::Paste),
        ("Paste Special", MenuMessage::SmartPaste),
        ("Delete", MenuMessage::Delete),
        ("Select All", MenuMessage::SelectAll),
        ("Duplicate", MenuMessage::Duplicate),
        ("Find…", MenuMessage::Find),
        ("Find and Replace…", MenuMessage::Replace),
        // View
        ("Zoom In", MenuMessage::ZoomIn),
        ("Zoom Out", MenuMessage::ZoomOut),
        ("Zoom to Fit", MenuMessage::ZoomFit),
        ("Toggle Grid", MenuMessage::ToggleGrid),
        ("Cycle Grid Size", MenuMessage::CycleGrid),
        // Place
        ("Place Wire", MenuMessage::PlaceWire),
        ("Place Bus", MenuMessage::PlaceBus),
        ("Place Net Label", MenuMessage::PlaceLabel),
        ("Place Component", MenuMessage::PlaceComponent),
        // Design
        ("Annotate Schematics", MenuMessage::Annotate),
        ("Annotate Quietly", MenuMessage::AnnotateQuietly),
        ("Reset Annotations", MenuMessage::AnnotateReset),
        (
            "Reset Duplicate Annotations",
            MenuMessage::AnnotateResetDuplicates,
        ),
        ("Force Annotate All", MenuMessage::AnnotateForceAll),
        ("Back-Annotate from PCB", MenuMessage::AnnotateBack),
        ("Annotate Sheets", MenuMessage::AnnotateSheets),
        ("Run ERC", MenuMessage::Erc),
        ("Toggle Auto-Focus", MenuMessage::ToggleAutoFocus),
        ("Generate BOM", MenuMessage::GenerateBom),
        // Tools
        ("PCB Trace Calculator…", MenuMessage::OpenPcbTraceCalculator),
        ("Open Preferences", MenuMessage::OpenPreferences),
        ("New Part", MenuMessage::ToolsNewPart),
        ("Remove Part", MenuMessage::ToolsRemovePart),
        ("Document Options", MenuMessage::ToolsDocumentOptions),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_passes_all() {
        let entries = vec![CommandEntry {
            source: CommandSource::Command,
            label: "Open Preferences".into(),
            detail: String::new(),
            action: CommandAction::Menu(MenuMessage::OpenPreferences),
        }];
        let ranked = rank_results(&entries, "");
        assert_eq!(ranked.len(), 1);
    }

    #[test]
    fn missing_chars_score_none() {
        assert_eq!(fuzzy_score("xyz", "Open Preferences"), None);
    }

    #[test]
    fn prefix_outscores_substring() {
        let pre = fuzzy_score("pre", "Preferences").unwrap();
        let mid = fuzzy_score("pre", "Compress Path").unwrap();
        assert!(pre > mid, "prefix should beat substring: {pre} vs {mid}");
    }

    #[test]
    fn contiguous_outscores_split() {
        let cont = fuzzy_score("save", "Save File").unwrap();
        let split = fuzzy_score("save", "Set Animation Variant Edit").unwrap();
        assert!(
            cont > split,
            "contiguous should beat split: {cont} vs {split}"
        );
    }

    #[test]
    fn word_boundary_bonus() {
        // "open" matching after the space in "Find Open" should beat
        // matching inside "Reopen" (no boundary on the o).
        let boundary = fuzzy_score("op", "Find Open").unwrap();
        let nonboundary = fuzzy_score("op", "Reopen").unwrap();
        assert!(boundary > nonboundary, "{boundary} vs {nonboundary}");
    }

    #[test]
    fn rank_filters_and_orders() {
        let entries = vec![
            CommandEntry {
                source: CommandSource::Command,
                label: "Preferences".into(),
                detail: String::new(),
                action: CommandAction::Menu(MenuMessage::OpenPreferences),
            },
            CommandEntry {
                source: CommandSource::Command,
                label: "Save".into(),
                detail: String::new(),
                action: CommandAction::Menu(MenuMessage::Save),
            },
            CommandEntry {
                source: CommandSource::File,
                label: "preferences.bak".into(),
                detail: String::new(),
                action: CommandAction::OpenFile(std::path::PathBuf::from("preferences.bak")),
            },
        ];
        let ranked = rank_results(&entries, "pref");
        // Both "Preferences" entries match; "Save" doesn't.
        let matched: Vec<usize> = ranked.iter().map(|(idx, _)| *idx).collect();
        assert_eq!(matched.len(), 2);
        // Command source breaks the tie before File source.
        assert_eq!(matched[0], 0);
    }
}
