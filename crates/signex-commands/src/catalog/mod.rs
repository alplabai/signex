//! The command catalog (#278, epic #2326 slice 1b): every command's metadata,
//! as UI-free data. Labels are message ids; a front end renders them.

use crate::{AppCommandId, Modifiers, ShortcutContext};
use signex_i18n_ids::{MessageId, command_ids};

pub use categories::{category_title_id, title_words};
pub use closest::closest_command_ids;
pub use operands::{MAX_TARGETS, Operand, OperandKind};

mod annotate;
mod arrange;
mod board;
mod board_rule_operands;
mod categories;
mod clipboard;
mod closest;
mod documents;
mod drawing;
mod drc;
mod erc;
mod grid;
mod hierarchy;
mod history;
mod layers;
mod library;
mod nets;
mod operands;
mod pcb_sync;
mod placement;
mod properties;
mod query;
mod search;
mod selection;
mod tool;
mod transform;
mod view;
mod wiring;
mod workspace;

/// Coarse editor-surface bucket used by the Keyboard Shortcuts pane to
/// group commands for display. Distinct from [`CommandMetadata::category`],
/// which stays fine-grained (place / edit / view …); the group is the
/// primary *surface* a command belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandGroup {
    /// Shared editing / file / view / transform commands available on
    /// every surface, plus anything without an obvious home.
    General,
    /// Schematic-specific placement, net, annotation and sheet-navigation
    /// commands.
    Schematic,
    /// PCB routing / layer / via / DRC commands.
    Pcb,
    /// Footprint-editor and 3D-view commands.
    ThreeD,
}

impl CommandGroup {
    /// Display order for the grouped Keyboard Shortcuts view.
    pub const ALL: &'static [CommandGroup] = &[
        CommandGroup::General,
        CommandGroup::Schematic,
        CommandGroup::Pcb,
        CommandGroup::ThreeD,
    ];

    /// Human-readable header shown above each group.
    pub fn display_name(&self) -> &'static str {
        match self {
            CommandGroup::General => "General",
            CommandGroup::Schematic => "Schematic",
            CommandGroup::Pcb => "PCB",
            CommandGroup::ThreeD => "3D",
        }
    }

    /// [`Self::display_name`] as a catalog message.
    pub fn title(&self) -> MessageId {
        use signex_i18n_ids::shortcut_ids as ids;
        match self {
            CommandGroup::General => ids::KEYMAP_GROUP_GENERAL,
            CommandGroup::Schematic => ids::KEYMAP_GROUP_SCHEMATIC,
            CommandGroup::Pcb => ids::KEYMAP_GROUP_PCB,
            CommandGroup::ThreeD => ids::KEYMAP_GROUP_3D,
        }
    }
}

/// Surface-agnostic icon key. A view surface (menu, toolbar, command
/// palette) maps the key to its actual glyph/asset; the catalog stays a
/// plain identifier so adding an icon never means adding a variant here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconId(pub &'static str);

/// A command's suggested default keyboard shortcut, carried on its
/// catalog entry. Distinct from a bound key stroke in a keymap profile: a
/// profile's own binding always overrides this default. `key` is a
/// canonical token spelling (e.g. `"c"`, `"delete"`, `"f1"`), matching the
/// keymap's key-token serde naming.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyBind {
    pub modifiers: Modifiers,
    pub key: &'static str,
}

/// Coarse document-kind gate for [`Enablement::RequiresDocument`]. Mirrors
/// the primary editor surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentKind {
    Schematic,
    Pcb,
    Footprint,
    Symbol,
    Library,
}

/// Fixed predicate gating when a command is enabled. Evaluating this
/// against live application state is future work (a command
/// registry/dispatch consumer, tracked separately) — today it only
/// travels with the catalog entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Enablement {
    /// Always enabled, regardless of selection/document state.
    #[default]
    Always,
    /// Enabled only when the active surface has a non-empty selection.
    RequiresSelection,
    /// Enabled only when the active document matches the given kind.
    RequiresDocument(DocumentKind),
    /// Enabled only when a net color is active/selected.
    RequiresNetColor,
}

