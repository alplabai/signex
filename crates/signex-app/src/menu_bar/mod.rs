//! Top menu bar using iced_aw MenuBar with proper dropdown/submenu support.
//!
//! Altium-style menu structure: File, Edit, View, Place, Design, Tools, Window, Help.
//! iced_aw handles all overlay positioning, hover-to-switch, and keyboard navigation.

use iced::widget::{container, row};
use iced::{Color, Element, Length, Theme};
use iced_aw::menu::{DrawPath, Item, Menu, MenuBar};
use signex_types::theme::ThemeTokens;
use signex_widgets::{raw, text};

use crate::keymap::{AppCommandId, CompiledKeymap};
use crate::styles;

/// Default type / spacing scales; equal to the old literals at the default size.
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

// ─── Messages ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum MenuMessage {
    /// No-op dispatched by passive menu-bar roots ("File", "Edit", …) and
    /// submenu headers ("Export", "Annotation"). iced's `button` widget
    /// (0.14, `src/button.rs:342`) forces `Status::Disabled` whenever
    /// `on_press` is `None`, which means the hover style never fires. By
    /// wiring these buttons to `NoOp` we unlock `Status::Hovered` so the
    /// highlight shows on pointer-over alone, even before iced_aw opens a
    /// dropdown. The message is swallowed by `handle_menu_message` via
    /// its `Task::none()` fallthrough — no handler needs to match it.
    NoOp,
    /// A row that carries a command id: runs the id through
    /// [`crate::app::Signex::dispatch_command`], like its shortcut and the
    /// palette do, so one id has one behaviour on every surface.
    Command(AppCommandId),
    /// Help ▸ About Signex.
    OpenAbout,
    /// Help ▸ Documentation — opens the user guide in the browser.
    OpenDocumentation,
    /// Help ▸ Open Sample Project — unpacks the bundled sample into the
    /// per-user config directory (once) and opens it.
    OpenSampleProject,
    /// Help ▸ Report an Issue — opens the issue tracker in the browser.
    ReportIssue,
    // File
    NewProject,
    OpenProject,
    /// File ▸ Open Recent ▸ `<project>` — reopens through the same path
    /// as File ▸ Open. A path that has vanished is dropped from the list
    /// with a notice instead of being opened.
    OpenRecentProject(std::path::PathBuf),
    /// File ▸ Open Recent ▸ Clear Recent.
    ClearRecentProjects,
    /// File ▸ Import ▸ Altium Project… — runs the `signex-altium-import`
    /// companion on a picked `.PrjPcb` / `.SchDoc` / `.PcbDoc` / `.SchLib`
    /// / `.PcbLib`.
    ImportAltium,
    SaveAs,
    /// File ▸ Save All — saves every unsaved document and Component Preview
    /// row, then does nothing else (no close, no quit).
    SaveAll,
    /// File ▸ Close Document — closes the active tab like the tab menu's
    /// Close; nothing when no tab is open.
    CloseDocument,
    /// Project ▸ Close Project — closes the active project like the project
    /// tree's Close Project, through its unsaved-changes gate; nothing when
    /// no project is open.
    CloseProject,
    ExportPdf,
    ExportNetlist,
    ExportBom,
    ExportErcReport,
    /// File ▸ Exit — closes the main window via the same path as the
    /// chrome ✕ button (`Message::Window(WindowMsg::CloseMainWindow)`). Wired through
    /// `handle_menu_file_command`.
    Exit,
    /// File ▸ Library ▸ Open Library… (v0.9 Phase 1).
    LibraryOpenLibrary,
    /// File ▸ Library ▸ Place Component… (v0.9 Phase 1).
    LibraryPlaceComponent,
    /// Project tree → right-click → Add New to Project ▸ Component
    /// Library. Emitted from `view_context_submenu` for the project
    /// root; the dispatcher resolves the active project and forwards
    /// to `LibraryMessage::CreateLibraryAt(project_root)`.
    AddComponentLibrary,
    /// Library node → right-click → Add New ▸ Component. Emitted
    /// from `view_project_tree_context_menu` when the user right-
    /// clicks a library node; the dispatcher folds it into the
    /// existing `LibraryMessage::NewComponent` modal flow.
    AddLibraryComponent,
    /// Library node → right-click → Add New ▸ Symbol. Resolves the
    /// clicked library, mints `Symbol::empty()` via the mounted
    /// adapter, and opens the new `.snxsym` as a standalone editor
    /// tab. See `handle_add_library_primitive`.
    AddLibrarySymbol,
    /// Library node → right-click → Add New ▸ Footprint. Same flow
    /// as `AddLibrarySymbol` but mints `Footprint::empty()` and
    /// opens a `.snxfpt` tab.
    AddLibraryFootprint,
    // Edit ▸ Arrange (drawings only, #1583)
    BringDrawingsToFront,
    SendDrawingsToBack,
    BringDrawingsForward,
    SendDrawingsBackward,
    // View
    ZoomIn,
    ZoomOut,
    OpenProjectsPanel,
    OpenComponentsPanel,
    OpenNavigatorPanel,
    OpenPropertiesPanel,
    /// Open the active project's options tab.
    OpenProjectOptions,
    OpenErcPanel,
    OpenMessagesPanel,
    OpenSignexAiPanel,
    /// Place ▸ Text — the text-note tool.
    PlaceText,
    /// Place ▸ No Connect — the no-connect tool.
    PlaceNoConnect,
    // Project ▸ Annotation
    Annotate,
    AnnotateReset,
    AnnotateResetDuplicates,
    AnnotateBack,
    AnnotateSheets,
    // Tools
    /// Tools ▸ Passive Network Calculator — opens the in-app
    /// resistor/capacitor/inductor calculator modal. Not an OS window
    /// (see `passive_calculator_modal.rs` for why); re-invoking while it
    /// is already open is a no-op, and the entered state survives a
    /// close/reopen. Routed to `OverlayMsg::OpenPassiveCalculator`.
    OpenPassiveCalculator,
    /// Open the Keyboard Shortcuts reference modal — a single page
    /// listing every binding from the active keymap profile, callable from
    /// Help ▸ Keyboard Shortcuts and from F1.
    OpenKeyboardShortcuts,
    /// Tools ▸ New Part — bumps the active `.snxsym` symbol's max
    /// `part_number` by one and switches the editor's active_part to
    /// the new value. No-op when no Symbol editor is the active tab.
    ToolsNewPart,
    /// Tools ▸ Remove Part — drops the active part on the active
    /// `.snxsym` symbol; pins on that part are demoted to part 1 so
    /// the data survives. No-op when only one part exists or no
    /// Symbol editor is the active tab.
    ToolsRemovePart,
    /// Tools ▸ Document Options... — opens the Document Options modal
    /// for the active `.snxlib` (sheet color / grid / unit). Mirrors
    /// Altium's Tools ▸ Document Options entry. No-op when not on
    /// a SchLib tab.
    ToolsDocumentOptions,
    /// Tools -> Up Hierarchy: open the sheet that references the focused one.
    AscendToParentSheet,
    /// Activate the next / previous schematic tab (wraps).
    NextSheet,
    /// Activate the next / previous main-window document tab of any kind.
    NextDocumentTab,
    PreviousDocumentTab,
    PreviousSheet,
}

