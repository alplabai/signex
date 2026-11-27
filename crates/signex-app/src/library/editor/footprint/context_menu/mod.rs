//! Right-click canvas context menu for the footprint editor.
//!
//! - On bare canvas: Find Similar, Place ▸, Selection ▸, View ▸, Paste
//!   (with a pad on the clipboard), Properties.
//! - On a pad (the selection): Move Selection By…, Align ▸, Rotate
//!   Selection, Flip Selection, Rectangular Pattern…, Edit Sketch,
//!   Properties, Pad Actions ▸, Cut, Copy, Paste, Find Similar, and
//!   Delete last after a divider.
//! - On a silk graphic: Properties, Find Similar, Delete last.
//!
//! The rows are data ([`menu_entries`]) drawn by the shared
//! `signex_widgets::active_bar_dropdown` renderer, so the card has one icon
//! column, every label starts at the same x and the shortcuts sit
//! right-aligned. A shortcut shown for a command comes from the active
//! keymap; Space and X are the canvas's own rotate and flip keys. A row no
//! key runs shows none.
//!
//! Mounting: `app/view/overlays/bars.rs` (`footprint_context_menu_overlay`)
//! places the card at `editor.state.context_menu.x/y`, window-absolute
//! coordinates computed by the canvas on the right-button release.

use std::path::PathBuf;

use iced::Element;
use iced::widget::svg::Handle;
use signex_i18n::{I18n, MessageId, key_ids, menu_ids};
use signex_library::primitive::footprint::Footprint;
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};

use crate::app::FootprintDocument;
use crate::icons as ic;
use crate::keymap::{CatalogTextExt, CompiledKeymap};
use crate::library::editor::footprint::state::context_menu::SelectionAction;
use crate::library::editor::footprint::state::silk_selection::is_sketch_owned_silk;
use crate::library::editor::footprint::state::{
    AlignOp, FootprintContextAction, FootprintContextSubmenu, FootprintContextTarget, PadsTool,
};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

#[cfg(test)]
mod tests;

/// Card width in px, as the mockup's menu.
const MENU_WIDTH: f32 = 236.0;

/// Theme the icon handles are built for. The card tints every icon with
/// its own ink, so the handle's theme tint is never seen.
const ICON_THEME: ThemeId = ThemeId::DEFAULT;

/// Build the right-click context menu card for the active footprint
/// editor. Returns `None` when the menu is closed.
pub fn view_context_menu<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    path: &'a std::path::Path,
    has_clipboard: bool,
    keymap: &'a CompiledKeymap,
    i18n: &I18n,
) -> Option<Element<'a, LibraryMessage>> {
    let entries = menu_entries(editor, path, has_clipboard, keymap, i18n)?;
    Some(signex_widgets::active_bar_dropdown::view(
        entries,
        tokens,
        Some(MENU_WIDTH),
    ))
}

/// The rows of the open menu, top to bottom; `None` when it is closed.
pub(super) fn menu_entries(
    editor: &FootprintDocument,
    path: &std::path::Path,
    has_clipboard: bool,
    keymap: &CompiledKeymap,
    i18n: &I18n,
) -> Option<Vec<DropdownEntry<LibraryMessage>>> {
    let state = editor.state.context_menu.as_ref()?;
    let mut menu = Menu {
        path: path.to_path_buf(),
        i18n,
        keymap,
        open: state.submenu,
        entries: Vec::new(),
    };
    match state.target {
        FootprintContextTarget::Empty => empty_canvas_rows(&mut menu, has_clipboard),
        FootprintContextTarget::Pad(idx) => {
            let header = editor.state.pads.get(idx).map_or_else(
                || indexed_header(i18n, menu_ids::CTX_FP_PAD_HEADER, "number", idx),
                |p| {
                    i18n.message_plain(
                        menu_ids::CTX_FP_PAD_HEADER,
                        &[("number", p.number.as_str().into())],
                    )
                },
            );
            pad_rows(&mut menu, header, has_clipboard);
        }
        FootprintContextTarget::SilkF(idx) => {
            let header = match editor.primitive().silk_f.get(idx) {
                Some(g) => i18n.message_plain(
                    menu_ids::CTX_FP_SILK_HEADER,
                    &[("kind", menu.text(silk_kind_label(&g.kind)).into())],
                ),
                None => indexed_header(i18n, menu_ids::CTX_FP_SILK_UNKNOWN, "index", idx),
            };
            silk_rows(&mut menu, header, silk_delete_msg(editor.primitive(), idx));
        }
    }
    Some(menu.entries)
}

