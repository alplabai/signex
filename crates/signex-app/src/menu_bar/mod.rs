//! Top menu bar using iced_aw MenuBar with proper dropdown/submenu support.
//!
//! Altium-style menu structure: File, Edit, View, Place, Design, Tools, Window, Help.
//! iced_aw handles all overlay positioning, hover-to-switch, and keyboard navigation.
//! Anchored on the left by the Signex wordmark — PNGs rasterised from
//! `brand/signex-logo-{white,black}.svg` into `brand/generated/` at 1×/2×/3×
//! the on-screen 96×31 logical size. Regenerate via
//! `python installer/build-wordmark.py`.

use std::sync::LazyLock;

use iced::widget::{button, container, image, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};
use iced_aw::menu::{DrawPath, Item, Menu, MenuBar};
use iced_aw::style::menu_bar as menu_style;
use signex_types::theme::ThemeTokens;

use crate::keymap::{AppCommandId, CompiledKeymap};
use crate::styles;

/// Default type / spacing scales; equal to the old literals at the default size.
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Wordmark PNGs pre-rasterised from `signex-logo-{white,black}.svg` at
/// 1× / 2× / 3× the on-screen 96×31 logical size. Picked at view-time by
/// window scale factor so the lockup renders 1:1 with device pixels —
/// which is the only way to get crisp path-text at a size this small
/// (resvg's path rasterization has no font hinting, so a single SVG
/// stretched across DPI tiers aliases at the stems). Regenerate with
/// `python installer/build-wordmark.py` after editing the source SVGs.
static BRAND_WORDMARK_WHITE_1X: LazyLock<image::Handle> = LazyLock::new(|| {
    image::Handle::from_bytes(
        include_bytes!("../../assets/brand/generated/wordmark-white-1x.png").as_slice(),
    )
});
static BRAND_WORDMARK_WHITE_2X: LazyLock<image::Handle> = LazyLock::new(|| {
    image::Handle::from_bytes(
        include_bytes!("../../assets/brand/generated/wordmark-white-2x.png").as_slice(),
    )
});
static BRAND_WORDMARK_WHITE_3X: LazyLock<image::Handle> = LazyLock::new(|| {
    image::Handle::from_bytes(
        include_bytes!("../../assets/brand/generated/wordmark-white-3x.png").as_slice(),
    )
});
static BRAND_WORDMARK_BLACK_1X: LazyLock<image::Handle> = LazyLock::new(|| {
    image::Handle::from_bytes(
        include_bytes!("../../assets/brand/generated/wordmark-black-1x.png").as_slice(),
    )
});
static BRAND_WORDMARK_BLACK_2X: LazyLock<image::Handle> = LazyLock::new(|| {
    image::Handle::from_bytes(
        include_bytes!("../../assets/brand/generated/wordmark-black-2x.png").as_slice(),
    )
});
static BRAND_WORDMARK_BLACK_3X: LazyLock<image::Handle> = LazyLock::new(|| {
    image::Handle::from_bytes(
        include_bytes!("../../assets/brand/generated/wordmark-black-3x.png").as_slice(),
    )
});

/// Logical on-screen size of the wordmark (matches the SVG aspect of
/// 1600:520 → ~3.08:1). Changing this requires regenerating the PNGs at
/// matching multiples via `build-wordmark.py`. Stored as `f32` because
/// `iced::widget::Image::{width,height}` take `impl Into<Length>` and
/// `Length` implements `From<f32>` but not `From<u16>`.
const WORDMARK_LOGICAL_W: f32 = 96.0;
const WORDMARK_LOGICAL_H: f32 = 31.0;

/// Pick the PNG tier that will render closest to 1:1 with device pixels
/// at the given OS scale factor. The small slack (`+ 0.05`) absorbs
/// floating-point jitter — at exactly 1.0 we want the 1× asset, not the
/// 2× downsampled to 96 px.
pub(crate) fn wordmark_tier(scale: f32) -> u8 {
    let s = if scale > 0.0 { scale } else { 1.0 };
    if s <= 1.05 {
        1
    } else if s <= 2.05 {
        2
    } else {
        3
    }
}

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
    Save,
    SaveAs,
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
    // Edit
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SmartPaste,
    Delete,
    SelectAll,
    Duplicate,
    // Edit ▸ Arrange (drawings only, #1583)
    BringDrawingsToFront,
    SendDrawingsToBack,
    BringDrawingsForward,
    SendDrawingsBackward,
    Find,
    Replace,
    // View
    ZoomIn,
    ZoomOut,
    ZoomFit,
    ToggleGrid,
    CycleGrid,
    OpenProjectsPanel,
    OpenComponentsPanel,
    OpenNavigatorPanel,
    OpenPropertiesPanel,
    OpenErcPanel,
    OpenMessagesPanel,
    OpenSignalPanel,
    // Place
    PlaceWire,
    PlaceBus,
    PlaceLabel,
    PlaceComponent,
    /// Place ▸ Power Port — arms the Active Bar's default (GND) power port.
    PlacePowerPort,
    /// Place ▸ Text — the text-note tool.
    PlaceText,
    /// Place ▸ No Connect — the no-connect tool.
    PlaceNoConnect,
    PlaceJunction,
    // Design
    Annotate,
    AnnotateQuietly,
    AnnotateReset,
    AnnotateResetDuplicates,
    AnnotateForceAll,
    AnnotateBack,
    AnnotateSheets,
    Erc,
    ToggleAutoFocus,
    // Tools
    /// Open the Preferences dialog.
    OpenPreferences,
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
                | Self::OpenSignalPanel
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
    /// OS scale factor of the window hosting this menu bar. Drives the
    /// wordmark PNG tier picker (1× / 2× / 3×) so the lockup is rendered
    /// at 1:1 with device pixels. Defaults to 1.0 before the main
    /// window opens and for detached-modal / undocked-tab windows until
    /// per-window scale tracking lands.
    pub scale_factor: f32,
    /// Active committed shortcut profile for menu shortcut labels.
    pub active_keymap: Option<std::sync::Arc<CompiledKeymap>>,
    /// File ▸ Open Recent entries, most recent first.
    pub recent_projects: &'a [std::path::PathBuf],
    /// Panels open somewhere (docked, floating or detached): the View
    /// menu's panel rows show a check mark for these (#103).
    pub(crate) open_panels: Vec<crate::panels::PanelKind>,
}

