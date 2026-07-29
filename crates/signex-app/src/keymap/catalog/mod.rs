use crate::keymap::{AppCommandId, Modifiers};

mod active_bar;
mod general;
mod pcb;
mod schematic;
mod threed;

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
}

/// Surface-agnostic icon key. A view surface (menu, toolbar, command
/// palette) maps the key to its actual glyph/asset; the catalog stays a
/// plain identifier so adding an icon never means adding a variant here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconId(pub &'static str);

/// A command's suggested default keyboard shortcut, carried on its
/// catalog entry. Distinct from a bound [`crate::keymap::KeyStroke`] in a
/// keymap profile: a profile's own binding always overrides this
/// default. `key` is a canonical token spelling (e.g. `"c"`, `"delete"`,
/// `"f1"`), matching [`crate::keymap::KeyToken`]'s serde naming.
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
    /// Only meaningful/available when driven from the GUI (no headless
    /// or scripted equivalent).
    pub gui_only: bool,
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

    /// Mutates the open document and lands on the undo stack.
    pub const MUTATES: Self = Self {
        mutates_doc: true,
        undoable: true,
        ..Self::NONE
    };

    /// Mutates the document, undoable, and only reachable interactively
    /// (in-place edits, drag/place gestures, cursor-anchored actions).
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandMetadata {
    pub id: &'static str,
    pub category: &'static str,
    /// Descriptive command label — shown in the Keyboard Shortcuts pane
    /// and the (future) command palette, where the full phrasing helps
    /// ("Place wire", "Zoom in at cursor").
    pub label: &'static str,
    /// Optional terse label for menu surfaces (menu_bar). `None` falls
    /// back to `label`. Lets one command carry both a short menu label
    /// ("Wire") and a descriptive catalog label ("Place wire") so the two
    /// UIs don't have to agree on a single string. Resolve via
    /// [`CommandMetadata::menu_label`].
    pub menu_label: Option<&'static str>,
    pub group: CommandGroup,
    /// Surface-agnostic icon key. `None` until a command is given one.
    pub icon: Option<IconId>,
    /// Suggested default keyboard shortcut. `None` until filled in.
    pub keybind: Option<KeyBind>,
    /// Fixed enablement predicate. Defaults to [`Enablement::Always`].
    pub enable: Enablement,
    /// GUI/undo/visibility flags. Defaults to all-`false`.
    pub flags: CommandFlags,
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
        label: "",
        menu_label: None,
        group: CommandGroup::General,
        icon: None,
        keybind: None,
        enable: Enablement::Always,
        flags: CommandFlags {
            gui_only: false,
            mutates_doc: false,
            undoable: false,
            hidden: false,
        },
    };

    /// The label a menu surface should display: the terse `menu_label`
    /// override when present, else the descriptive `label`.
    pub const fn menu_label(&self) -> &'static str {
        match self.menu_label {
            Some(menu) => menu,
            None => self.label,
        }
    }
}

/// Every command-metadata table, one per surface group. `metadata_for`
/// and the tests iterate across all of them; the split keeps each table
/// file within the size cap without changing lookup behaviour (command
/// ids are unique across groups).
const TABLES: &[&[CommandMetadata]] = &[
    general::GENERAL,
    schematic::SCHEMATIC,
    active_bar::ACTIVE_BAR,
    pcb::PCB,
    threed::THREE_D,
];

/// Flattened iterator over every command's metadata, across all groups.
fn all_metadata() -> impl Iterator<Item = &'static CommandMetadata> {
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

pub fn metadata_for(command: &AppCommandId) -> Option<&'static CommandMetadata> {
    all_metadata().find(|metadata| metadata.id == command.as_str())
}