/// GUI/undo/visibility flags for a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CommandFlags {
    /// Window UI only: a modal, a panel, the view, an armed tool or a pointer
    /// gesture. It has no operand form and a headless caller is refused it
    /// (#1934, #2170).
    pub gui_only: bool,
    /// Acts without a window although it neither mutates the document nor
    /// takes operands (a selection step, a copy, a check). Only a headless
    /// caller reads this: see [`CommandMetadata::headless_use`] (#2170).
    pub runs_headless: bool,
    /// Mutates the open document.
    pub mutates_doc: bool,
    /// Its mutation is recorded on the undo stack.
    pub undoable: bool,
    /// Hidden from menus/command-palette listings (still dispatchable).
    pub hidden: bool,
}

impl CommandFlags {
    /// No flags — the command neither touches the document nor needs a
    /// pointer. Spelled out on every entry so "nobody decided yet" and
    /// "deliberately none" stay distinguishable.
    pub const NONE: Self = Self {
        gui_only: false,
        runs_headless: false,
        mutates_doc: false,
        undoable: false,
        hidden: false,
    };

    /// Needs the GUI — an interactive placement, a cursor-relative view
    /// operation, a modal, or a panel toggle. No headless equivalent.
    pub const GUI_ONLY: Self = Self {
        gui_only: true,
        ..Self::NONE
    };

    /// Acts without a window and does not mutate the document (#2170).
    pub const RUNS_HEADLESS: Self = Self {
        runs_headless: true,
        ..Self::NONE
    };

    /// Mutates the open document and lands on the undo stack.
    pub const MUTATES: Self = Self {
        mutates_doc: true,
        undoable: true,
        ..Self::NONE
    };

    /// Mutates the document, undoable, and has no operand form: only the GUI
    /// drives it (in-place edits, drag gestures, cursor-anchored actions).
    pub const GUI_MUTATES: Self = Self {
        gui_only: true,
        mutates_doc: true,
        undoable: true,
        ..Self::NONE
    };

    /// Mutates the document but is NOT itself undoable — `undo` / `redo`,
    /// which drive the stack rather than push onto it.
    pub const MUTATES_NOT_UNDOABLE: Self = Self {
        mutates_doc: true,
        ..Self::NONE
    };
}

/// What a headless run (no window) does with a catalog command (#2170).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadlessUse {
    /// Its whole effect is window UI (a modal, a panel, the view, an armed
    /// tool): refused with a reason, never reported as run.
    WindowOnly,
    /// It has a document, file or engine effect, so it runs.
    Runs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandMetadata {
    pub id: &'static str,
    pub category: &'static str,
    /// Descriptive command label message — shown in the Keyboard Shortcuts
    /// pane and the command palette, where the full phrasing helps
    /// ("Place wire", "Zoom in at cursor"). A front end renders it.
    pub label: MessageId,
    /// Optional terse label message for menu surfaces (menu_bar). `None`
    /// falls back to `label`. Lets one command carry both a short menu label
    /// ("Wire") and a descriptive catalog label ("Place wire") so the two
    /// UIs don't have to agree on a single string. See
    /// [`CommandMetadata::menu_label_id`].
    pub menu_label: Option<MessageId>,
    pub group: CommandGroup,
    /// Surface-agnostic icon key. `None` until a command is given one.
    pub icon: Option<IconId>,
    /// Suggested default keyboard shortcut. `None` until filled in.
    pub keybind: Option<KeyBind>,
    /// Fixed enablement predicate. Defaults to [`Enablement::Always`].
    pub enable: Enablement,
    /// GUI/undo/visibility flags. Defaults to all-`false`.
    pub flags: CommandFlags,
    /// The operands a CLI or agent can pass instead of a click or a dialog
    /// answer: the inputs the command needs, the rest comes from the
    /// pre-placement form and project defaults as for a click. Empty: the
    /// command has no operand form (#1934).
    pub operands: &'static [Operand],
}