impl MenuMessage {
    /// The command a row runs in the window it was clicked in: a row that
    /// carries a command id, and Export ▸ PDF, which opens the Print preview
    /// and so runs as Print (#1813). Such a row clicked in an undocked
    /// window arrives as `UiMsg::MenuInWindow`, so the command acts on that
    /// window's document (#1550, #1794); every other row arrives as
    /// `Message::Menu`.
    pub(crate) fn window_command(&self) -> Option<AppCommandId> {
        match self {
            Self::Command(id) => Some(id.clone()),
            Self::ExportPdf => AppCommandId::new("print").ok(),
            _ => None,
        }
    }

    /// The message of a row that runs command `id`, for tests.
    #[cfg(test)]
    pub(crate) fn command(id: &str) -> Self {
        Self::Command(AppCommandId::new(id).expect("a command id"))
    }

    /// Whether this is a View-menu panel row, which carries a check mark.
    pub(crate) fn is_panel_row(&self) -> bool {
        matches!(
            self,
            Self::OpenProjectsPanel
                | Self::OpenComponentsPanel
                | Self::OpenNavigatorPanel
                | Self::OpenPropertiesPanel
                | Self::OpenErcPanel
                | Self::OpenMessagesPanel
                | Self::OpenSignexAiPanel
        )
    }
}