fn empty_canvas_rows(menu: &mut Menu<'_>, has_clipboard: bool) {
    menu.command_disabled("find_similar_objects", ic::icon_dd_find_similar(ICON_THEME));
    menu.separator();

    // Place ▸ switches the Pads-mode tool; the rest of the Pads palette
    // stays on the active bar.
    if menu.submenu(
        menu_ids::MENU_PLACE,
        ic::icon_dd_place_menu(ICON_THEME),
        FootprintContextSubmenu::Place,
    ) {
        // No key arms a Pads-mode tool, so these rows show none (#2603).
        for (label, tool) in [
            (menu_ids::CTX_FP_PAD, PadsTool::PlacePad),
            (menu_ids::CTX_FP_TRACK, PadsTool::PlaceTrack),
            (menu_ids::CTX_SHAPE_ARC, PadsTool::PlaceArc),
            (menu_ids::CTX_FP_POLYGON_REGION, PadsTool::PlacePolygon),
            (menu_ids::CTX_FP_STRING_TEXT, PadsTool::PlaceString),
        ] {
            let text = menu.text(label);
            menu.child(text, String::new(), FootprintEditorMsg::SetPadsTool(tool));
        }
    }

    if menu.submenu(
        menu_ids::CTX_FP_SELECTION,
        ic::icon_dd_select_all(ICON_THEME),
        FootprintContextSubmenu::Selection,
    ) {
        let label = menu.command_label("select_all");
        menu.command_child(
            label,
            "select_all",
            action(FootprintContextAction::SelectAllPads),
        );
        let label = menu.text(menu_ids::CTX_DESELECT_ALL);
        menu.command_child(
            label,
            "unselect_all",
            action(FootprintContextAction::DeselectAll),
        );
    }

    if menu.submenu(
        menu_ids::MENU_VIEW,
        ic::icon_dd_preferences(ICON_THEME),
        FootprintContextSubmenu::View,
    ) {
        // Fit All does not reach the footprint canvas: no key (#2603).
        let label = menu.text(menu_ids::CTX_FIT_TO_WINDOW);
        menu.child(
            label,
            String::new(),
            action(FootprintContextAction::FitToWindow),
        );
    }

    // Paste appears only with a pad on the clipboard; it pastes at the
    // last known cursor position.
    if has_clipboard {
        menu.command_row(
            "paste",
            ic::icon_dd_paste(ICON_THEME),
            FootprintEditorMsg::PastePad,
        );
        menu.separator();
    }
    menu.command_row_without_keys(
        "edit_selected_object_properties",
        ic::icon_dd_properties(ICON_THEME),
        FootprintEditorMsg::CloseContextMenu,
    );
}

fn pad_rows(menu: &mut Menu<'_>, header: String, has_clipboard: bool) {
    menu.entries.push(DropdownEntry::Header(header));
    menu.separator();

    // The selection: transforms first, as on the active bar's Move menu.
    // No command opens Move By, so it shows no key (#2603).
    menu.pick(
        menu_ids::CTX_FP_MOVE_BY,
        ic::icon_dd_move_xy(ICON_THEME),
        String::new(),
        SelectionAction::MoveBy,
    );
    if menu.submenu(
        menu_ids::CTX_ALIGN,
        ic::icon_dd_align_menu(ICON_THEME),
        FootprintContextSubmenu::Align,
    ) {
        for (label, pick) in ALIGN_ROWS {
            let text = menu.text(label);
            menu.child(
                text,
                String::new(),
                action(FootprintContextAction::OnSelection(pick)),
            );
        }
    }
    let space = menu.text(key_ids::KEY_SPACE);
    menu.pick(
        menu_ids::CTX_FP_ROTATE_SELECTION,
        ic::icon_dd_rotate(ICON_THEME),
        space.clone(),
        SelectionAction::Rotate,
    );
    menu.pick(
        menu_ids::CTX_FP_FLIP_SELECTION,
        ic::icon_dd_flip_x(ICON_THEME),
        "X".to_string(),
        SelectionAction::Flip,
    );
    menu.separator();
    menu.pick(
        menu_ids::CTX_FP_RECT_PATTERN,
        ic::icon_sk_rect_pattern(ICON_THEME),
        String::new(),
        SelectionAction::RectangularPattern,
    );
    menu.pick(
        menu_ids::CTX_FP_EDIT_SKETCH,
        ic::icon_sk_create(ICON_THEME),
        String::new(),
        SelectionAction::EditSketch,
    );
    menu.separator();

    menu.command_row_without_keys(
        "edit_selected_object_properties",
        ic::icon_dd_properties(ICON_THEME),
        FootprintEditorMsg::CloseContextMenu,
    );
    // Pad Actions ▸: rotate and flip wired; Custom Pad and Thermal
    // Connection Points wait for their subsystems.
    if menu.submenu(
        menu_ids::CTX_FP_PAD_ACTIONS,
        ic::icon_dd_part_actions(ICON_THEME),
        FootprintContextSubmenu::PadActions,
    ) {
        let rows = [
            (
                menu_ids::CTX_FP_ROTATE_90,
                space,
                FootprintEditorMsg::ActiveBarRotateSelection,
            ),
            (
                menu_ids::CTX_FP_FLIP_LAYER,
                "X".to_string(),
                FootprintEditorMsg::ActiveBarFlipSelection,
            ),
            (
                menu_ids::CTX_FP_CUSTOM_PAD,
                String::new(),
                FootprintEditorMsg::CloseContextMenu,
            ),
            (
                menu_ids::CTX_FP_THERMAL_POINTS,
                String::new(),
                FootprintEditorMsg::CloseContextMenu,
            ),
        ];
        for (label, keys, msg) in rows {
            let text = menu.text(label);
            menu.child(text, keys, msg);
        }
    }
    menu.command_row(
        "cut",
        ic::icon_dd_cut(ICON_THEME),
        FootprintEditorMsg::CutPad,
    );
    menu.command_row(
        "copy",
        ic::icon_dd_copy(ICON_THEME),
        FootprintEditorMsg::CopyPad,
    );
    if has_clipboard {
        menu.command_row(
            "paste",
            ic::icon_dd_paste(ICON_THEME),
            FootprintEditorMsg::PastePad,
        );
    }
    menu.command_disabled("find_similar_objects", ic::icon_dd_find_similar(ICON_THEME));
    menu.separator();
    menu.command_row(
        "delete_selection",
        ic::icon_dd_delete(ICON_THEME),
        FootprintEditorMsg::DeleteSelected,
    );
}