impl CommandMetadata {
    /// Base value for struct-update syntax (`..CommandMetadata::DEFAULT`)
    /// in the const catalog tables below — every entry there
    /// re-specifies `id`/`category`/`label`/`menu_label`/`group`
    /// explicitly, so only the four descriptor fields actually take this
    /// default (`None` / `None` / `Always` / all-`false`).
    pub const DEFAULT: Self = Self {
        id: "",
        category: "",
        label: command_ids::CMD_UNNAMED,
        menu_label: None,
        group: CommandGroup::General,
        icon: None,
        keybind: None,
        enable: Enablement::Always,
        flags: CommandFlags {
            gui_only: false,
            runs_headless: false,
            mutates_doc: false,
            undoable: false,
            hidden: false,
        },
        operands: &[],
    };

    /// How a headless caller may use this command, or `None` while nobody
    /// decided (#2170): window only, or runs (a document mutation, an operand
    /// form, or [`CommandFlags::runs_headless`]). A row both `gui_only` and
    /// `runs_headless` is `None` too. Read on the headless path only.
    pub fn headless_use(&self) -> Option<HeadlessUse> {
        let f = self.flags;
        match (f.gui_only, f.runs_headless) {
            (true, true) => None,
            (true, false) => Some(HeadlessUse::WindowOnly),
            (false, true) => Some(HeadlessUse::Runs),
            (false, false) => {
                (f.mutates_doc || !self.operands.is_empty()).then_some(HeadlessUse::Runs)
            }
        }
    }

    /// The message a menu surface displays: the terse `menu_label` override
    /// when present, else the descriptive `label`.
    pub const fn menu_label_id(&self) -> MessageId {
        match self.menu_label {
            Some(menu) => menu,
            None => self.label,
        }
    }
}

/// The message of the menu text of `command_id`, for row data that renders
/// later; `cmd-unnamed` for an id the catalog does not have.
pub fn command_menu_message(command_id: &str) -> MessageId {
    metadata_for_str(command_id).map_or(command_ids::CMD_UNNAMED, CommandMetadata::menu_label_id)
}

/// Every command-metadata table, one per domain area (#2447). The order of
/// `TABLES`, and of the rows within each table, is the catalog order: the
/// command palette with an empty query, `catalog.golden.tsv` and the
/// `signex commands` JSON list commands in it. `metadata_for` and the tests
/// iterate across all tables; command ids are unique across them.
///
/// Parameterised families (power ports, net colours, selection-arm modes)
/// have no per-value ids: see
/// `docs/audit/command-registry-action-surface-2026-07-25.md` §4.
const TABLES: &[&[CommandMetadata]] = &[
    documents::DOCUMENTS,
    query::QUERY,
    history::HISTORY,
    clipboard::CLIPBOARD,
    properties::PROPERTIES,
    arrange::ARRANGE,
    transform::TRANSFORM,
    placement::PLACEMENT,
    wiring::WIRING,
    drawing::DRAWING,
    tool::TOOL,
    hierarchy::HIERARCHY,
    annotate::ANNOTATE,
    erc::ERC,
    drc::DRC,
    pcb_sync::PCB_SYNC,
    board::BOARD,
    layers::LAYERS,
    nets::NETS,
    selection::SELECTION,
    search::SEARCH,
    view::VIEW,
    grid::GRID,
    library::LIBRARY,
    workspace::WORKSPACE,
];

/// Flattened iterator over every command's metadata, across all groups, in
/// table order.
pub fn all_metadata() -> impl Iterator<Item = &'static CommandMetadata> {
    TABLES.iter().flat_map(|table| table.iter())
}

/// Every command id in the catalog, in table order.
///
/// Exists so a consumer can ask "what is in the catalog?" without
/// needing `CommandMetadata` itself — the bridge's coverage ratchet
/// (`app::command::bridge`) compares this against the ids
/// `core_to_message` actually matches.
pub fn all_command_ids() -> impl Iterator<Item = &'static str> {
    all_metadata().map(|metadata| metadata.id)
}