impl Default for MenuContext<'_> {
    fn default() -> Self {
        Self {
            commands: Default::default(),
            has_project: false,
            scale_factor: 1.0,
            active_keymap: None,
            recent_projects: &[],
            open_panels: Vec::new(),
        }
    }
}

// ─── Constants ────────────────────────────────────────────────

pub const MENU_BAR_HEIGHT: f32 = 36.0;
const DROPDOWN_WIDTH: f32 = 240.0;

/// Menu typography scale. Labels are the baseline; shortcuts ride a step
/// smaller so they read as metadata; the submenu chevron rides a step
/// larger because the `›` glyph is optically lighter than Latin letters
/// at the same pixel height and would otherwise look shrunken next to the
/// label. Exposed as constants so every menu row pulls from the same
/// scale — no `size(11)`/`size(12)`/`size(14)` sprinkled around.
const MENU_LABEL_SIZE: f32 = 12.0;
const MENU_SHORTCUT_SIZE: f32 = 11.0;
const MENU_CHEVRON_SIZE: f32 = 18.0;

/// Approximate visible width of the menu bar in pixels. Includes the
/// Signex wordmark on the left, plus the sum of root button widths
/// (label glyphs at `MENU_LABEL_SIZE` + horizontal padding from
/// `root_btn`) plus the chrome's left padding. Used by the chrome
/// to clamp the centered search bar so it can't slide under the
/// menu items on narrow windows.
pub fn approx_menu_bar_width() -> f32 {
    // `root_btn` uses `padding([7, 6])` → 12 px horizontal per button.
    const PER_BTN_PADDING: f32 = 12.0;
    // Approx pixels per character at MENU_LABEL_SIZE (12 pt sans-serif).
    // Slight overestimate so we err on the side of "no overlap."
    const PX_PER_CHAR: f32 = 7.5;
    // Chrome strip's left padding from `view_main_window_chrome`.
    const CHROME_LEFT_PAD: f32 = 8.0;
    // Gap between wordmark and the first menu root button.
    const WORDMARK_TO_MENU_GAP: f32 = 8.0;
    let labels_total: f32 = model::ROOT_TITLES
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
            panel_bg: styles::ti(tokens.paper),
            border: styles::ti(tokens.border),
            hover: styles::ti(tokens.hover),
        }
    }
}

pub mod model;
pub(crate) mod native;
mod view;

#[cfg(test)]
pub(crate) mod panel_check_tests;
#[cfg(test)]
mod planned_tests;

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

/// Menu path of the Run ERC command, built from the same command label and
/// menu the Design menu uses so a message naming it cannot drift.
pub(crate) fn run_erc_menu_path() -> String {
    format!(
        "Design ▸ {}",
        cmd_label("run_erc", "Electrical Rules Check")
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
        return Some(fallback.to_string());
    };
    let command = AppCommandId::new(command_id).ok()?;
    keymap.shortcut_label(&command)
}

#[cfg(test)]
mod shortcut_tests;