pub fn fallback_label(command: &AppCommandId) -> String {
    command
        .as_str()
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut label = first.to_ascii_uppercase().to_string();
                    label.push_str(chars.as_str());
                    label
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
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
    fn menu_label_falls_back_to_label_when_unset() {
        // No override → menus show the descriptive label unchanged.
        let copy = metadata_for(&AppCommandId::new("copy").unwrap()).unwrap();
        assert_eq!(copy.menu_label, None);
        assert_eq!(copy.menu_label(), copy.label);
        assert_eq!(copy.menu_label(), "Copy");
        // Override present → menus get the terse form while `label` keeps
        // the descriptive phrasing for the Keyboard Shortcuts pane.
        let wire = metadata_for(&AppCommandId::new("place_wire").unwrap()).unwrap();
        assert_eq!(wire.label, "Place wire");
        assert_eq!(wire.menu_label(), "Wire");
    }

    #[test]
    fn menu_label_overrides_match_menu_bar_text() {
        // Locks the terse menu labels to what menu_bar renders today, so a
        // catalog edit that would silently change a menu's visible text
        // fails here (drift guard toward #271/#272). (command id, menu text)
        const MENU: &[(&str, &str)] = &[
            ("new_document", "New Project"),
            ("open_document", "Open..."),
            ("save_document", "Save"),
            ("save_document_as", "Save As..."),
            ("smart_paste", "Paste Special"),
            ("delete_selection", "Delete"),
            ("select_all", "Select All"),
            ("zoom_to_fit", "Fit All"),
            ("zoom_in_at_cursor", "Zoom In"),
            ("toggle_visible_grid", "Toggle Grid"),
            ("place_wire", "Wire"),
            ("place_bus", "Bus"),
            ("place_net_label", "Net Label"),
            ("open_components_panel", "Component..."),
            ("run_erc", "Electrical Rules Check"),
            ("open_preferences", "Preferences..."),
            ("show_current_command_hotkeys", "Keyboard Shortcuts"),
        ];
        for (id, expected) in MENU {
            let md = metadata_for(&AppCommandId::new(*id).unwrap())
                .unwrap_or_else(|| panic!("missing catalog entry for `{id}`"));
            assert_eq!(md.menu_label(), *expected, "menu label drift for `{id}`");
        }
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

    /// Golden-snapshot test (signex#276): locks the STABLE command-id
    /// surface — `id` + `group` + `category` — because external CLI/plugin
    /// tooling depends on ids staying put. The churny descriptor fields
    /// (`icon`/`keybind`/`enable`/`flags`, added by #275/#479) are
    /// deliberately NOT captured here and may change freely.
    ///
    /// A failing diff means one of two things:
    ///   - a command was renamed or removed. This is a breaking change for
    ///     downstream consumers: add an alias + a deprecation note, do NOT
    ///     just regenerate the golden to make the test pass again.
    ///   - a command was added, or an existing one's `group`/`category`
    ///     changed on purpose. Regenerate with:
    ///       `UPDATE_GOLDEN=1 cargo test -p signex-app command_id_surface_matches_golden_snapshot`
    #[test]
    fn command_id_surface_matches_golden_snapshot() {
        #[derive(serde::Serialize)]
        struct CommandIdSnapshot {
            id: &'static str,
            group: String,
            category: &'static str,
        }

        let mut snapshot: Vec<CommandIdSnapshot> = all_metadata()
            .map(|metadata| CommandIdSnapshot {
                id: metadata.id,
                group: format!("{:?}", metadata.group),
                category: metadata.category,
            })
            .collect();
        snapshot.sort_by_key(|entry| entry.id);

        let actual = serde_json::to_string_pretty(&snapshot).unwrap() + "\n";
        let golden_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/commands.json");

        if std::env::var_os("UPDATE_GOLDEN").is_some() {
            std::fs::write(golden_path, &actual)
                .unwrap_or_else(|err| panic!("failed to write golden {golden_path}: {err}"));
            return;
        }

        let expected = std::fs::read_to_string(golden_path)
            .unwrap_or_else(|err| panic!("failed to read golden {golden_path}: {err}"));
        assert_eq!(
            actual, expected,
            "command-id surface drifted from crates/signex-app/tests/golden/commands.json — \
             see this test's doc comment before regenerating"
        );
    }

    /// Every catalog row must state `enable` and `flags` explicitly.
    ///
    /// A source scan rather than a value check, because the two cannot be
    /// told apart at runtime: `Enablement::Always` + `CommandFlags::NONE`
    /// is both a legitimate answer and what an unpopulated row inherits
    /// from `..CommandMetadata::DEFAULT`. Only the source says whether
    /// somebody decided. Mirrors the source-scanning style of
    /// `keymap::menu_command_tests` and `app::command::bridge`'s guards.
    #[test]
    fn every_catalog_entry_sets_enable_and_flags() {
        const TABLE_SRC: &[(&str, &str)] = &[
            ("general.rs", include_str!("general.rs")),
            ("schematic.rs", include_str!("schematic.rs")),
            ("active_bar.rs", include_str!("active_bar.rs")),
            ("pcb.rs", include_str!("pcb.rs")),
            ("threed.rs", include_str!("threed.rs")),
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
}