/// Id -> metadata index over every table, built once on first lookup.
///
/// The catalog is `const` data, so the index can never go stale. On a
/// duplicate id the first table entry wins, matching the linear `find`
/// it replaces. Menu rows, the shortcuts modal and the Active Bar
/// dropdown all resolve ids per row per frame (#1380).
fn metadata_index() -> &'static std::collections::HashMap<&'static str, &'static CommandMetadata> {
    static INDEX: std::sync::OnceLock<
        std::collections::HashMap<&'static str, &'static CommandMetadata>,
    > = std::sync::OnceLock::new();
    INDEX.get_or_init(|| {
        let mut index = std::collections::HashMap::new();
        for metadata in all_metadata() {
            index.entry(metadata.id).or_insert(metadata);
        }
        index
    })
}

pub fn metadata_for(command: &AppCommandId) -> Option<&'static CommandMetadata> {
    metadata_for_str(command.as_str())
}

/// [`metadata_for`] for a bare id string, so a per-row caller holding a
/// `&str` does not allocate an `AppCommandId` just to look it up.
pub fn metadata_for_str(id: &str) -> Option<&'static CommandMetadata> {
    metadata_index().get(id).copied()
}

/// The linear scan `metadata_for` used before the index; kept so a test
/// can prove the two agree for every catalog id.
#[cfg(test)]
fn metadata_for_linear(id: &str) -> Option<&'static CommandMetadata> {
    all_metadata().find(|metadata| metadata.id == id)
}

/// `General`-group commands that act without an editor: file and tab
/// management, preferences, panels, search, the palette, the shortcuts
/// pane, history (`is_enabled` gates it on the real stacks) and the
/// project-wide jobs that assemble every sheet (`is_enabled` gates those on
/// `has_sheets`, the predicate that enables their menu rows, so palette and
/// menu agree on a PCB tab or a project with no tab open).
///
/// Every other `General` command is editor-scoped. The list is the
/// exception on purpose: a new `General` command stays off the start
/// screen until someone decides it is global.
const GLOBAL_COMMANDS: &[&str] = &[
    "new_document",
    "new_project_with_profile",
    "open_document",
    "save_document",
    "save_document_as",
    "save_all",
    "save_project_as",
    "print",
    "close_active_document",
    "close_all_documents",
    "tile_windows_horizontally",
    "tile_windows_vertically",
    "untile_documents",
    "apply_layout_schematic",
    "apply_layout_board",
    "apply_layout_review",
    "apply_layout_notebook",
    "save_workspace_layout",
    "manage_workspace_layouts",
    "reset_workspace_layout",
    "next_document_tab",
    "previous_document_tab",
    "undo",
    "redo",
    "open_command_palette",
    "open_preferences",
    "open_schematic_preferences",
    "open_library_browser",
    "open_net_inspector",
    "open_compiled_design",
    "show_current_command_hotkeys",
    "show_current_command_shortcuts",
    "toggle_floating_panels",
    "toggle_properties_panel",
    "toggle_schematic_filter_panel",
    "toggle_schematic_list_panel",
    "toggle_search_panel",
    "toggle_auto_focus",
    "find",
    "find_and_replace",
    "find_next",
    "find_previous",
    "find_text",
    "report_manager_bom",
    "annotate_schematic_quietly",
    "force_annotate_all_schematics",
    "run_erc",
    "open_erc_dialog",
    "number_schematic_sheets",
    "compile_project",
    "update_pcb_from_schematic",
    "compare_board_with_schematic",
    "back_annotate_schematics",
    "new_pcb_document",
    "new_schematic_sheet",
    "open_rules",
];