/// Context passed into `view` so each menu leaf can decide whether to
/// render as an active link or a disabled item. Keeps the menu
/// context-aware — e.g. Annotate / ERC / Save are unclickable when no
/// schematic is open.
#[derive(Debug, Clone)]
pub struct MenuContext<'a> {
    /// The enablement facts for this window; rows keyed on a command id
    /// grey themselves through [`crate::app::command::is_enabled`] (#368).
    pub(crate) commands: crate::app::command::CommandContext,
    /// Reserved for guarding project-wide items (e.g. multi-sheet
    /// navigator, BOM across project) when those land. Currently no
    /// menu entry reads this but the field stays so callers don't
    /// need to update their struct literal when we wire it up.
    pub has_project: bool,
    /// Active committed shortcut profile for menu shortcut labels.
    pub active_keymap: Option<std::sync::Arc<CompiledKeymap>>,
    /// File ▸ Open Recent entries, most recent first.
    pub recent_projects: &'a [std::path::PathBuf],
    /// Panels open somewhere (docked, floating or detached): the View
    /// menu's panel rows show a check mark for these (#103).
    pub(crate) open_panels: Vec<crate::panels::PanelKind>,
    /// The locale the menu titles render in (#2055).
    pub i18n: std::sync::Arc<signex_i18n::I18n>,
    /// Cross Select Mode is on: Tools ▸ Cross Select Mode shows a check
    /// mark (#1312). On by default, like the mode.
    pub cross_select: bool,
}

impl Default for MenuContext<'_> {
    fn default() -> Self {
        Self {
            commands: Default::default(),
            has_project: false,
            active_keymap: None,
            recent_projects: &[],
            open_panels: Vec::new(),
            i18n: signex_i18n::I18n::en_us(),
            cross_select: true,
        }
    }
}

// ─── Constants ────────────────────────────────────────────────

/// Height of the title and menu bar.
pub const MENU_BAR_HEIGHT: f32 = crate::styles::ChromeMetrics::DEFAULT.menu;

/// Menu typography scale. Labels are the baseline; shortcuts ride a step
/// smaller so they read as metadata; the submenu chevron rides a step
/// larger because the `›` glyph is optically lighter than Latin letters
/// at the same pixel height and would otherwise look shrunken next to the
/// label. Exposed as constants so every menu row pulls from the same
/// scale — no `size(11)`/`size(12)`/`size(14)` sprinkled around.
const MENU_SHORTCUT_SIZE: f32 = signex_widgets::active_bar_dropdown::MENU_SHORTCUT_SIZE;

/// Logical width of the chrome wordmark slot the legacy chrome reserves.
const WORDMARK_LOGICAL_W: f32 = 96.0;

/// Approximate visible width of the menu bar in pixels. Includes the
/// Signex wordmark on the left, plus the sum of root button widths
/// (label glyphs at `MENU_LABEL_SIZE` + horizontal padding from
/// `root_btn`) plus the chrome's left padding. Used by the chrome
/// to clamp the centered search bar so it can't slide under the
/// menu items on narrow windows.
pub fn approx_menu_bar_width(i18n: &signex_i18n::I18n) -> f32 {
    // `root_btn` uses `padding([7, 6])` → 12 px horizontal per button.
    const PER_BTN_PADDING: f32 = 12.0;
    // Approx pixels per character at MENU_LABEL_SIZE (12 pt sans-serif).
    // Slight overestimate so we err on the side of "no overlap."
    const PX_PER_CHAR: f32 = 7.5;
    // Chrome strip's left padding from `view_main_window_chrome`.
    const CHROME_LEFT_PAD: f32 = 8.0;
    // Gap between wordmark and the first menu root button.
    const WORDMARK_TO_MENU_GAP: f32 = 8.0;
    let labels_total: f32 = model::root_titles(i18n)
        .iter()
        .map(|l| l.chars().count() as f32 * PX_PER_CHAR + PER_BTN_PADDING)
        .sum();
    CHROME_LEFT_PAD + WORDMARK_LOGICAL_W + WORDMARK_TO_MENU_GAP + labels_total
}