fn silk_rows(menu: &mut Menu<'_>, header: String, delete: Option<FootprintEditorMsg>) {
    menu.entries.push(DropdownEntry::Header(header));
    menu.separator();
    menu.command_disabled(
        "edit_selected_object_properties",
        ic::icon_dd_properties(ICON_THEME),
    );
    menu.command_disabled("find_similar_objects", ic::icon_dd_find_similar(ICON_THEME));
    menu.separator();
    // A baked entry comes back with the next bake: not deletable.
    match delete {
        Some(msg) => menu.command_row("delete_selection", ic::icon_dd_delete(ICON_THEME), msg),
        None => menu.command_disabled("delete_selection", ic::icon_dd_delete(ICON_THEME)),
    }
}

/// The Align ▸ rows: the dialog first, then the operations of the active
/// bar's Align menu, then the grid snap.
const ALIGN_ROWS: [(MessageId, SelectionAction); 10] = [
    (menu_ids::CTX_FP_ALIGN_DIALOG, SelectionAction::AlignDialog),
    (
        menu_ids::CTX_FP_ALIGN_LEFT,
        SelectionAction::Align(AlignOp::Left),
    ),
    (
        menu_ids::CTX_FP_ALIGN_RIGHT,
        SelectionAction::Align(AlignOp::Right),
    ),
    (
        menu_ids::CTX_FP_ALIGN_HCENTER,
        SelectionAction::Align(AlignOp::CenterH),
    ),
    (
        menu_ids::CTX_FP_DISTRIBUTE_H,
        SelectionAction::Align(AlignOp::DistributeH),
    ),
    (
        menu_ids::CTX_FP_ALIGN_TOP,
        SelectionAction::Align(AlignOp::Top),
    ),
    (
        menu_ids::CTX_FP_ALIGN_BOTTOM,
        SelectionAction::Align(AlignOp::Bottom),
    ),
    (
        menu_ids::CTX_FP_ALIGN_VCENTER,
        SelectionAction::Align(AlignOp::CenterV),
    ),
    (
        menu_ids::CTX_FP_DISTRIBUTE_V,
        SelectionAction::Align(AlignOp::DistributeV),
    ),
    (menu_ids::CTX_FP_ALIGN_TO_GRID, SelectionAction::AlignToGrid),
];

fn action(action: FootprintContextAction) -> FootprintEditorMsg {
    FootprintEditorMsg::ContextMenuAction(action)
}

/// The rows being built, and what they need to read.
struct Menu<'a> {
    path: PathBuf,
    i18n: &'a I18n,
    keymap: &'a CompiledKeymap,
    open: Option<FootprintContextSubmenu>,
    entries: Vec<DropdownEntry<LibraryMessage>>,
}