/// The editor contexts in which `command` can act, or `None` when it is
/// global (File, Preferences, panels, the palette itself).
///
/// Derived from the command's own metadata, never from a keymap profile's
/// binding sections: the built-in profiles bind nearly everything under
/// `global`, so a binding says nothing about where a command belongs.
///
/// 1. Ids in [`GLOBAL_COMMANDS`] are global.
/// 2. A few `General` commands act in one editor (`break_wire`, the grid
///    picker) or two (the Net Classes panel: a sheet or a board); they are
///    listed here.
/// 3. `Enablement::RequiresDocument(kind)` names the editor outright.
/// 4. The `Schematic` / `Pcb` / `ThreeD` surface group decides.
/// 5. Any other `General` command is editor-scoped.
pub fn required_contexts(command: &AppCommandId) -> Option<&'static [ShortcutContext]> {
    const EDITORS: &[ShortcutContext] = &[
        ShortcutContext::Schematic,
        ShortcutContext::Pcb,
        ShortcutContext::Footprint,
        ShortcutContext::Library,
    ];
    if GLOBAL_COMMANDS.contains(&command.as_str()) {
        return None;
    }
    match command.as_str() {
        "break_wire"
        | "drag_keep_connections"
        | "cycle_wire_bus_graphic_mode"
        | "cycle_wiring_mode"
        | "switch_segment_posture" => return Some(&[ShortcutContext::Schematic]),
        "open_grid_picker" | "cycle_snap_grid_forward" => {
            return Some(&[ShortcutContext::Footprint]);
        }
        // The project's net-class table, from a sheet or a board.
        "open_net_classes" | "open_net_classes_panel" => {
            return Some(&[ShortcutContext::Schematic, ShortcutContext::Pcb]);
        }
        _ => {}
    }
    // An id with no metadata is not in the catalog, so nothing to offer.
    let metadata = metadata_for(command)?;
    if let Enablement::RequiresDocument(kind) = metadata.enable {
        return Some(match kind {
            DocumentKind::Schematic => &[ShortcutContext::Schematic],
            DocumentKind::Pcb => &[ShortcutContext::Pcb],
            DocumentKind::Footprint => &[ShortcutContext::Footprint],
            DocumentKind::Symbol | DocumentKind::Library => &[ShortcutContext::Library],
        });
    }
    match metadata.group {
        CommandGroup::Schematic => Some(&[ShortcutContext::Schematic]),
        CommandGroup::Pcb => Some(&[ShortcutContext::Pcb]),
        CommandGroup::ThreeD => Some(&[ShortcutContext::Footprint]),
        CommandGroup::General => Some(EDITORS),
    }
}

/// Whether `command` may be offered while `live` contexts are active.
pub fn offered_in(command: &AppCommandId, live: &[ShortcutContext]) -> bool {
    required_contexts(command).is_none_or(|needed| needed.iter().any(|c| live.contains(c)))
}

