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

use crate::styles;

/// Wordmark PNGs pre-rasterised from `signex-logo-{white,black}.svg` at
/// 1× / 2× / 3× the on-screen 96×31 logical size. Picked at view-time by
/// window scale factor so the lockup renders 1:1 with device pixels —
/// which is the only way to get crisp path-text at a size this small
/// (resvg's path rasterization has no font hinting, so a single SVG
/// stretched across DPI tiers aliases at the stems). Regenerate with
/// `python installer/build-wordmark.py` after editing the source SVGs.
static BRAND_WORDMARK_WHITE_1X: LazyLock<image::Handle> = LazyLock::new(|| {
    image::Handle::from_bytes(
        include_bytes!("../assets/brand/generated/wordmark-white-1x.png").as_slice(),
    )
});
static BRAND_WORDMARK_WHITE_2X: LazyLock<image::Handle> = LazyLock::new(|| {
    image::Handle::from_bytes(
        include_bytes!("../assets/brand/generated/wordmark-white-2x.png").as_slice(),
    )
});
static BRAND_WORDMARK_WHITE_3X: LazyLock<image::Handle> = LazyLock::new(|| {
    image::Handle::from_bytes(
        include_bytes!("../assets/brand/generated/wordmark-white-3x.png").as_slice(),
    )
});
static BRAND_WORDMARK_BLACK_1X: LazyLock<image::Handle> = LazyLock::new(|| {
    image::Handle::from_bytes(
        include_bytes!("../assets/brand/generated/wordmark-black-1x.png").as_slice(),
    )
});
static BRAND_WORDMARK_BLACK_2X: LazyLock<image::Handle> = LazyLock::new(|| {
    image::Handle::from_bytes(
        include_bytes!("../assets/brand/generated/wordmark-black-2x.png").as_slice(),
    )
});
static BRAND_WORDMARK_BLACK_3X: LazyLock<image::Handle> = LazyLock::new(|| {
    image::Handle::from_bytes(
        include_bytes!("../assets/brand/generated/wordmark-black-3x.png").as_slice(),
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
fn wordmark_tier(scale: f32) -> u8 {
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
#[allow(dead_code)]
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
    // File
    NewProject,
    OpenProject,
    Save,
    SaveAs,
    PrintPreview,
    ExportPdf,
    ExportNetlist,
    ExportBom,
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
    GenerateBom,
    // Tools
    /// Open the Preferences dialog.
    OpenPreferences,
}

/// Context passed into `view` so each menu leaf can decide whether to
/// render as an active link or a disabled item. Keeps the menu
/// context-aware — e.g. Annotate / ERC / Save are unclickable when no
/// schematic is open.
#[derive(Debug, Clone, Copy)]
pub struct MenuContext {
    pub has_schematic: bool,
    pub has_pcb: bool,
    /// Reserved for guarding project-wide items (e.g. multi-sheet
    /// navigator, BOM across project) when those land. Currently no
    /// menu entry reads this but the field stays so callers don't
    /// need to update their struct literal when we wire it up.
    #[allow(dead_code)]
    pub has_project: bool,
    pub has_selection: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    /// OS scale factor of the window hosting this menu bar. Drives the
    /// wordmark PNG tier picker (1× / 2× / 3×) so the lockup is rendered
    /// at 1:1 with device pixels. Defaults to 1.0 before the main
    /// window opens and for detached-modal / undocked-tab windows until
    /// per-window scale tracking lands.
    pub scale_factor: f32,
}

impl Default for MenuContext {
    fn default() -> Self {
        Self {
            has_schematic: false,
            has_pcb: false,
            has_project: false,
            has_selection: false,
            can_undo: false,
            can_redo: false,
            scale_factor: 1.0,
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

// ─── View: Menu Bar ──────────────────────────────────────────

pub fn view(tokens: &ThemeTokens, ctx: MenuContext) -> Element<'static, MenuMessage> {
    let mc = MenuColors::from_tokens(tokens);
    // `leaf_if(enabled, ..)` wraps `leaf`/`leaf_stub` — enabled items
    // dispatch their message, disabled items render greyed-out like
    // the stub entries so Annotate / ERC / Save can't fire when no
    // schematic is loaded.
    let leaf_if = |label: &str,
                   shortcut: Option<&str>,
                   msg: MenuMessage,
                   enabled: bool|
     -> Item<'static, MenuMessage, Theme, iced::Renderer> {
        if enabled {
            leaf(label, shortcut, msg, mc)
        } else {
            leaf_stub(label, shortcut, mc)
        }
    };

    let menu_template = |items| {
        Menu::new(items)
            .max_width(DROPDOWN_WIDTH)
            // Sit a couple of pixels below the bar — offset(0) overlaps
            // the bar's bottom row because the dropdown's 1px border
            // paints on the same pixel as the bar's baseline.
            .offset(2.0)
            .spacing(2.0)
            // iced_aw paints the dropdown's background quad at
            // `items.x - padding.left` (see `pad_rectangle` in
            // `iced_aw/src/widget/menu/menu_tree.rs`). Any positive
            // left-padding here drags the visible dropdown LEFT of the
            // root button's highlight box. Zero on the left keeps the
            // dropdown's left border flush with the root's layout
            // bounds, matching Altium's alignment.
            .padding(iced::Padding {
                top: 5.0,
                right: 5.0,
                bottom: 5.0,
                left: 0.0,
            })
    };

    let export_menu = Item::with_menu(
        submenu_item_btn("Export", mc),
        menu_template(vec![
            leaf_if(
                "PDF…",
                Some("Ctrl+Shift+P"),
                MenuMessage::ExportPdf,
                ctx.has_schematic,
            ),
            leaf_if(
                "Netlist (.net)...",
                None,
                MenuMessage::ExportNetlist,
                ctx.has_schematic,
            ),
            leaf_if(
                "Bill of Materials…",
                None,
                MenuMessage::ExportBom,
                ctx.has_schematic,
            ),
        ]),
    );

    let file_menu = Item::with_menu(
        root_btn("File", mc),
        menu_template(vec![
            leaf_stub("New Project", Some("Ctrl+N"), mc),
            leaf("Open...", Some("Ctrl+O"), MenuMessage::OpenProject, mc),
            separator(mc),
            leaf_if("Save", Some("Ctrl+S"), MenuMessage::Save, ctx.has_schematic),
            leaf_if(
                "Save As...",
                Some("Ctrl+Shift+S"),
                MenuMessage::SaveAs,
                ctx.has_schematic,
            ),
            separator(mc),
            // Print Preview... previously lived here as a top-level
            // leaf duplicating Export → PDF's shortcut. Consolidated
            // under Export → PDF so there's one surface that opens the
            // same preview flow. Ctrl+P (the former top-level shortcut)
            // still fires `MenuMessage::PrintPreview` through the
            // global key handler; only the menu row moves.
            export_menu,
            separator(mc),
            leaf_stub("Exit", None, mc),
        ]),
    );

    let edit_menu = Item::with_menu(
        root_btn("Edit", mc),
        menu_template(vec![
            leaf_if("Undo", Some("Ctrl+Z"), MenuMessage::Undo, ctx.can_undo),
            leaf_if("Redo", Some("Ctrl+Y"), MenuMessage::Redo, ctx.can_redo),
            separator(mc),
            leaf_if("Cut", Some("Ctrl+X"), MenuMessage::Cut, ctx.has_selection),
            leaf_if("Copy", Some("Ctrl+C"), MenuMessage::Copy, ctx.has_selection),
            leaf_if(
                "Paste",
                Some("Ctrl+V"),
                MenuMessage::Paste,
                ctx.has_schematic,
            ),
            leaf_if(
                "Smart Paste",
                Some("Shift+Ctrl+V"),
                MenuMessage::SmartPaste,
                ctx.has_schematic,
            ),
            leaf_if(
                "Duplicate",
                Some("Ctrl+D"),
                MenuMessage::Duplicate,
                ctx.has_selection,
            ),
            leaf_if(
                "Delete",
                Some("Del"),
                MenuMessage::Delete,
                ctx.has_selection,
            ),
            separator(mc),
            leaf_if(
                "Select All",
                Some("Ctrl+A"),
                MenuMessage::SelectAll,
                ctx.has_schematic,
            ),
            separator(mc),
            leaf_if("Find", Some("Ctrl+F"), MenuMessage::Find, ctx.has_schematic),
            leaf_if(
                "Find and Replace",
                Some("Ctrl+H"),
                MenuMessage::Replace,
                ctx.has_schematic,
            ),
        ]),
    );

    let view_menu = Item::with_menu(
        root_btn("View", mc),
        menu_template(vec![
            leaf_stub("Zoom In", Some("Ctrl+="), mc),
            leaf_stub("Zoom Out", Some("Ctrl+-"), mc),
            leaf_if(
                "Fit All",
                Some("Home"),
                MenuMessage::ZoomFit,
                ctx.has_schematic || ctx.has_pcb,
            ),
            separator(mc),
            leaf_if(
                "Toggle Grid",
                Some("Shift+Ctrl+G"),
                MenuMessage::ToggleGrid,
                ctx.has_schematic || ctx.has_pcb,
            ),
            leaf_if(
                "Cycle Grid Size",
                Some("G"),
                MenuMessage::CycleGrid,
                ctx.has_schematic || ctx.has_pcb,
            ),
            leaf_if(
                "AutoFocus (dim unselected)",
                Some("F9"),
                MenuMessage::ToggleAutoFocus,
                ctx.has_schematic,
            ),
            separator(mc),
            // Panel-open entries are always available — panels work
            // without an active document (show empty state).
            leaf("Projects", None, MenuMessage::OpenProjectsPanel, mc),
            leaf("Components", None, MenuMessage::OpenComponentsPanel, mc),
            leaf("Navigator", None, MenuMessage::OpenNavigatorPanel, mc),
            leaf("Properties", None, MenuMessage::OpenPropertiesPanel, mc),
            leaf("ERC", None, MenuMessage::OpenErcPanel, mc),
            leaf("Messages", None, MenuMessage::OpenMessagesPanel, mc),
            leaf("Signal", None, MenuMessage::OpenSignalPanel, mc),
        ]),
    );

    let place_menu = Item::with_menu(
        root_btn("Place", mc),
        menu_template(vec![
            leaf_if("Wire", Some("W"), MenuMessage::PlaceWire, ctx.has_schematic),
            leaf_if("Bus", Some("B"), MenuMessage::PlaceBus, ctx.has_schematic),
            leaf_if(
                "Net Label",
                Some("L"),
                MenuMessage::PlaceLabel,
                ctx.has_schematic,
            ),
            separator(mc),
            leaf_if(
                "Component...",
                Some("P"),
                MenuMessage::PlaceComponent,
                ctx.has_schematic,
            ),
            leaf_stub("Power Port", None, mc),
            separator(mc),
            leaf_stub("Text", None, mc),
            leaf_stub("No Connect", None, mc),
            leaf_stub("Sheet Entry", None, mc),
        ]),
    );

    // Design → Annotation submenu mirrors Altium's Annotation cascade.
    // Every entry gated on `has_schematic` — annotating without a
    // project open is nonsense.
    let annotation_submenu: Item<'static, MenuMessage, Theme, iced::Renderer> = Item::with_menu(
        submenu_item_btn("Annotation", mc),
        menu_template(vec![
            leaf_if(
                "Annotate Schematics...",
                None,
                MenuMessage::Annotate,
                ctx.has_schematic,
            ),
            leaf_if(
                "Reset Schematic Designators...",
                None,
                MenuMessage::AnnotateReset,
                ctx.has_schematic,
            ),
            leaf_if(
                "Reset Duplicate Schematic Designators...",
                None,
                MenuMessage::AnnotateResetDuplicates,
                ctx.has_schematic,
            ),
            separator(mc),
            leaf_if(
                "Annotate Schematics Quietly",
                Some("Alt+A"),
                MenuMessage::AnnotateQuietly,
                ctx.has_schematic,
            ),
            leaf_if(
                "Force Annotate All Schematics",
                Some("Shift+Alt+A"),
                MenuMessage::AnnotateForceAll,
                ctx.has_schematic,
            ),
            separator(mc),
            leaf_stub("Back Annotate Schematics...", None, mc),
            leaf_stub("Number Schematic Sheets...", None, mc),
        ]),
    );

    let design_menu = Item::with_menu(
        root_btn("Design", mc),
        menu_template(vec![
            annotation_submenu,
            separator(mc),
            leaf_if(
                "Electrical Rules Check",
                Some("F8"),
                MenuMessage::Erc,
                ctx.has_schematic,
            ),
            separator(mc),
            leaf_stub("Generate BOM", None, mc),
            leaf_stub("Generate Netlist", None, mc),
        ]),
    );

    let tools_menu = Item::with_menu(
        root_btn("Tools", mc),
        menu_template(vec![
            leaf_stub("Assign Footprints", None, mc),
            leaf_stub("Library Editor", None, mc),
            separator(mc),
            leaf_stub("Design Rule Check", None, mc),
            leaf_stub("Net Inspector", None, mc),
            separator(mc),
            leaf(
                "Preferences...",
                Some("Ctrl+,"),
                MenuMessage::OpenPreferences,
                mc,
            ),
        ]),
    );

    let window_menu = Item::with_menu(
        root_btn("Window", mc),
        menu_template(vec![
            leaf_stub("Tile Horizontally", None, mc),
            leaf_stub("Tile Vertically", None, mc),
            separator(mc),
            leaf_stub("Close All Documents", None, mc),
        ]),
    );

    let help_menu = Item::with_menu(
        root_btn("Help", mc),
        menu_template(vec![
            leaf_stub("About Signex", None, mc),
            separator(mc),
            leaf_stub("Keyboard Shortcuts", None, mc),
        ]),
    );

    let mb: MenuBar<'static, MenuMessage, Theme, iced::Renderer> = MenuBar::new(vec![
        file_menu,
        edit_menu,
        view_menu,
        place_menu,
        design_menu,
        tools_menu,
        window_menu,
        help_menu,
    ])
    .spacing(1.0)
    .padding([1, 4])
    .close_on_item_click_global(true)
    .close_on_background_click_global(true)
    // `Backdrop` paints `styling.path` behind the active root while its
    // dropdown is open, so "File / Edit / Place / …" stays visibly lit
    // after the pointer leaves the root and enters the submenu — matches
    // Altium. `FakeHovering` (the default) only affects items inside the
    // dropdown and leaves the root dark.
    .draw_path(DrawPath::Backdrop)
    .style(move |_theme: &Theme, _status| menu_style::Style {
        bar_background: Background::Color(mc.toolbar_bg),
        bar_border: Border::default(),
        bar_shadow: iced::Shadow::default(),
        menu_background: Background::Color(mc.panel_bg),
        menu_border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: mc.border,
        },
        menu_shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
            offset: iced::Vector::new(2.0, 4.0),
            blur_radius: 8.0,
        },
        path: Background::Color(mc.hover),
        path_border: Border {
            width: 1.0,
            radius: 2.0.into(),
            color: mc.border,
        },
    });

    // Wordmark — white on dark themes, near-black on light themes. Picked
    // by toolbar background luminance so custom themes also resolve
    // correctly. The asset is a PNG pre-rasterised at 1×/2×/3× the
    // on-screen 96×31 logical size; `wordmark_tier` picks the one that
    // matches the window's scale factor so text edges stay crisp at
    // 100 %, 125 %, 150 %, 200 %, 300 % Windows scaling. Filter method
    // is Linear so fractional in-between scales (e.g. 1.5×) downsample
    // cleanly from the 2× asset rather than hard-pixelating.
    let dark = is_dark_surface(tokens.toolbar_bg);
    let handle = match (dark, wordmark_tier(ctx.scale_factor)) {
        (true, 1) => (*BRAND_WORDMARK_WHITE_1X).clone(),
        (true, 2) => (*BRAND_WORDMARK_WHITE_2X).clone(),
        (true, _) => (*BRAND_WORDMARK_WHITE_3X).clone(),
        (false, 1) => (*BRAND_WORDMARK_BLACK_1X).clone(),
        (false, 2) => (*BRAND_WORDMARK_BLACK_2X).clone(),
        (false, _) => (*BRAND_WORDMARK_BLACK_3X).clone(),
    };
    let wordmark = image(handle)
        .width(WORDMARK_LOGICAL_W)
        .height(WORDMARK_LOGICAL_H)
        .filter_method(image::FilterMethod::Linear);

    // Just the wordmark + menu roots. The caller decides how to wrap this
    // (plain strip on secondary windows, draggable chrome with window
    // controls on the borderless main window).
    row![wordmark, mb]
        .spacing(10)
        .align_y(iced::Alignment::Center)
        .into()
}

