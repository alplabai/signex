//! Keyboard shortcut registry — Altium-compatible defaults.
//!
//! Shortcut handling is done in the subscription in app.rs.
//! This module provides the shortcut definitions for display in menus.

/// A keyboard shortcut for display in menus.
#[allow(dead_code)]
pub struct Shortcut {
    pub key: &'static str,
    pub modifiers: &'static str,
    pub description: &'static str,
}

/// All registered shortcuts (for menu display).
#[allow(dead_code)]
pub const SHORTCUTS: &[Shortcut] = &[
    Shortcut {
        key: "C",
        modifiers: "Ctrl",
        description: "Copy",
    },
    Shortcut {
        key: "X",
        modifiers: "Ctrl",
        description: "Cut",
    },
    Shortcut {
        key: "V",
        modifiers: "Ctrl",
        description: "Paste",
    },
    Shortcut {
        key: "V",
        modifiers: "Shift+Ctrl",
        description: "Smart paste",
    },
    Shortcut {
        key: "D",
        modifiers: "Ctrl",
        description: "Duplicate",
    },
    Shortcut {
        key: "Z",
        modifiers: "Ctrl",
        description: "Undo",
    },
    Shortcut {
        key: "Y",
        modifiers: "Ctrl",
        description: "Redo",
    },
    Shortcut {
        key: "S",
        modifiers: "Ctrl",
        description: "Save",
    },
    Shortcut {
        key: "F",
        modifiers: "Ctrl",
        description: "Find",
    },
    Shortcut {
        key: "H",
        modifiers: "Ctrl",
        description: "Find and Replace",
    },
    Shortcut {
        key: "Q",
        modifiers: "Ctrl",
        description: "Cycle units",
    },
    Shortcut {
        key: "A",
        modifiers: "Ctrl",
        description: "Select all",
    },
    Shortcut {
        key: "W",
        modifiers: "",
        description: "Draw wire",
    },
    Shortcut {
        key: "B",
        modifiers: "",
        description: "Draw bus",
    },
    Shortcut {
        key: "T",
        modifiers: "",
        description: "Place text",
    },
    Shortcut {
        key: "L",
        modifiers: "",
        description: "Place net label",
    },
    Shortcut {
        key: "P",
        modifiers: "",
        description: "Place component",
    },
    Shortcut {
        key: "Escape",
        modifiers: "",
        description: "Cancel / deselect",
    },
    Shortcut {
        key: "Space",
        modifiers: "",
        description: "Rotate 90 CCW",
    },
    Shortcut {
        key: "R",
        modifiers: "",
        description: "Rotate selected",
    },
    Shortcut {
        key: "X",
        modifiers: "",
        description: "Mirror X",
    },
    Shortcut {
        key: "Y",
        modifiers: "",
        description: "Mirror Y",
    },
    Shortcut {
        key: "Home",
        modifiers: "",
        description: "Fit all / center view",
    },
    Shortcut {
        key: "G",
        modifiers: "",
        description: "Cycle grid size",
    },
    Shortcut {
        key: "G",
        modifiers: "Shift+Ctrl",
        description: "Toggle grid visibility",
    },
    Shortcut {
        key: "F5",
        modifiers: "",
        description: "Toggle net color override",
    },
    Shortcut {
        key: "F11",
        modifiers: "",
        description: "Toggle Properties panel",
    },
    Shortcut {
        key: "F",
        modifiers: "Shift",
        description: "Find Similar Objects",
    },
    Shortcut {
        key: "Delete",
        modifiers: "",
        description: "Delete selection",
    },
    Shortcut {
        key: "F2",
        modifiers: "",
        description: "In-place text edit",
    },
];