pub fn fallback_label(command: &AppCommandId) -> String {
    categories::title_words(command.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group_of(id: &str) -> CommandGroup {
        all_metadata()
            .find(|metadata| metadata.id == id)
            .unwrap_or_else(|| panic!("missing command metadata for `{id}`"))
            .group
    }

    /// #2451: Design ▸ Net Classes and its alias Tools ▸ Net Classes open
    /// one panel on a sheet or a board; the catalog scopes both so.
    #[test]
    fn the_net_classes_ids_are_scoped_to_a_sheet_or_a_board() {
        for id in ["open_net_classes", "open_net_classes_panel"] {
            let command = AppCommandId::new(id).expect("a catalog id");
            let metadata = metadata_for(&command).expect("in the catalog");
            assert_eq!(metadata.enable, Enablement::Always, "{id}");
            assert_eq!(group_of(id), CommandGroup::General, "{id}");
            assert_eq!(
                required_contexts(&command),
                Some(&[ShortcutContext::Schematic, ShortcutContext::Pcb][..]),
                "{id}"
            );
        }
    }

    #[test]
    fn indexed_metadata_lookup_equals_the_linear_scan_for_every_id() {
        let mut count = 0;
        for metadata in all_metadata() {
            let command = AppCommandId::new(metadata.id).expect("catalog ids are valid");
            let indexed = metadata_for(&command).expect("indexed lookup finds every catalog id");
            let linear = metadata_for_linear(metadata.id).expect("linear finds it");
            assert!(
                std::ptr::eq(indexed, linear),
                "`{}` resolved differently",
                metadata.id
            );
            count += 1;
        }
        assert!(count > 100, "catalog unexpectedly small: {count}");
        let unknown = AppCommandId::new("definitely_not_a_command").expect("valid id shape");
        assert!(metadata_for(&unknown).is_none());
    }

    #[test]
    fn every_command_has_a_group_in_display_order() {
        // Catches an entry that was left with a stray / unlisted group.
        for metadata in all_metadata() {
            assert!(
                CommandGroup::ALL.contains(&metadata.group),
                "command `{}` has a group absent from CommandGroup::ALL",
                metadata.id
            );
        }
    }

    #[test]
    fn command_groups_follow_primary_surface() {
        // Schematic: placement, design, ERC, PCB-sync, net, sheet-nav.
        assert_eq!(group_of("place_wire"), CommandGroup::Schematic);
        assert_eq!(group_of("place_symbol"), CommandGroup::Schematic);
        assert_eq!(group_of("run_erc"), CommandGroup::Schematic);
        assert_eq!(group_of("annotate_schematic"), CommandGroup::Schematic);
        assert_eq!(
            group_of("update_pcb_from_schematic"),
            CommandGroup::Schematic
        );
        assert_eq!(group_of("next_sheet"), CommandGroup::Schematic);
        assert_eq!(
            group_of("highlight_net_under_cursor"),
            CommandGroup::Schematic
        );
        assert_eq!(group_of("open_components_panel"), CommandGroup::Schematic);
        // Footprint editor / 3D view.
        assert_eq!(group_of("footprint_mode_pads"), CommandGroup::ThreeD);
        assert_eq!(group_of("footprint_mode_view_3d"), CommandGroup::ThreeD);
        // Shared editing / transform / view / file → General.
        assert_eq!(group_of("copy"), CommandGroup::General);
        assert_eq!(group_of("rotate_clockwise"), CommandGroup::General);
        assert_eq!(group_of("mirror_x"), CommandGroup::General);
        assert_eq!(group_of("zoom_to_fit"), CommandGroup::General);
        assert_eq!(group_of("open_preferences"), CommandGroup::General);
    }

    #[test]
    fn grouping_partitions_every_command_exactly_once() {
        let summed: usize = CommandGroup::ALL
            .iter()
            .map(|group| {
                all_metadata()
                    .filter(|metadata| metadata.group == *group)
                    .count()
            })
            .sum();
        assert_eq!(
            summed,
            all_metadata().count(),
            "each command must land in exactly one CommandGroup"
        );
        // The two primary EDA surfaces must carry commands.
        assert!(all_metadata().any(|metadata| metadata.group == CommandGroup::Schematic));
        assert!(all_metadata().any(|metadata| metadata.group == CommandGroup::ThreeD));
    }

    #[test]
    fn icon_and_keybind_still_inherit_the_default() {
        // `icon` and `keybind` are the two descriptor fields still
        // unpopulated: an icon needs a design decision per command, and a
        // catalog `keybind` is only a *suggested* default that any profile
        // binding overrides, so nothing reads it yet. Asserting them for
        // every entry catches a row that drops its
        // `..CommandMetadata::DEFAULT` tail and silently changes what it
        // inherits.
        //
        // `enable` and `flags` used to be asserted here too, at their
        // defaults. They are now populated per command
        // (`every_catalog_entry_sets_enable_and_flags` below), so pinning
        // them at the default would pin the *absence* of the decision.
        for metadata in all_metadata() {
            assert_eq!(
                metadata.icon, None,
                "`{}` should default to no icon",
                metadata.id
            );
            assert_eq!(
                metadata.keybind, None,
                "`{}` should default to no keybind",
                metadata.id
            );
        }
    }

    /// Every catalog row must state `enable` and `flags` explicitly.
    ///
    /// A source scan rather than a value check, because the two cannot be
    /// told apart at runtime: `Enablement::Always` + `CommandFlags::NONE`
    /// is both a legitimate answer and what an unpopulated row inherits
    /// from `..CommandMetadata::DEFAULT`. Only the source says whether
    /// somebody decided. Mirrors the source-scanning style of the app's
    /// `keymap::menu_command_tests` and `app::command::bridge` guards.
    #[test]
    fn every_catalog_entry_sets_enable_and_flags() {
        const TABLE_SRC: &[(&str, &str)] = &[
            ("documents.rs", include_str!("documents.rs")),
            ("query.rs", include_str!("query.rs")),
            ("history.rs", include_str!("history.rs")),
            ("clipboard.rs", include_str!("clipboard.rs")),
            ("properties.rs", include_str!("properties.rs")),
            ("arrange.rs", include_str!("arrange.rs")),
            ("transform.rs", include_str!("transform.rs")),
            ("placement.rs", include_str!("placement.rs")),
            ("wiring.rs", include_str!("wiring.rs")),
            ("drawing.rs", include_str!("drawing.rs")),
            ("tool.rs", include_str!("tool.rs")),
            ("hierarchy.rs", include_str!("hierarchy.rs")),
            ("annotate.rs", include_str!("annotate.rs")),
            ("erc.rs", include_str!("erc.rs")),
            ("drc.rs", include_str!("drc.rs")),
            ("pcb_sync.rs", include_str!("pcb_sync.rs")),
            ("board.rs", include_str!("board.rs")),
            ("layers.rs", include_str!("layers.rs")),
            ("nets.rs", include_str!("nets.rs")),
            ("selection.rs", include_str!("selection.rs")),
            ("search.rs", include_str!("search.rs")),
            ("view.rs", include_str!("view.rs")),
            ("grid.rs", include_str!("grid.rs")),
            ("library.rs", include_str!("library.rs")),
            ("workspace.rs", include_str!("workspace.rs")),
        ];

        let mut missing: Vec<String> = Vec::new();
        let mut seen = 0usize;
        for (file, src) in TABLE_SRC {
            for entry in src.split("    CommandMetadata {").skip(1) {
                let body = entry.split("    },").next().unwrap_or(entry);
                let id = body
                    .split("id: \"")
                    .nth(1)
                    .and_then(|rest| rest.split('"').next())
                    .unwrap_or("<unparsed>");
                seen += 1;
                if !body.contains("enable: ") {
                    missing.push(format!("{file}: `{id}` has no `enable:`"));
                }
                if !body.contains("flags: ") {
                    missing.push(format!("{file}: `{id}` has no `flags:`"));
                }
            }
        }

        assert_eq!(
            seen,
            all_metadata().count(),
            "the source scan found {seen} entries but the tables hold {} — \
             the `CommandMetadata {{` split in this test has drifted from \
             how the tables are written",
            all_metadata().count()
        );
        assert!(
            missing.is_empty(),
            "catalog entries must state `enable` and `flags` rather than \
             inheriting them from ..CommandMetadata::DEFAULT — an inherited \
             `Always` / all-false is indistinguishable from nobody having \
             decided: {missing:#?}"
        );
    }

    /// The compiled catalog and `scripts/catalog-ids.txt` hold the same ids
    /// (#2447). `scripts/check_catalog_ids.sh` reads the source with ast-grep;
    /// this test reads what the program reads through `TABLES`, so a table
    /// left out of `TABLES` or a row compiled out by a `cfg` fails here even
    /// when its file still holds the row.
    #[test]
    fn compiled_catalog_matches_the_accepted_id_list() {
        use std::collections::BTreeSet;
        const LIST: &str = include_str!("../../../../scripts/catalog-ids.txt");
        let listed: BTreeSet<&str> = LIST
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect();
        let compiled: BTreeSet<&str> = all_command_ids().collect();

        let removed: Vec<&&str> = listed.difference(&compiled).collect();
        assert!(
            removed.is_empty(),
            "listed in scripts/catalog-ids.txt but not in the compiled catalog \
             (deleted, renamed, or its table left out of TABLES): {removed:?}. \
             Removing a command needs a maintainer decision and its line \
             removed from the list in the same PR."
        );
        let unlisted: Vec<&&str> = compiled.difference(&listed).collect();
        assert!(
            unlisted.is_empty(),
            "catalog ids missing from scripts/catalog-ids.txt, add one line \
             each: {unlisted:?}"
        );
    }
}