/// Wrap a menu element in the toolbar-strip styled container used on
/// secondary (undocked-tab) windows that keep their OS title bar.
pub fn wrap_plain<'a, M: 'a>(menu: Element<'a, M>, tokens: &ThemeTokens) -> Element<'a, M> {
    container(menu)
        .padding([0, 8])
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

/// Leaf menu item with an action.
fn leaf(
    label: &str,
    shortcut: Option<&str>,
    msg: MenuMessage,
    mc: MenuColors,
) -> Item<'static, MenuMessage, Theme, iced::Renderer> {
    Item::new(menu_item_btn(label, shortcut, Some(msg), mc))
}

/// Leaf menu item — disabled/stub (no action yet).
fn leaf_stub(
    label: &str,
    shortcut: Option<&str>,
    mc: MenuColors,
) -> Item<'static, MenuMessage, Theme, iced::Renderer> {
    Item::new(menu_item_btn(label, shortcut, None, mc))
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
    .spacing(8)
    .align_y(iced::Alignment::Center);
    button(r)
        // Match the padding used by `menu_item_btn` so Export sits on the
        // same left/right grid as the normal leaf rows (Save, Open…).
        .padding([4, 12])
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
            .padding([2, 8])
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(mc.border)),
                ..container::Style::default()
            }),
    )
}

/// Build a single menu item button with label + shortcut text.
fn menu_item_btn(
    label: &str,
    shortcut: Option<&str>,
    msg: Option<MenuMessage>,
    mc: MenuColors,
) -> Element<'static, MenuMessage> {
    let enabled = msg.is_some();
    let text_c = if enabled { mc.text } else { mc.text_disabled };

    let label = label.to_owned();
    let mut r = row![
        text(label)
            .size(MENU_LABEL_SIZE)
            .color(text_c)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .width(Length::Fill);

    if let Some(sc) = shortcut {
        let sc = sc.to_owned();
        r = r.push(iced::widget::Space::new().width(Length::Fill));
        r = r.push(
            text(sc)
                .size(MENU_SHORTCUT_SIZE)
                .color(mc.text_muted)
                .wrapping(iced::widget::text::Wrapping::None),
        );
    }

    let hover_bg = mc.hover;
    let btn = button(r).padding([4, 12]).width(Length::Fill).style(
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