/// Wrap a menu element in the toolbar-strip styled container used on
/// secondary (undocked-tab) windows that keep their OS title bar.
pub fn wrap_plain<'a, M: 'a>(menu: Element<'a, M>, tokens: &ThemeTokens) -> Element<'a, M> {
    container(menu)
        .padding([0.0, SP.m])
        .width(Length::Fill)
        .style(styles::toolbar_strip(tokens))
        .into()
}

/// Perceptual-luminance test used to pick the white/black wordmark and
/// (later) matching chrome icons. Mirrors the sRGB Y' coefficients so
/// cyan/green tones don't fool the check like a naive (r+g+b)/3 would.
fn is_dark_surface(c: signex_types::theme::Color) -> bool {
    let r = c.r as f32 / 255.0;
    let g = c.g as f32 / 255.0;
    let b = c.b as f32 / 255.0;
    let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    lum < 0.5
}

// ─── Private helpers ─────────────────────────────────────────

/// Root-level menu button (top bar).
///
/// Altium paints a subtle framed highlight behind the label on hover and
/// keeps it lit while the dropdown is open. `button::Status::Hovered` covers
/// the pointer case; `Pressed` is the "menu is open" state (iced_aw holds
/// the root in Pressed while its submenu is visible).
fn root_btn(label: &str, mc: MenuColors) -> Element<'static, MenuMessage> {
    let label = label.to_owned();
    let hover_bg = mc.hover;
    let border = mc.border;
    let text_c = mc.text;
    button(text(label).size(MENU_LABEL_SIZE).color(text_c))
        // Tight horizontal padding so the highlight box hugs the label —
        // Altium's root buttons don't extend far past their text. A wide
        // highlight pushes the dropdown's anchor (iced_aw uses the
        // button's layout bounds) out to the left of where the label
        // sits, which reads as "dropdown starts too far left."
        .padding([7, 6])
        .on_press(MenuMessage::NoOp)
        .style(move |_: &Theme, status: button::Status| {
            let lit = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: if lit {
                    Some(Background::Color(hover_bg))
                } else {
                    None
                },
                text_color: text_c,
                border: Border {
                    width: if lit { 1.0 } else { 0.0 },
                    radius: 2.0.into(),
                    color: border,
                },
                ..button::Style::default()
            }
        })
        .into()
}

/// Menu row that acts as a submenu header — label on the left, right
/// chevron on the right, no shortcut. Does not dispatch on click; the
/// menu framework opens the nested submenu on hover.
fn submenu_item_btn(label: &str, mc: MenuColors) -> Element<'static, MenuMessage> {
    let label = label.to_owned();
    let r = row![
        text(label).size(MENU_LABEL_SIZE).color(mc.text),
        iced::widget::Space::new().width(Length::Fill),
        text("›".to_string())
            .size(MENU_CHEVRON_SIZE)
            .color(mc.text_muted),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);
    button(r)
        // Match the padding used by `menu_item_btn` so Export sits on the
        // same left/right grid as the normal leaf rows (Save, Open…).
        .padding([SP.s, SP.l])
        .width(Length::Fill)
        .on_press(MenuMessage::NoOp)
        .style(move |_: &Theme, status: button::Status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => {
                    Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.06)))
                }
                _ => None,
            };
            button::Style {
                background: bg,
                border: Border::default(),
                text_color: mc.text,
                ..button::Style::default()
            }
        })
        .into()
}

/// Separator line between menu sections.
fn separator(mc: MenuColors) -> Item<'static, MenuMessage, Theme, iced::Renderer> {
    Item::new(
        container(iced::widget::Space::new())
            .height(1)
            .width(Length::Fill)
            .padding([SP.xs, SP.m])
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(mc.border)),
                ..container::Style::default()
            }),
    )
}

/// Build a single menu item button with label + shortcut text.
fn menu_item_btn(
    label: &str,
    checked: Option<bool>,
    shortcut: Option<String>,
    msg: Option<MenuMessage>,
    mc: MenuColors,
) -> Element<'static, MenuMessage> {
    let enabled = msg.is_some();
    let text_c = if enabled { mc.text } else { mc.text_disabled };

    let label = label.to_owned();
    // A toggle row reserves the mark column whether or not it is checked,
    // so the labels of a group line up.
    let mark = checked.map(|on| {
        text(if on { "\u{2713}" } else { "" })
            .size(MENU_LABEL_SIZE)
            .color(text_c)
            .width(Length::Fixed(14.0))
    });
    let mut r = iced::widget::Row::new()
        .spacing(SP.m)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);
    if let Some(mark) = mark {
        r = r.push(mark);
    }
    r = r.push(
        text(label)
            .size(MENU_LABEL_SIZE)
            .color(text_c)
            .wrapping(iced::widget::text::Wrapping::None),
    );

    if let Some(sc) = shortcut {
        r = r.push(iced::widget::Space::new().width(Length::Fill));
        r = r.push(
            text(sc)
                .size(MENU_SHORTCUT_SIZE)
                .color(mc.text_muted)
                .wrapping(iced::widget::text::Wrapping::None),
        );
    }

    let hover_bg = mc.hover;
    let btn = button(r).padding([SP.s, SP.l]).width(Length::Fill).style(
        move |_: &Theme, status: button::Status| {
            let bg = if enabled {
                match status {
                    button::Status::Hovered | button::Status::Pressed => {
                        Some(Background::Color(hover_bg))
                    }
                    _ => None,
                }
            } else {
                None
            };
            button::Style {
                background: bg,
                text_color: text_c,
                border: Border {
                    radius: 2.0.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            }
        },
    );

    if let Some(m) = msg {
        btn.on_press(m).into()
    } else {
        btn.into()
    }
}

#[cfg(test)]
mod golden_tests;