/// Extracted theme colors (all Copy+ʼstatic so closures remain ʼstatic).
#[derive(Clone, Copy)]
struct MenuColors {
    text: Color,
    text_muted: Color,
    text_disabled: Color,
    toolbar_bg: Color,
    panel_bg: Color,
    border: Color,
    hover: Color,
    /// Separator line inside a dropdown.
    line_soft: Color,
    shadow: iced::Shadow,
}

impl MenuColors {
    fn from_tokens(tokens: &ThemeTokens) -> Self {
        Self {
            text: styles::ti(tokens.text),
            text_muted: styles::ti(tokens.text_secondary),
            text_disabled: {
                let t = styles::ti(tokens.text_secondary);
                Color { a: t.a * 0.6, ..t }
            },
            toolbar_bg: styles::ti(tokens.toolbar_bg),
            panel_bg: styles::tokens::resolve(tokens).raised,
            border: styles::ti(tokens.border),
            hover: styles::ti(tokens.hover),
            line_soft: styles::tokens::resolve(tokens).line_soft,
            shadow: styles::floating_shadow(tokens),
        }
    }
}

mod graphite;
pub mod model;
pub(crate) mod native;
mod view;

#[cfg(test)]
pub(crate) mod panel_check_tests;
#[cfg(test)]
mod parameter_manager_tests;

#[cfg(test)]
mod planned_tests;

pub use graphite::approx_menu_bar_width as approx_graphite_menu_bar_width;
pub use view::view;

/// Menu-display label for `command_id`, sourced from the command table's
/// terse `menu_label` (which falls back to the descriptive `label`). The
/// `fallback` literal covers a command with no catalog entry so the visible
/// menu text is never changed by the lookup. Pairs with [`shortcut_for`] so
/// a menu row's label and keybinding come from the same `AppCommandId`.
pub(crate) fn cmd_label(command_id: &str, fallback: &str) -> String {
    crate::keymap::metadata_for_str(command_id)
        .map(|meta| meta.menu_label().to_string())
        .unwrap_or_else(|| fallback.to_string())
}

/// Menu path of the Electrical Rules Check row, built from the label and menu
/// the Project menu uses, so a message naming it cannot drift.
pub(crate) fn erc_menu_path() -> String {
    format!(
        "Project ▸ {}",
        cmd_label("open_erc_dialog", "Electrical Rules Check")
    )
}

/// Whether the row for `command_id` is enabled: the same predicate the
/// keyboard path dispatches by.
fn enabled(ctx: &MenuContext<'_>, command_id: &str) -> bool {
    crate::keymap::AppCommandId::new(command_id)
        .is_ok_and(|id| crate::app::command::is_enabled(&id, &ctx.commands))
}

/// Shortcut text for a menu row. The hardcoded `fallback` stands in only
/// when there is no active keymap at all; with one, a command it does not
/// bind (or the user unbound) shows no hint (#1378).
fn shortcut_for(ctx: &MenuContext<'_>, command_id: &str, fallback: &str) -> Option<String> {
    let Some(keymap) = ctx.active_keymap.as_ref() else {
        return (!fallback.is_empty()).then(|| fallback.to_string());
    };
    Some(keymap.shortcut_label_for_id(command_id)).filter(|label| !label.is_empty())
}

#[cfg(test)]
mod shortcut_tests;

/// Wrap a menu element in the toolbar-strip styled container used on
/// secondary (undocked-tab) windows that keep their OS title bar.
pub fn wrap_plain<'a, M: 'a>(menu: Element<'a, M>, tokens: &ThemeTokens) -> Element<'a, M> {
    iced::advanced::Widget::_boxed(
        container(menu)
            .padding([0.0, SP.m])
            .width(Length::Fill)
            .style(styles::toolbar_strip(tokens)),
    )
}

// ─── Private helpers ─────────────────────────────────────────

#[cfg(test)]
mod golden_tests;
#[cfg(test)]
mod layer_tests;
#[cfg(test)]
mod tests;
