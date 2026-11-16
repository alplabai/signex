//! The command catalog (#278, epic #2326 slice 1b): every command's metadata,
//! as UI-free data. Labels are message ids; a front end renders them.

use crate::Modifiers;
use signex_i18n_ids::{MessageId, command_ids};

pub use operands::{Operand, OperandKind};

pub mod active_bar;
pub mod categories;
pub mod general;
pub mod operands;
pub mod pcb;
pub mod schematic;
pub mod threed;

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
