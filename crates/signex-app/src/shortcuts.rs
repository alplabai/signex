//! Keyboard shortcut registry — Altium-compatible defaults.
//!
//! Shortcut handling is done in the subscription in app.rs.
//! This module provides the shortcut definitions for display in menus
//! and the Help ▸ Keyboard Shortcuts modal.

/// A keyboard shortcut for display in menus and the shortcuts modal.
#[allow(dead_code)]
pub struct Shortcut {
    pub key: &'static str,
    pub modifiers: &'static str,
    pub description: &'static str,
    /// Group label for the Help modal — listed in section order.
    pub category: ShortcutCategory,
}

/// Section grouping. Modal renders one section per variant in
/// declaration order. When mode-specific shortcuts (PCB editor,
/// SCH library) ship later, they will get their own variants
/// (e.g. `PcbDraw`, `LibraryEdit`) and the modal will render
/// only the section(s) matching the active editor.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutCategory {
    File,
    Edit,
    Place,
    Modify,
    Select,
    View,
}

impl ShortcutCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::File => "File",
            Self::Edit => "Edit",
            Self::Place => "Place",
            Self::Modify => "Modify",
            Self::Select => "Select",
            Self::View => "View",
        }
    }

    pub const fn order() -> &'static [Self] {
        &[
            Self::File,
            Self::Edit,
            Self::Place,
            Self::Modify,
            Self::Select,
            Self::View,
        ]
    }
}

/// All registered shortcuts (for menu + help-modal display).
/// Grouped by category in declaration order.
#[allow(dead_code)]
pub const SHORTCUTS: &[Shortcut] = &[
    // ── File ─────────────────────────────────────────────
    Shortcut {
        key: "S",
        modifiers: "Ctrl",
        description: "Save",
        category: ShortcutCategory::File,
    },
    // ── Edit ─────────────────────────────────────────────
    Shortcut {
        key: "C",
        modifiers: "Ctrl",
        description: "Copy",
        category: ShortcutCategory::Edit,
    },
    Shortcut {
        key: "X",
        modifiers: "Ctrl",
        description: "Cut",
        category: ShortcutCategory::Edit,
    },
    Shortcut {
        key: "V",
        modifiers: "Ctrl",
        description: "Paste",
        category: ShortcutCategory::Edit,
    },
    Shortcut {
        key: "V",
        modifiers: "Shift+Ctrl",
        description: "Smart paste",
        category: ShortcutCategory::Edit,
    },
    Shortcut {
        key: "D",
        modifiers: "Ctrl",
        description: "Duplicate",
        category: ShortcutCategory::Edit,
    },
    Shortcut {
        key: "Z",
        modifiers: "Ctrl",
        description: "Undo",
        category: ShortcutCategory::Edit,
    },
    Shortcut {
        key: "Y",
        modifiers: "Ctrl",
        description: "Redo",
        category: ShortcutCategory::Edit,
    },
    Shortcut {
        key: "F",
        modifiers: "Ctrl",
        description: "Find",
        category: ShortcutCategory::Edit,
    },
    Shortcut {
        key: "H",
        modifiers: "Ctrl",
        description: "Find and Replace",
        category: ShortcutCategory::Edit,
    },
    Shortcut {
        key: "Delete",
        modifiers: "",
        description: "Delete selection",
        category: ShortcutCategory::Edit,
    },
    Shortcut {
        key: "F2",
        modifiers: "",
        description: "In-place text edit",
        category: ShortcutCategory::Edit,
    },
    // ── Place ────────────────────────────────────────────
    Shortcut {
        key: "W",
        modifiers: "",
        description: "Draw wire",
        category: ShortcutCategory::Place,
    },
    Shortcut {
        key: "B",
        modifiers: "",
        description: "Draw bus",
        category: ShortcutCategory::Place,
    },
    Shortcut {
        key: "T",
        modifiers: "",
        description: "Place text",
        category: ShortcutCategory::Place,
    },
    Shortcut {
        key: "L",
        modifiers: "",
        description: "Place net label",
        category: ShortcutCategory::Place,
    },
    Shortcut {
        key: "P",
        modifiers: "",
        description: "Place component",
        category: ShortcutCategory::Place,
    },
    // ── Modify ───────────────────────────────────────────
    Shortcut {
        key: "Space",
        modifiers: "",
        description: "Rotate 90 CCW",
        category: ShortcutCategory::Modify,
    },
    Shortcut {
        key: "R",
        modifiers: "",
        description: "Rotate selected",
        category: ShortcutCategory::Modify,
    },
    Shortcut {
        key: "X",
        modifiers: "",
        description: "Mirror X",
        category: ShortcutCategory::Modify,
    },
    Shortcut {
        key: "Y",
        modifiers: "",
        description: "Mirror Y",
        category: ShortcutCategory::Modify,
    },
    // ── Select ───────────────────────────────────────────
    Shortcut {
        key: "A",
        modifiers: "Ctrl",
        description: "Select all",
        category: ShortcutCategory::Select,
    },
    Shortcut {
        key: "F",
        modifiers: "Shift",
        description: "Find Similar Objects",
        category: ShortcutCategory::Select,
    },
    Shortcut {
        key: "Escape",
        modifiers: "",
        description: "Cancel / deselect",
        category: ShortcutCategory::Select,
    },
    // ── View ─────────────────────────────────────────────
    Shortcut {
        key: "Q",
        modifiers: "Ctrl",
        description: "Cycle units",
        category: ShortcutCategory::View,
    },
    Shortcut {
        key: "Home",
        modifiers: "",
        description: "Fit all / center view",
        category: ShortcutCategory::View,
    },
    Shortcut {
        key: "G",
        modifiers: "",
        description: "Cycle grid size",
        category: ShortcutCategory::View,
    },
    Shortcut {
        key: "G",
        modifiers: "Shift+Ctrl",
        description: "Toggle grid visibility",
        category: ShortcutCategory::View,
    },
    Shortcut {
        key: "F5",
        modifiers: "",
        description: "Toggle net color override",
        category: ShortcutCategory::View,
    },
    Shortcut {
        key: "F11",
        modifiers: "",
        description: "Toggle Properties panel",
        category: ShortcutCategory::View,
    },
];