impl Menu<'_> {
    fn msg(&self, msg: FootprintEditorMsg) -> LibraryMessage {
        LibraryMessage::PrimitiveEditorEvent {
            path: self.path.clone(),
            msg: PrimitiveEdit::Footprint(msg),
        }
    }

    fn text(&self, id: MessageId) -> String {
        self.i18n.message_plain(id, &[])
    }

    /// A command row shows the command's own catalog text.
    fn command_label(&self, id: &str) -> String {
        crate::keymap::metadata_for_str(id)
            .map_or_else(|| id.to_string(), |meta| meta.menu_label_in(self.i18n))
    }

    /// The command's key in the active keymap; empty when unbound.
    fn keys(&self, id: &str) -> String {
        self.keymap.shortcut_label_for_id(id)
    }

    fn separator(&mut self) {
        self.entries.push(DropdownEntry::Separator);
    }

    fn push(&mut self, mut item: DropdownItem<LibraryMessage>, keys: String) {
        if !keys.is_empty() {
            item = item.shortcut(keys);
        }
        self.entries.push(DropdownEntry::Item(item));
    }

    fn row(&mut self, label: String, icon: Handle, keys: String, msg: FootprintEditorMsg) {
        let item = DropdownItem::new(label, self.msg(msg)).icon(icon);
        self.push(item, keys);
    }

    /// A row of a command, with the command's text and key.
    fn command_row(&mut self, id: &'static str, icon: Handle, msg: FootprintEditorMsg) {
        let (label, keys) = (self.command_label(id), self.keys(id));
        let item = DropdownItem::new(label, self.msg(msg))
            .icon(icon)
            .command(id);
        self.push(item, keys);
    }

    /// A command row that shows no key: the row does not run the command.
    fn command_row_without_keys(
        &mut self,
        id: &'static str,
        icon: Handle,
        msg: FootprintEditorMsg,
    ) {
        let label = self.command_label(id);
        self.row(label, icon, String::new(), msg);
    }

    fn command_disabled(&mut self, id: &'static str, icon: Handle) {
        let item = DropdownItem::passive(self.command_label(id))
            .icon(icon)
            .disabled(true);
        self.entries.push(DropdownEntry::Item(item));
    }

    /// A row acting on the selection: closes the menu, then runs `pick`.
    fn pick(&mut self, label: MessageId, icon: Handle, keys: String, pick: SelectionAction) {
        let text = self.text(label);
        self.row(
            text,
            icon,
            keys,
            action(FootprintContextAction::OnSelection(pick)),
        );
    }

    /// A submenu launcher; a click opens it in place, or closes it when
    /// open. Returns whether it is open, so the caller adds its rows.
    fn submenu(&mut self, label: MessageId, icon: Handle, which: FootprintContextSubmenu) -> bool {
        let is_open = self.open == Some(which);
        let toggle = FootprintEditorMsg::ContextMenuOpenSubmenu((!is_open).then_some(which));
        let item = DropdownItem::new(self.text(label), self.msg(toggle))
            .icon(icon)
            .submenu()
            .active(is_open);
        self.entries.push(DropdownEntry::Item(item));
        is_open
    }

    /// A row of an open submenu, indented under its launcher.
    fn child(&mut self, label: String, keys: String, msg: FootprintEditorMsg) {
        let item = DropdownItem::new(format!("  {label}"), self.msg(msg));
        self.push(item, keys);
    }

    /// A submenu row of command `id`, with the command's key.
    fn command_child(&mut self, label: String, id: &'static str, msg: FootprintEditorMsg) {
        let keys = self.keys(id);
        let item = DropdownItem::new(format!("  {label}"), self.msg(msg)).command(id);
        self.push(item, keys);
    }
}

/// Surface the silk graphic's kind in the menu header so the user can
/// tell what they are about to delete or inspect.
fn silk_kind_label(kind: &signex_library::FpGraphicKind) -> MessageId {
    use signex_library::FpGraphicKind;
    match kind {
        FpGraphicKind::Line { .. } => menu_ids::CTX_FP_TRACK,
        FpGraphicKind::Rectangle { .. } => menu_ids::CTX_SHAPE_RECTANGLE,
        FpGraphicKind::Circle { .. } => menu_ids::CTX_SHAPE_CIRCLE,
        FpGraphicKind::Arc { .. } => menu_ids::CTX_SHAPE_ARC,
        FpGraphicKind::Text { .. } => menu_ids::CTX_FP_SILK_STRING,
        FpGraphicKind::Polygon { .. } => menu_ids::CTX_FP_SILK_REGION,
    }
}

/// The message the silk menu's Delete sends; `None` (item disabled) for an
/// entry baked from the sketch, which the next bake would bring back.
fn silk_delete_msg(fp: &Footprint, idx: usize) -> Option<FootprintEditorMsg> {
    (!is_sketch_owned_silk(fp, idx)).then_some(FootprintEditorMsg::DeleteSilkF)
}

/// Header naming a pad or silk graphic by its index, when it has no number
/// or is gone. The index is an identifier, so it shows without digit grouping.
fn indexed_header(i18n: &I18n, id: MessageId, key: &str, idx: usize) -> String {
    i18n.message_plain(id, &[(key, idx.to_string().into())])
}
