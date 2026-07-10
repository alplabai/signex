//! Preferences dialog — Altium-style modal with left navigation + right content.
//!
//! Opened via Tools > Preferences (or keyboard shortcut).
//! Left side: tree of settings categories.
//! Right side: settings panel for the selected category.

use crate::render_config::{GridStyle, LabelStyle, MultisheetStyle, PowerPortStyle};
use iced::widget::{
    Column, Space, button, column, container, row, scrollable, svg, text, text_input,
};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_types::theme::ThemeId;

use crate::app::view::dialogs::{
    MODAL_CLOSE_X_HIT_H, MODAL_CLOSE_X_HIT_W, MODAL_CLOSE_X_HOVER, MODAL_CLOSE_X_ICON,
    MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE,
};
use crate::fonts;
use crate::styles::MODAL_CORNER_RADIUS;

// ─── Navigation Items ─────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefNav {
    Appearance,
    /// Keyboard-shortcut profile editor — pick / create / delete /
    /// import / export a profile and rebind individual commands via the
    /// chord recorder. Backed by `UiState::preferences_keymap_editor`.
    Keyboard,
    /// Electrical Rule Check — per-rule severity override.
    Erc,
    /// v0.9 Library settings — Distributor APIs, lifecycle defaults.
    LibraryDistributors,
    /// User-editable component-class registry — surfaces in the New
    /// Component / Edit Component class dropdowns. Backed by
    /// `prefs.json::component_classes`; defaults seed from
    /// `fonts::default_component_classes` on a fresh install.
    ComponentClasses,
    // Future: Editor, Shortcuts, ...
}

impl PrefNav {
    pub const ALL: &'static [PrefNav] = &[
        PrefNav::Appearance,
        PrefNav::Keyboard,
        PrefNav::Erc,
        PrefNav::LibraryDistributors,
        PrefNav::ComponentClasses,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PrefNav::Appearance => "Appearance",
            PrefNav::Keyboard => "Keyboard Shortcuts",
            PrefNav::Erc => "Electrical Rules",
            PrefNav::LibraryDistributors => "Distributor APIs",
            // Now a *seed* pane — the actual class registry is
            // per-library inside each `.snxlib`'s manifest. This
            // list is what every newly-created library gets seeded
            // with, so users can establish a personal taxonomy
            // baseline once and have new libraries inherit it.
            PrefNav::ComponentClasses => "Default Component Classes",
        }
    }

    pub fn group(self) -> &'static str {
        match self {
            PrefNav::Appearance => "System",
            PrefNav::Keyboard => "System",
            PrefNav::Erc => "Validation",
            PrefNav::LibraryDistributors => "Library",
            PrefNav::ComponentClasses => "Library",
        }
    }
}

// ─── Messages ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum PrefMsg {
    /// Navigate to a category.
    Nav(PrefNav),
    /// Close without saving (only if not dirty; app ignores if dirty).
    Close,
    /// Discard all unsaved changes and close.
    DiscardAndClose,
    /// Commit the current draft and keep the dialog open.
    Save,
    /// Update the draft theme (not applied until Save).
    DraftTheme(ThemeId),
    /// Update the draft UI font (not applied until Save).
    DraftFont(String),
    /// Update draft power port drawing style (applies as live preview).
    DraftPowerPortStyle(PowerPortStyle),
    /// Update draft global/hier label drawing style (applies as live preview).
    DraftLabelStyle(LabelStyle),
    /// Update draft hierarchical-sheet drawing style (applies as live preview).
    DraftMultisheetStyle(MultisheetStyle),
    /// Update draft visible-grid render style (applies as live preview).
    DraftGridStyle(GridStyle),
    /// Update the default symbol-editor grid size. Persisted on Save.
    DraftSymbolGridSize(f32),
    /// Update the symbol-editor grid style (applies immediately, persisted).
    DraftSymbolGridStyle(GridStyle),
    /// Open a file picker to import a custom theme JSON.
    ImportTheme,
    /// Save the current draft theme as a JSON file.
    ExportTheme,
    /// Loaded JSON content from an import pick dialog.
    ThemeFileLoaded(String),
    /// Set the severity override for an ERC rule. Setting the override
    /// to the default value clears the entry instead.
    DraftErcSeverity(signex_erc::RuleKind, signex_erc::Severity),
    /// Clear every ERC severity override — reset to defaults.
    ResetErcSeverities,
    /// Library → Distributor APIs pane forwarded a settings message.
    /// Folded into `PrefMsg` so the Preferences modal can mount the
    /// live panel as a first-class pane without breaking the
    /// existing `Message::Preferences(PreferencesMsg::Inner(_))` plumbing. The handler
    /// re-dispatches via `Message::Library(LibraryMessage::Settings)`.
    LibrarySettings(crate::library::messages::SettingsMsg),
    /// Component-class editor messages — all edits land on the
    /// `preferences_draft_component_classes` mirror so Cancel /
    /// Discard reverts cleanly. Save copies the draft into the live
    /// `component_classes` list and persists via
    /// `fonts::write_component_classes_pref`.
    ComponentClassEditKey {
        index: usize,
        key: String,
    },
    ComponentClassEditLabel {
        index: usize,
        label: String,
    },
    ComponentClassAdd,
    ComponentClassRemove {
        index: usize,
    },
    /// Reset the draft list to the seed defaults (`DEFAULT_COMPONENT_CLASSES`).
    ComponentClassResetDefaults,

    // ── Keyboard Shortcuts pane ──
    // All edits land on `UiState::preferences_keymap_editor` (a working
    // copy) so Cancel / Discard revert cleanly; Save commits the set
    // and recompiles the live keymap.
    /// Switch the active profile shown in the editor.
    KeymapProfileSelected(String),
    /// Fork the active profile into a new editable custom profile.
    KeymapCreateCustomProfile,
    /// Delete the active custom profile draft (built-ins are protected).
    KeymapDeleteActiveProfile,
    /// Open a file picker to import a custom profile (`.toml`).
    KeymapImportProfile,
    /// Loaded profile source from the import pick dialog.
    KeymapProfileLoaded(String),
    /// Save the active profile to a `.toml` file.
    KeymapExportProfile,
    /// Direct text edit of a binding's trigger (kept for API parity;
    /// the recorder is the primary edit path).
    KeymapBindingChanged {
        command: crate::keymap::AppCommandId,
        context: crate::keymap::ShortcutContext,
        trigger: String,
    },
    /// Open the chord recorder for a specific binding.
    KeymapRecorderOpen {
        command: crate::keymap::AppCommandId,
        label: String,
        context: crate::keymap::ShortcutContext,
        trigger: String,
    },
    /// Dismiss the recorder without applying.
    KeymapRecorderCancel,
    /// Arm the recorder to capture keystrokes.
    KeymapRecorderStart,
    /// Stop capturing (keeps what was recorded).
    KeymapRecorderStop,
    /// Clear the captured strokes and keep recording.
    KeymapRecorderClear,
    /// Live modifier state while recording (drives the transient hint).
    KeymapRecorderModifiersChanged(crate::keymap::Modifiers),
    /// A captured keystroke, routed from the keyboard subscription.
    KeymapRecorderKeyPressed(crate::keymap::KeyStroke),
    /// Apply the recorded chord to the binding under edit.
    KeymapRecorderApply,
}

// ─── Dialog sizes ─────────────────────────────────────────────

const DLG_W: f32 = 960.0;
const DLG_H: f32 = 660.0;
const NAV_W: f32 = 220.0;
const HDR_H: f32 = 40.0;
const FOOTER_H: f32 = 44.0;

// ─── Colors ───────────────────────────────────────────────────

const DLG_BG: Color = Color::from_rgb(0.13, 0.13, 0.15);
const NAV_BG: Color = Color::from_rgb(0.10, 0.10, 0.12);
const CONTENT_BG: Color = Color::from_rgb(0.15, 0.15, 0.17);
const HDR_BG: Color = Color::from_rgb(0.11, 0.11, 0.13);
const ROW_ACTIVE: Color = Color::from_rgb(0.17, 0.30, 0.50);
const ROW_HOVER: Color = Color::from_rgb(0.18, 0.18, 0.21);
const SEP: Color = Color::from_rgb(0.22, 0.22, 0.25);
const TEXT_PRI: Color = Color::from_rgb(0.90, 0.90, 0.92);
const TEXT_MUT: Color = Color::from_rgb(0.50, 0.50, 0.55);
const WARN_YELLOW: Color = Color::from_rgb(0.95, 0.72, 0.15);
const BTN_IMPORT: Color = Color::from_rgb(0.18, 0.26, 0.40);
const BTN_IMPORT_HOV: Color = Color::from_rgb(0.24, 0.34, 0.52);
const BTN_DANGER: Color = Color::from_rgb(0.38, 0.16, 0.16);
const BTN_DANGER_HOV: Color = Color::from_rgb(0.50, 0.22, 0.22);

// ─── Public view ──────────────────────────────────────────────

/// Build the full-screen backdrop + centred dialog.
///
/// * `draft_theme`      — theme currently selected in the dialog (not yet saved)
/// * `saved_theme`      — the committed theme (used to detect unsaved changes)
/// * `draft_font`       — UI font name pending save
/// * `custom_name`      — name of the loaded custom theme (if any)
/// * `dirty`            — whether there are unsaved changes
#[allow(clippy::too_many_arguments)]
pub fn view<'a>(
    nav: PrefNav,
    draft_theme: ThemeId,
    saved_theme: ThemeId,
    draft_font: &str,
    draft_power_port_style: PowerPortStyle,
    draft_label_style: LabelStyle,
    draft_multisheet_style: MultisheetStyle,
    draft_grid_style: GridStyle,
    draft_symbol_grid_size_mm: f32,
    draft_symbol_grid_style: GridStyle,
    custom_name: Option<&'a str>,
    dirty: bool,
    erc_overrides: &'a std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
    distributor_settings: &'a crate::library::state::DistributorSettings,
    panel_tokens: &'a signex_types::theme::ThemeTokens,
    draft_component_classes: &'a [crate::fonts::ComponentClassEntry],
    keymap_editor: &'a crate::keymap::KeymapEditorModel,
    keymap_status: &'a str,
    keymap_recorder: Option<&'a crate::app::KeymapRecorderState>,
    theme_id: ThemeId,
) -> Element<'a, PrefMsg> {
    let dialog = build_dialog(
        nav,
        draft_theme,
        saved_theme,
        draft_font,
        draft_power_port_style,
        draft_label_style,
        draft_multisheet_style,
        draft_grid_style,
        draft_symbol_grid_size_mm,
        draft_symbol_grid_style,
        custom_name,
        dirty,
        erc_overrides,
        distributor_settings,
        panel_tokens,
        draft_component_classes,
        keymap_editor,
        keymap_status,
        keymap_recorder,
        theme_id,
    );

    container(
        column![
            Space::new().height(Length::Fill),
            row![
                Space::new().width(Length::Fill),
                dialog,
                Space::new().width(Length::Fill),
            ],
            Space::new().height(Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.65))),
        ..container::Style::default()
    })
    .into()
}

// ─── Dialog shell ─────────────────────────────────────────────

/// Render the Preferences dialog body without the centred backdrop —
/// used by `view_detached_modal` so the dialog fills its own OS window.
/// In-window callers go through `view()` which wraps this in a tinted
/// dismiss layer.
#[allow(clippy::too_many_arguments)]
pub(crate) fn view_body<'a>(
    nav: PrefNav,
    draft_theme: ThemeId,
    saved_theme: ThemeId,
    draft_font: &'a str,
    draft_power_port_style: PowerPortStyle,
    draft_label_style: LabelStyle,
    draft_multisheet_style: MultisheetStyle,
    draft_grid_style: GridStyle,
    draft_symbol_grid_size_mm: f32,
    draft_symbol_grid_style: GridStyle,
    custom_name: Option<&'a str>,
    dirty: bool,
    erc_overrides: &'a std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
    distributor_settings: &'a crate::library::state::DistributorSettings,
    panel_tokens: &'a signex_types::theme::ThemeTokens,
    draft_component_classes: &'a [crate::fonts::ComponentClassEntry],
    keymap_editor: &'a crate::keymap::KeymapEditorModel,
    keymap_status: &'a str,
    keymap_recorder: Option<&'a crate::app::KeymapRecorderState>,
    theme_id: ThemeId,
) -> Element<'a, PrefMsg> {
    build_dialog(
        nav,
        draft_theme,
        saved_theme,
        draft_font,
        draft_power_port_style,
        draft_label_style,
        draft_multisheet_style,
        draft_grid_style,
        draft_symbol_grid_size_mm,
        draft_symbol_grid_style,
        custom_name,
        dirty,
        erc_overrides,
        distributor_settings,
        panel_tokens,
        draft_component_classes,
        keymap_editor,
        keymap_status,
        keymap_recorder,
        theme_id,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_dialog<'a>(
    nav: PrefNav,
    draft_theme: ThemeId,
    saved_theme: ThemeId,
    draft_font: &str,
    draft_power_port_style: PowerPortStyle,
    draft_label_style: LabelStyle,
    draft_multisheet_style: MultisheetStyle,
    draft_grid_style: GridStyle,
    draft_symbol_grid_size_mm: f32,
    draft_symbol_grid_style: GridStyle,
    custom_name: Option<&'a str>,
    dirty: bool,
    erc_overrides: &'a std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
    distributor_settings: &'a crate::library::state::DistributorSettings,
    panel_tokens: &'a signex_types::theme::ThemeTokens,
    draft_component_classes: &'a [crate::fonts::ComponentClassEntry],
    keymap_editor: &'a crate::keymap::KeymapEditorModel,
    keymap_status: &'a str,
    keymap_recorder: Option<&'a crate::app::KeymapRecorderState>,
    theme_id: ThemeId,
) -> Element<'a, PrefMsg> {
    // ── Header ── canonical modal chrome (28px, asymmetric padding,
    // SVG close-X with red hover) — same shape every other modal in
    // the app uses so the chrome stays consistent across surfaces.
    let header = container(
        row![
            text("Preferences")
                .size(MODAL_HEADER_TITLE_SIZE)
                .color(TEXT_PRI),
            Space::new().width(Length::Fill),
            close_btn(theme_id),
        ]
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .height(MODAL_HEADER_HEIGHT)
    .padding(MODAL_HEADER_PADDING)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(HDR_BG)),
        border: Border {
            width: 0.0,
            radius: iced::border::Radius::default()
                .top_left(MODAL_CORNER_RADIUS)
                .top_right(MODAL_CORNER_RADIUS),
            ..Border::default()
        },
        ..container::Style::default()
    });

    // ── Body: nav | divider | content ──
    let body = row![
        build_nav(nav),
        // Vertical divider
        container(Space::new())
            .width(1)
            .height(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(SEP)),
                ..container::Style::default()
            }),
        build_content(
            nav,
            draft_theme,
            saved_theme,
            draft_font,
            draft_power_port_style,
            draft_label_style,
            draft_multisheet_style,
            draft_grid_style,
            draft_symbol_grid_size_mm,
            draft_symbol_grid_style,
            custom_name,
            erc_overrides,
            distributor_settings,
            panel_tokens,
            draft_component_classes,
            keymap_editor,
            keymap_status,
            keymap_recorder,
        ),
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    // ── Footer (only when dirty) ──
    let footer_opt = build_footer(dirty);

    // ── Assemble ──
    let h_divider = || -> Element<'a, PrefMsg> {
        container(Space::new())
            .width(Length::Fill)
            .height(1)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(SEP)),
                ..container::Style::default()
            })
            .into()
    };

    let mut col_items: Vec<Element<'a, PrefMsg>> = vec![header.into(), h_divider(), body.into()];
    if let Some(footer) = footer_opt {
        col_items.push(h_divider());
        col_items.push(footer);
    }

    container(Column::with_children(col_items).spacing(0))
        .width(DLG_W)
        .height(DLG_H)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(DLG_BG)),
            border: Border {
                width: 1.0,
                radius: MODAL_CORNER_RADIUS.into(),
                color: SEP,
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.7),
                offset: iced::Vector::new(0.0, 8.0),
                blur_radius: 24.0,
            },
            ..container::Style::default()
        })
        .clip(true)
        .into()
}

// ─── Left navigation ──────────────────────────────────────────

fn build_nav<'a>(active: PrefNav) -> Element<'a, PrefMsg> {
    let mut col = column![].spacing(0).width(NAV_W);

    // Group headers + items
    let mut last_group = "";
    for &item in PrefNav::ALL {
        let group = item.group();
        if group != last_group {
            last_group = group;
            col = col.push(
                container(text(group.to_uppercase()).size(9).color(TEXT_MUT))
                    .padding(iced::Padding {
                        top: 10.0,
                        right: 12.0,
                        bottom: 4.0,
                        left: 12.0,
                    })
                    .width(Length::Fill),
            );
        }
        col = col.push(nav_item(item, active));
    }

    container(scrollable(col).width(Length::Fill))
        .width(NAV_W)
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(NAV_BG)),
            ..container::Style::default()
        })
        .into()
}

fn nav_item<'a>(item: PrefNav, active: PrefNav) -> Element<'a, PrefMsg> {
    let is_active = item == active;
    let bg = if is_active {
        Some(Background::Color(ROW_ACTIVE))
    } else {
        None
    };
    let tc = if is_active { Color::WHITE } else { TEXT_PRI };

    button(
        container(row![text(item.label()).size(12).color(tc),].align_y(iced::Alignment::Center))
            .padding([6, 12])
            .width(Length::Fill),
    )
    .padding(0)
    .width(Length::Fill)
    .on_press(PrefMsg::Nav(item))
    .style(move |_: &Theme, status: button::Status| {
        let bg = match (is_active, status) {
            (true, _) => Some(Background::Color(ROW_ACTIVE)),
            (false, button::Status::Hovered) => Some(Background::Color(ROW_HOVER)),
            _ => bg,
        };
        button::Style {
            background: bg,
            border: Border::default(),
            text_color: tc,
            ..button::Style::default()
        }
    })
    .into()
}

// ─── Right content ────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn build_content<'a>(
    nav: PrefNav,
    draft_theme: ThemeId,
    saved_theme: ThemeId,
    draft_font: &str,
    draft_power_port_style: PowerPortStyle,
    draft_label_style: LabelStyle,
    draft_multisheet_style: MultisheetStyle,
    draft_grid_style: GridStyle,
    draft_symbol_grid_size_mm: f32,
    draft_symbol_grid_style: GridStyle,
    custom_name: Option<&'a str>,
    erc_overrides: &'a std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
    distributor_settings: &'a crate::library::state::DistributorSettings,
    panel_tokens: &'a signex_types::theme::ThemeTokens,
    draft_component_classes: &'a [crate::fonts::ComponentClassEntry],
    keymap_editor: &'a crate::keymap::KeymapEditorModel,
    keymap_status: &'a str,
    keymap_recorder: Option<&'a crate::app::KeymapRecorderState>,
) -> Element<'a, PrefMsg> {
    let inner = match nav {
        PrefNav::Appearance => content_appearance(
            draft_theme,
            saved_theme,
            draft_font,
            draft_power_port_style,
            draft_label_style,
            draft_multisheet_style,
            draft_grid_style,
            draft_symbol_grid_size_mm,
            draft_symbol_grid_style,
            custom_name,
        ),
        PrefNav::Erc => content_erc(erc_overrides),
        // Library → Distributor APIs — the library subsystem owns
        // the actual form
        // (`crate::library::settings::distributor_apis::view`); we
        // re-emit its `LibraryMessage::Settings(_)` wrapper as
        // `PrefMsg::LibrarySettings(_)` so the Preferences modal's
        // single message bus stays cohesive. The handler in
        // `app/handlers/preferences.rs` re-dispatches via
        // `Message::Library` to keep the canonical settings state on
        // `LibraryState.settings`.
        PrefNav::LibraryDistributors => {
            content_library_distributors(distributor_settings, panel_tokens)
        }
        PrefNav::Keyboard => {
            content_keyboard_shortcuts(keymap_editor, keymap_status, keymap_recorder)
        }
        PrefNav::ComponentClasses => content_component_classes(draft_component_classes),
    };

    container(scrollable(inner).width(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(0)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(CONTENT_BG)),
            ..container::Style::default()
        })
        .into()
}

// ─── Library Distributor APIs page (live) ─────────────────

/// Mount the live Distributor APIs panel inside the Preferences modal.
///
/// The panel emits `LibraryMessage::Settings(_)`; we wrap every
/// message in `PrefMsg::LibrarySettings(_)` so the modal's outer
/// `Message::Preferences(PreferencesMsg::Inner(_))` map stays a single layer. The
/// `app/handlers/preferences.rs` handler unwraps and re-dispatches
/// via `Message::Library` so the canonical state writeback runs
/// through the same dispatcher the Tools-menu surface uses.
fn content_library_distributors<'a>(
    settings: &'a crate::library::state::DistributorSettings,
    tokens: &'a signex_types::theme::ThemeTokens,
) -> Element<'a, PrefMsg> {
    let header: Element<'a, PrefMsg> = column![
        section_title("Library — Distributor APIs"),
        Space::new().height(8),
    ]
    .padding([16, 20])
    .into();

    // The library panel returns `Element<'a, LibraryMessage>`. We
    // map to `PrefMsg::LibrarySettings` for every Settings sub-
    // variant; non-Settings library messages are ignored (they're
    // never produced by `distributor_apis::view`).
    let pane =
        crate::library::settings::distributor_apis::view(settings, tokens).map(|lm| match lm {
            crate::library::messages::LibraryMessage::Settings(s) => PrefMsg::LibrarySettings(s),
            // distributor_apis::view never produces anything else.
            _ => PrefMsg::Close,
        });

    column![header, container(pane).padding([0, 20]).width(Length::Fill)]
        .spacing(0)
        .into()
}

// ─── Appearance page ──────────────────────────────────────────

fn content_appearance<'a>(
    draft_theme: ThemeId,
    _saved_theme: ThemeId,
    draft_font: &str,
    draft_power_port_style: PowerPortStyle,
    draft_label_style: LabelStyle,
    draft_multisheet_style: MultisheetStyle,
    draft_grid_style: GridStyle,
    draft_symbol_grid_size_mm: f32,
    draft_symbol_grid_style: GridStyle,
    custom_name: Option<&'a str>,
) -> Element<'a, PrefMsg> {
    let mut col = column![].spacing(0).padding([16, 20]);

    // ── Section: Theme ──
    col = col.push(section_title("Theme"));
    col = col.push(Space::new().height(10));

    // Built-in theme data: (id, display name, description)
    let builtins: &[(ThemeId, &str, &str)] = &[
        (
            ThemeId::Signex,
            "Signex",
            "Default Signex schematic palette",
        ),
        (
            ThemeId::Alplab,
            "Alp Lab",
            "Alp Lab brand cyan accent on the Signex chrome",
        ),
        (
            ThemeId::VsCodeDark,
            "VS Code Dark",
            "VS Code inspired dark theme",
        ),
        (
            ThemeId::CatppuccinMocha,
            "Catppuccin Mocha",
            "Warm soft dark with pastels",
        ),
        (
            ThemeId::GitHubDark,
            "GitHub Dark",
            "GitHub's dark mode colors",
        ),
        (
            ThemeId::SolarizedLight,
            "Solarized Light",
            "Warm light tone-on-tone",
        ),
        (ThemeId::Nord, "Nord", "Arctic blue cool dark theme"),
    ];

    // Build list of all theme entries including optional custom
    let mut entries: Vec<(ThemeId, String, &'static str)> = builtins
        .iter()
        .map(|&(id, name, desc)| (id, name.to_string(), desc))
        .collect();
    if let Some(name) = custom_name {
        entries.push((
            ThemeId::Custom,
            format!("\u{2728} {name}"),
            "Custom imported theme",
        ));
    }

    // Rows of 2
    let mut i = 0;
    while i < entries.len() {
        let (id_a, ref name_a, desc_a) = entries[i];
        let card_a = theme_card(id_a, name_a, desc_a, draft_theme);
        let row_elem: Element<'_, PrefMsg> = if i + 1 < entries.len() {
            let (id_b, ref name_b, desc_b) = entries[i + 1];
            let card_b = theme_card(id_b, name_b, desc_b, draft_theme);
            row![card_a, Space::new().width(12), card_b]
                .width(Length::Fill)
                .into()
        } else {
            row![card_a, Space::new().width(Length::Fill)]
                .width(Length::Fill)
                .into()
        };
        col = col.push(row_elem);
        col = col.push(Space::new().height(10));
        i += 2;
    }

    // ── Custom theme import/export ──
    col = col.push(Space::new().height(4));
    col = col.push(
        row![
            import_btn(),
            Space::new().width(8),
            export_btn(),
            Space::new().width(Length::Fill),
        ]
        .align_y(iced::Alignment::Center),
    );

    // ── Divider ──
    col = col.push(Space::new().height(16));
    col = col.push(h_sep());
    col = col.push(Space::new().height(16));

    // ── Section: UI Font ──
    col = col.push(section_title("Font"));
    col = col.push(Space::new().height(10));
    col = col.push(
        row![
            column![
                text("UI Font").size(12).color(TEXT_PRI),
                text("Applies to all panels and menus. Requires restart.")
                    .size(10)
                    .color(TEXT_MUT),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            {
                let families = fonts::system_font_families();
                let current_owned = draft_font.to_string();
                iced::widget::pick_list(
                    families.as_slice(),
                    Some(current_owned),
                    PrefMsg::DraftFont,
                )
                .text_size(12)
                .width(200)
            },
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(Space::new().height(20));

    // ── Section: Schematic Editor ──
    col = col.push(h_sep());
    col = col.push(Space::new().height(16));
    col = col.push(section_title("Schematic Editor"));
    col = col.push(Space::new().height(10));
    col = col.push(
        row![
            column![
                text("Grid Style").size(12).color(TEXT_PRI),
                text("Appearance of snap points on the schematic canvas.")
                    .size(10)
                    .color(TEXT_MUT),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(
                GridStyle::ALL,
                Some(draft_grid_style),
                PrefMsg::DraftGridStyle,
            )
            .text_size(12)
            .width(200),
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(Space::new().height(20));

    // ── Section: Power Port Symbols ──
    col = col.push(section_title("Power Ports"));
    col = col.push(Space::new().height(10));
    col = col.push(
        row![
            column![
                text("Power Port Style").size(12).color(TEXT_PRI),
                text("Choose how power symbols are rendered on canvas.")
                    .size(10)
                    .color(TEXT_MUT),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(
                [PowerPortStyle::Altium, PowerPortStyle::Standard],
                Some(draft_power_port_style),
                PrefMsg::DraftPowerPortStyle,
            )
            .text_size(12)
            .width(200),
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(Space::new().height(12));
    col = col.push(
        text("Restyled mode reshapes pin labels for rendering only. Standard preserves the library symbol's authored appearance.")
            .size(10)
            .color(TEXT_MUT),
    );
    col = col.push(Space::new().height(16));
    col = col.push(
        row![
            column![
                text("Global/Hier Label Style").size(12).color(TEXT_PRI),
                text("Controls multi-sheet and global label appearance.")
                    .size(10)
                    .color(TEXT_MUT),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(
                [LabelStyle::Standard, LabelStyle::Altium],
                Some(draft_label_style),
                PrefMsg::DraftLabelStyle,
            )
            .text_size(12)
            .width(200),
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(Space::new().height(16));
    col = col.push(
        row![
            column![
                text("Multisheet Style").size(12).color(TEXT_PRI),
                text(
                    "Controls hierarchical sheet body fill defaults. \
                     Per-sheet colours from the source file always win."
                )
                .size(10)
                .color(TEXT_MUT),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(
                [MultisheetStyle::Standard, MultisheetStyle::Altium],
                Some(draft_multisheet_style),
                PrefMsg::DraftMultisheetStyle,
            )
            .text_size(12)
            .width(200),
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(Space::new().height(20));

    // ── Section: Symbol Editor ──
    col = col.push(h_sep());
    col = col.push(Space::new().height(16));
    col = col.push(section_title("Symbol Editor"));
    col = col.push(Space::new().height(10));
    col = col.push(
        row![
            column![
                text("Default Grid Size").size(12).color(TEXT_PRI),
                text(
                    "Applied when a symbol library is first opened. \
                      Can be changed per-library from the canvas status bar."
                )
                .size(10)
                .color(TEXT_MUT),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(
                crate::canvas::grid::GRID_SIZE_LABELS,
                crate::canvas::grid::GRID_SIZES_MM
                    .iter()
                    .zip(crate::canvas::grid::GRID_SIZE_LABELS.iter())
                    .find(|(sz, _)| (**sz - draft_symbol_grid_size_mm).abs() < 0.001)
                    .map(|(_, lbl)| *lbl),
                |lbl: &'static str| {
                    let mm = crate::canvas::grid::GRID_SIZES_MM
                        .iter()
                        .zip(crate::canvas::grid::GRID_SIZE_LABELS.iter())
                        .find(|(_, l)| **l == lbl)
                        .map(|(sz, _)| *sz)
                        .unwrap_or(1.27);
                    PrefMsg::DraftSymbolGridSize(mm)
                },
            )
            .text_size(12)
            .width(200),
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(Space::new().height(16));
    col = col.push(
        row![
            column![
                text("Grid Style").size(12).color(TEXT_PRI),
                text("Appearance of snap points on the symbol editor canvas.")
                    .size(10)
                    .color(TEXT_MUT),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(
                GridStyle::ALL,
                Some(draft_symbol_grid_style),
                PrefMsg::DraftSymbolGridStyle,
            )
            .text_size(12)
            .width(200),
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(Space::new().height(20));

    col.into()
}

// ─── Widget helpers ───────────────────────────────────────────

fn section_title<'a>(title: &str) -> Element<'a, PrefMsg> {
    column![
        text(title.to_owned()).size(13).color(TEXT_PRI),
        container(Space::new())
            .width(Length::Fill)
            .height(1)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(SEP)),
                ..container::Style::default()
            }),
    ]
    .spacing(6)
    .into()
}

fn h_sep<'a>() -> Element<'a, PrefMsg> {
    container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(SEP)),
            ..container::Style::default()
        })
        .into()
}

fn theme_card<'a>(
    id: ThemeId,
    name: &str,
    desc: &'static str,
    current: ThemeId,
) -> Element<'a, PrefMsg> {
    let is_active = id == current;
    let border_c = if is_active {
        Color::from_rgb(0.30, 0.55, 0.90)
    } else {
        SEP
    };
    let label = format!("{}{name}", if is_active { "✓ " } else { "" });
    let card_bg = if is_active {
        Color::from_rgb(0.13, 0.21, 0.35)
    } else {
        Color::from_rgb(0.17, 0.17, 0.20)
    };
    let text_color = if is_active { Color::WHITE } else { TEXT_PRI };
    let hover_bg = Color::from_rgb(0.20, 0.20, 0.24);
    let msg = PrefMsg::DraftTheme(id);

    button(
        container(
            column![
                text(label).size(12).color(text_color),
                text(desc)
                    .size(10)
                    .color(TEXT_MUT)
                    .wrapping(iced::widget::text::Wrapping::None),
            ]
            .spacing(4),
        )
        .padding([10, 12])
        .width(Length::Fill),
    )
    .padding(0)
    .width(Length::Fill)
    .on_press(msg)
    .style(move |_: &Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered | button::Status::Pressed => Background::Color(hover_bg),
            _ => Background::Color(card_bg),
        };
        button::Style {
            background: Some(bg),
            border: Border {
                width: 1.0,
                radius: 4.0.into(),
                color: border_c,
            },
            ..button::Style::default()
        }
    })
    .into()
}

/// Canonical close-X — same SVG glyph and red hover footprint as the
/// shared `view::dialogs::close_x_button`, generic over the modal's
/// message type so this version can compose into a `PrefMsg`
/// element. Matches the main-window chrome close (white glyph,
/// 46×28 hit-box, top-right rounded hover, Windows-native red).
fn close_btn<'a>(theme_id: ThemeId) -> Element<'a, PrefMsg> {
    let handle = crate::icons::icon_chrome_window_close(theme_id);
    button(
        container(
            svg(handle)
                .width(MODAL_CLOSE_X_ICON)
                .height(MODAL_CLOSE_X_ICON)
                .style(move |_: &Theme, _| svg::Style {
                    color: Some(Color::WHITE),
                }),
        )
        .width(MODAL_CLOSE_X_HIT_W)
        .height(MODAL_CLOSE_X_HIT_H)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center),
    )
    .padding(0)
    .on_press(PrefMsg::Close)
    .style(move |_: &Theme, status: button::Status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: if hovered {
                Some(Background::Color(MODAL_CLOSE_X_HOVER))
            } else {
                None
            },
            text_color: Color::WHITE,
            border: Border {
                radius: iced::border::Radius {
                    top_left: 0.0,
                    top_right: 4.0,
                    bottom_left: 0.0,
                    bottom_right: 0.0,
                },
                ..Border::default()
            },
            ..button::Style::default()
        }
    })
    .into()
}

/// Dynamic footer: Save + Close (clean) or ⚠ + Discard + Save (dirty).
/// Footer is rendered only when dirty — the X button in the header is the
/// canonical close action; a redundant "Close" button at the bottom is
/// noise. Save / Discard appear when there are unsaved changes.
fn build_footer<'a>(dirty: bool) -> Option<Element<'a, PrefMsg>> {
    if !dirty {
        return None;
    }
    let footer_row: Element<'a, PrefMsg> = row![
        text("● Unsaved changes").size(11).color(WARN_YELLOW),
        Space::new().width(Length::Fill),
        discard_btn(),
        Space::new().width(8),
        save_btn(),
    ]
    .align_y(iced::Alignment::Center)
    .into();

    Some(
        container(footer_row)
            .width(Length::Fill)
            .height(FOOTER_H)
            .padding([0, 16])
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(HDR_BG)),
                border: Border {
                    width: 1.0,
                    color: SEP,
                    radius: iced::border::Radius::default()
                        .bottom_left(MODAL_CORNER_RADIUS)
                        .bottom_right(MODAL_CORNER_RADIUS),
                },
                ..container::Style::default()
            })
            .into(),
    )
}

fn save_btn<'a>() -> Element<'a, PrefMsg> {
    button(text("Save").size(12).color(Color::WHITE))
        .padding([6, 20])
        .on_press(PrefMsg::Save)
        .style(|_: &Theme, status: button::Status| {
            let bg = match status {
                button::Status::Hovered => Color::from_rgb(0.18, 0.52, 0.30),
                _ => Color::from_rgb(0.14, 0.42, 0.24),
            };
            button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    radius: 3.0.into(),
                    ..Border::default()
                },
                text_color: Color::WHITE,
                ..button::Style::default()
            }
        })
        .into()
}

fn discard_btn<'a>() -> Element<'a, PrefMsg> {
    button(
        text("Discard & Close")
            .size(12)
            .color(Color::from_rgb(0.85, 0.60, 0.60)),
    )
    .padding([6, 16])
    .on_press(PrefMsg::DiscardAndClose)
    .style(|_: &Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered => BTN_DANGER_HOV,
            _ => BTN_DANGER,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                radius: 3.0.into(),
                ..Border::default()
            },
            text_color: Color::from_rgb(0.90, 0.70, 0.70),
            ..button::Style::default()
        }
    })
    .into()
}

fn close_footer_btn<'a>() -> Element<'a, PrefMsg> {
    button(text("Close").size(12).color(Color::WHITE))
        .padding([6, 20])
        .on_press(PrefMsg::Close)
        .style(|_: &Theme, status: button::Status| {
            let bg = match status {
                button::Status::Hovered => Color::from_rgb(0.28, 0.42, 0.65),
                _ => Color::from_rgb(0.22, 0.36, 0.58),
            };
            button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    radius: 3.0.into(),
                    ..Border::default()
                },
                text_color: Color::WHITE,
                ..button::Style::default()
            }
        })
        .into()
}

fn import_btn<'a>() -> Element<'a, PrefMsg> {
    button(text("⬆ Import Theme…").size(11).color(TEXT_PRI))
        .padding([5, 12])
        .on_press(PrefMsg::ImportTheme)
        .style(|_: &Theme, status: button::Status| {
            let bg = match status {
                button::Status::Hovered => BTN_IMPORT_HOV,
                _ => BTN_IMPORT,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: SEP,
                },
                text_color: TEXT_PRI,
                ..button::Style::default()
            }
        })
        .into()
}

fn export_btn<'a>() -> Element<'a, PrefMsg> {
    button(text("⬇ Export Theme…").size(11).color(TEXT_MUT))
        .padding([5, 12])
        .on_press(PrefMsg::ExportTheme)
        .style(|_: &Theme, status: button::Status| {
            let bg = match status {
                button::Status::Hovered => BTN_IMPORT_HOV,
                _ => BTN_IMPORT,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: SEP,
                },
                text_color: TEXT_MUT,
                ..button::Style::default()
            }
        })
        .into()
}

// === ERC Severity content ===

fn content_erc<'a>(
    overrides: &'a std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
) -> Element<'a, PrefMsg> {
    use signex_erc::{RuleKind, Severity};
    const RULES: &[RuleKind] = &[
        RuleKind::UnusedPin,
        RuleKind::DuplicateRefDesignator,
        RuleKind::HierPortDisconnected,
        RuleKind::DanglingWire,
        RuleKind::NetLabelConflict,
        RuleKind::OrphanLabel,
        RuleKind::BusBitWidthMismatch,
        RuleKind::BadHierSheetPin,
        RuleKind::MissingPowerFlag,
        RuleKind::PowerPortShort,
        RuleKind::SymbolOutsideSheet,
    ];
    const CHOICES: &[Severity] = &[
        Severity::Error,
        Severity::Warning,
        Severity::Info,
        Severity::Off,
    ];

    let header = column![
        text("Electrical Rules Severity").size(15).color(TEXT_PRI),
        Space::new().height(4),
        text("Per-rule severity override. Errors show red, Warnings yellow, Info blue; Off silences the rule entirely.")
            .size(11)
            .color(TEXT_MUT),
    ]
    .padding([16, 20]);

    let mut rows_col = column![].spacing(0).padding([0, 20]);
    for rule in RULES {
        let current = overrides
            .get(rule)
            .copied()
            .unwrap_or_else(|| rule.default_severity());
        let default_sev = rule.default_severity();
        let mut row_ui = row![
            text(rule.label())
                .size(12)
                .color(TEXT_PRI)
                .width(Length::Fill)
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center);
        for &sev in CHOICES {
            let active = current == sev;
            let r = *rule;
            let s = sev;
            let d = default_sev;
            row_ui = row_ui.push(
                button(text(severity_label(sev)).size(11).color(if active {
                    Color::from_rgb(1.0, 1.0, 1.0)
                } else {
                    TEXT_MUT
                }))
                .padding([4, 10])
                .on_press(PrefMsg::DraftErcSeverity(r, if s == d { d } else { s }))
                .style(move |_: &Theme, status: button::Status| {
                    let bg = if active {
                        severity_bg(sev)
                    } else if matches!(status, button::Status::Hovered) {
                        ROW_HOVER
                    } else {
                        NAV_BG
                    };
                    button::Style {
                        background: Some(Background::Color(bg)),
                        border: Border {
                            width: 1.0,
                            radius: 3.0.into(),
                            color: if active { severity_bg(sev) } else { SEP },
                        },
                        ..button::Style::default()
                    }
                }),
            );
        }
        rows_col = rows_col.push(container(row_ui).padding([6, 4]).width(Length::Fill).style(
            |_: &Theme| container::Style {
                background: None,
                border: Border {
                    width: 1.0,
                    color: SEP,
                    radius: 0.0.into(),
                },
                ..container::Style::default()
            },
        ));
    }

    let reset_row = container(
        row![
            Space::new().width(Length::Fill),
            button(text("Reset to defaults").size(11).color(TEXT_MUT))
                .padding([5, 12])
                .on_press(PrefMsg::ResetErcSeverities)
                .style(|_: &Theme, status: button::Status| {
                    let bg = match status {
                        button::Status::Hovered => BTN_IMPORT_HOV,
                        _ => BTN_IMPORT,
                    };
                    button::Style {
                        background: Some(Background::Color(bg)),
                        border: Border {
                            width: 1.0,
                            radius: 3.0.into(),
                            color: SEP,
                        },
                        text_color: TEXT_MUT,
                        ..button::Style::default()
                    }
                }),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([12, 20])
    .width(Length::Fill);

    column![header, rows_col, reset_row].spacing(0).into()
}

fn severity_label(sev: signex_erc::Severity) -> &'static str {
    match sev {
        signex_erc::Severity::Error => "Error",
        signex_erc::Severity::Warning => "Warning",
        signex_erc::Severity::Info => "Info",
        signex_erc::Severity::Off => "Off",
    }
}

fn severity_bg(sev: signex_erc::Severity) -> Color {
    match sev {
        signex_erc::Severity::Error => Color::from_rgb(0.58, 0.20, 0.22),
        signex_erc::Severity::Warning => Color::from_rgb(0.55, 0.45, 0.12),
        signex_erc::Severity::Info => Color::from_rgb(0.20, 0.36, 0.58),
        signex_erc::Severity::Off => Color::from_rgb(0.28, 0.28, 0.32),
    }
}

// ─── Keyboard Shortcuts editor ────────────────────────────────

fn content_keyboard_shortcuts<'a>(
    editor: &'a crate::keymap::KeymapEditorModel,
    status: &'a str,
    recorder: Option<&'a crate::app::KeymapRecorderState>,
) -> Element<'a, PrefMsg> {
    let profiles = editor.profiles();
    let active = profiles.iter().find(|profile| profile.active);
    let active_option = active.map(KeymapProfileOption::from);
    let active_is_custom = active
        .map(|profile| profile.kind == crate::keymap::ShortcutProfileKind::Custom)
        .unwrap_or(false);
    let active_summary = active
        .map(|profile| format!("{} bindings", profile.binding_count))
        .unwrap_or_else(|| "No active profile".to_string());

    let profile_options: Vec<KeymapProfileOption> =
        profiles.iter().map(KeymapProfileOption::from).collect();

    // Delete is only wired for custom profiles — built-ins are
    // protected by the model, so the button is inert on a built-in.
    let delete_button = {
        let base = button(container(text("Delete").size(11).color(Color::WHITE)).padding([5, 12]))
            .style(move |_: &Theme, status: button::Status| {
                let bg = match status {
                    button::Status::Hovered | button::Status::Pressed => BTN_DANGER_HOV,
                    _ => BTN_DANGER,
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: Color::WHITE,
                    border: Border {
                        radius: 3.0.into(),
                        ..Border::default()
                    },
                    ..button::Style::default()
                }
            });
        if active_is_custom {
            base.on_press(PrefMsg::KeymapDeleteActiveProfile)
        } else {
            base
        }
    };

    let header = column![
        section_title("Keyboard Shortcuts"),
        Space::new().height(4),
        text("Configure keyboard shortcut profiles. Command names and categories come from Signex command metadata.")
            .size(11)
            .color(TEXT_MUT),
    ]
    .spacing(6);

    let profile_row = row![
        column![
            text("Profile").size(12).color(TEXT_PRI),
            text(active_summary).size(10).color(TEXT_MUT),
        ]
        .spacing(3)
        .width(160),
        iced::widget::pick_list(profile_options, active_option, |profile| {
            PrefMsg::KeymapProfileSelected(profile.id)
        })
        .text_size(12)
        .width(180),
        button(container(text("Create").size(11).color(Color::WHITE)).padding([5, 12]))
            .on_press(PrefMsg::KeymapCreateCustomProfile)
            .style(move |_: &Theme, status: button::Status| {
                let bg = match status {
                    button::Status::Hovered | button::Status::Pressed => BTN_IMPORT_HOV,
                    _ => BTN_IMPORT,
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: Color::WHITE,
                    border: Border {
                        radius: 3.0.into(),
                        ..Border::default()
                    },
                    ..button::Style::default()
                }
            }),
        delete_button,
        button(container(text("Import").size(11).color(TEXT_PRI)).padding([5, 12]))
            .on_press(PrefMsg::KeymapImportProfile)
            .style(secondary_button_style),
        button(container(text("Export").size(11).color(TEXT_PRI)).padding([5, 12]))
            .on_press(PrefMsg::KeymapExportProfile)
            .style(secondary_button_style),
        Space::new().width(Length::Fill),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let conflicts = editor.active_conflicts();
    let conflict_count = conflicts.len();
    // A non-empty status (parse / save error, profile action) wins;
    // otherwise fall back to the conflict summary.
    let status_line = if !status.is_empty() {
        status.to_string()
    } else if conflict_count == 1 {
        "1 conflict in active profile".to_string()
    } else if conflict_count > 1 {
        format!("{conflict_count} conflicts in active profile")
    } else {
        "No conflicts detected in active keyboard shortcuts.".to_string()
    };

    let table_header = row![
        container(text("Category").size(11).color(TEXT_MUT)).width(Length::FillPortion(2)),
        container(text("Command").size(11).color(TEXT_MUT)).width(Length::FillPortion(4)),
        container(text("Context").size(11).color(TEXT_MUT)).width(Length::FillPortion(2)),
        container(text("Shortcut").size(11).color(TEXT_MUT)).width(Length::FillPortion(2)),
        container(text("State").size(11).color(TEXT_MUT)).width(Length::FillPortion(2)),
    ]
    .spacing(8)
    .padding([4, 0]);

    let rows = editor.rows();
    let active_profile_is_custom = editor.active_profile_is_custom();
    let mut table_rows: Vec<Element<'a, PrefMsg>> = Vec::with_capacity(rows.len());
    for row_model in rows {
        let has_conflict = conflicts.iter().any(|conflict| {
            conflict.context == row_model.context
                && conflict.trigger == row_model.trigger
                && row_model.command.as_ref().is_some_and(|command| {
                    command == &conflict.first_command || command == &conflict.second_command
                })
        });
        let state = if !row_model.trigger_valid {
            "Invalid"
        } else if has_conflict {
            "Conflict"
        } else if row_model.trigger.trim().is_empty() {
            "Unbound"
        } else if row_model.keyboard_editable && active_profile_is_custom {
            "Editable"
        } else if row_model.keyboard_editable {
            "Create custom"
        } else {
            "Gesture"
        };
        let state_color = if !row_model.trigger_valid || has_conflict {
            WARN_YELLOW
        } else if row_model.trigger.trim().is_empty() {
            TEXT_MUT
        } else {
            TEXT_PRI
        };

        // Keyboard-editable rows in a custom profile get an inline Edit
        // button that opens the chord recorder; everything else is
        // read-only (built-in profiles, pointer gestures).
        let trigger_cell: Element<'a, PrefMsg> = if active_profile_is_custom
            && row_model.keyboard_editable
        {
            if let Some(command) = row_model.command.clone() {
                let context = row_model.context;
                let label = row_model.label.clone();
                let trigger = row_model.trigger.clone();
                row![
                    shortcut_chip(&row_model.trigger, state_color),
                    button(container(text("Edit").size(10).color(TEXT_PRI)).padding([3, 8]))
                        .on_press(PrefMsg::KeymapRecorderOpen {
                            command,
                            label,
                            context,
                            trigger,
                        })
                        .style(secondary_button_style),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center)
                .into()
            } else {
                text(row_model.trigger).size(11).color(TEXT_MUT).into()
            }
        } else {
            shortcut_chip(&row_model.trigger, TEXT_PRI)
        };

        table_rows.push(
            row![
                container(
                    text(title_case(&row_model.category))
                        .size(11)
                        .color(TEXT_MUT)
                )
                .width(Length::FillPortion(2)),
                container(text(row_model.label).size(11).color(TEXT_PRI))
                    .width(Length::FillPortion(4)),
                container(
                    text(context_label(row_model.context))
                        .size(11)
                        .color(TEXT_MUT)
                )
                .width(Length::FillPortion(2)),
                container(trigger_cell).width(Length::FillPortion(2)),
                container(text(state).size(11).color(state_color)).width(Length::FillPortion(2)),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .padding([5, 0])
            .into(),
        );
    }

    let table: Element<'a, PrefMsg> = if table_rows.is_empty() {
        text("No shortcuts are defined in the active profile.")
            .size(11)
            .color(TEXT_MUT)
            .into()
    } else {
        Column::with_children(table_rows).spacing(2).into()
    };

    let mut content = column![
        header,
        profile_row,
        text(status_line).size(10).color(TEXT_MUT),
    ]
    .spacing(10)
    .padding(20);

    if let Some(recorder) = recorder {
        content = content.push(keymap_recorder_control(recorder));
    }

    content.push(h_sep()).push(table_header).push(table).into()
}

fn shortcut_chip<'a>(label: &str, color: Color) -> Element<'a, PrefMsg> {
    let label = if label.trim().is_empty() {
        "Unbound".to_string()
    } else {
        label.to_string()
    };
    container(text(label).size(11).color(color))
        .padding([3, 8])
        .width(Length::Shrink)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.10, 0.10, 0.12))),
            border: Border {
                width: 1.0,
                color: SEP,
                radius: 3.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn keymap_recorder_control<'a>(
    recorder: &'a crate::app::KeymapRecorderState,
) -> Element<'a, PrefMsg> {
    let recorded: Element<'a, PrefMsg> = if recorder.strokes.is_empty() {
        text("Press Record, then type a shortcut")
            .size(12)
            .color(TEXT_MUT)
            .into()
    } else {
        row(recorded_shortcut_chips(recorder))
            .spacing(6)
            .align_y(iced::Alignment::Center)
            .into()
    };

    let transient_modifiers = if recorder.recording
        && recorder.modifiers != crate::keymap::Modifiers::default()
        && recorder.strokes.len() < crate::app::KeymapRecorderState::MAX_STROKES
    {
        Some(shortcut_chip(
            &format!("{}...", modifiers_label(recorder.modifiers)),
            WARN_YELLOW,
        ))
    } else {
        None
    };
    let mut capture_row = row![
        text(if recorder.recording {
            "Recording"
        } else {
            "Recorded"
        })
        .size(11)
        .color(if recorder.recording {
            WARN_YELLOW
        } else {
            TEXT_MUT
        }),
        recorded,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);
    if let Some(transient_modifiers) = transient_modifiers {
        capture_row = capture_row.push(transient_modifiers);
    }

    let record_button = if recorder.recording {
        button(container(text("Stop").size(11).color(Color::WHITE)).padding([5, 12]))
            .on_press(PrefMsg::KeymapRecorderStop)
            .style(move |_: &Theme, status: button::Status| {
                let bg = match status {
                    button::Status::Hovered | button::Status::Pressed => BTN_DANGER_HOV,
                    _ => BTN_DANGER,
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: Color::WHITE,
                    border: Border {
                        radius: 3.0.into(),
                        ..Border::default()
                    },
                    ..button::Style::default()
                }
            })
    } else {
        button(container(text("Record").size(11).color(Color::WHITE)).padding([5, 12]))
            .on_press(PrefMsg::KeymapRecorderStart)
            .style(move |_: &Theme, status: button::Status| {
                let bg = match status {
                    button::Status::Hovered | button::Status::Pressed => BTN_IMPORT_HOV,
                    _ => BTN_IMPORT,
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: Color::WHITE,
                    border: Border {
                        radius: 3.0.into(),
                        ..Border::default()
                    },
                    ..button::Style::default()
                }
            })
    };

    container(
        column![
            row![
                column![
                    text("Edit Shortcut").size(15).color(TEXT_PRI),
                    text(recorder.command_label.clone()).size(11).color(TEXT_MUT),
                ]
                .spacing(3),
                Space::new().width(Length::Fill),
                button(container(text("Cancel").size(11).color(TEXT_PRI)).padding([5, 12]))
                    .on_press(PrefMsg::KeymapRecorderCancel)
                    .style(secondary_button_style),
            ]
            .align_y(iced::Alignment::Center),
            container(
                column![
                    row![
                        text(context_label(recorder.context)).size(11).color(TEXT_MUT),
                        text("Current").size(11).color(TEXT_MUT),
                        shortcut_chip(&recorder.original_trigger, TEXT_PRI),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                    capture_row,
                ]
                .spacing(10),
            )
            .padding(12)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(Color::from_rgb(0.10, 0.10, 0.12))),
                border: Border {
                    width: 1.0,
                    color: SEP,
                    radius: 4.0.into(),
                },
                ..container::Style::default()
            }),
            text("Press the exact keystroke to record it. ESC is recorded as a key; use Cancel to close.")
                .size(10)
                .color(TEXT_MUT),
            row![
                record_button,
                button(container(text("Clear").size(11).color(TEXT_PRI)).padding([5, 12]))
                    .on_press(PrefMsg::KeymapRecorderClear)
                    .style(secondary_button_style),
                Space::new().width(Length::Fill),
                button(container(text("OK").size(11).color(Color::WHITE)).padding([5, 12]))
                    .on_press(PrefMsg::KeymapRecorderApply)
                    .style(move |_: &Theme, status: button::Status| {
                        let bg = match status {
                            button::Status::Hovered | button::Status::Pressed => BTN_IMPORT_HOV,
                            _ => BTN_IMPORT,
                        };
                        button::Style {
                            background: Some(Background::Color(bg)),
                            text_color: Color::WHITE,
                            border: Border {
                                radius: 3.0.into(),
                                ..Border::default()
                            },
                            ..button::Style::default()
                        }
                    }),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(12),
    )
    .padding(16)
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(Color::from_rgb(0.085, 0.09, 0.105))),
        border: Border {
            width: 1.0,
            color: BTN_IMPORT,
            radius: 6.0.into(),
        },
        ..container::Style::default()
    })
    .into()
}

fn recorded_shortcut_chips<'a>(
    recorder: &'a crate::app::KeymapRecorderState,
) -> Vec<Element<'a, PrefMsg>> {
    recorder
        .strokes
        .iter()
        .map(|stroke| shortcut_chip(&stroke.to_string(), TEXT_PRI))
        .collect()
}

fn modifiers_label(modifiers: crate::keymap::Modifiers) -> String {
    let mut parts = Vec::new();
    if modifiers.ctrl {
        parts.push("Ctrl");
    }
    if modifiers.command && !modifiers.ctrl {
        parts.push("Cmd");
    }
    if modifiers.alt {
        parts.push("Alt");
    }
    if modifiers.shift {
        parts.push("Shift");
    }
    parts.join("+")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KeymapProfileOption {
    id: String,
    label: String,
}

impl From<&crate::keymap::KeymapEditorProfile> for KeymapProfileOption {
    fn from(profile: &crate::keymap::KeymapEditorProfile) -> Self {
        let kind = match profile.kind {
            crate::keymap::ShortcutProfileKind::BuiltIn => "built-in",
            crate::keymap::ShortcutProfileKind::Custom => "custom",
        };
        Self {
            id: profile.id.clone(),
            label: format!("{} ({kind})", profile.name),
        }
    }
}

impl std::fmt::Display for KeymapProfileOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

fn context_label(context: crate::keymap::ShortcutContext) -> &'static str {
    match context {
        crate::keymap::ShortcutContext::Global => "Global",
        crate::keymap::ShortcutContext::Schematic => "Schematic",
        crate::keymap::ShortcutContext::Footprint => "Footprint",
        crate::keymap::ShortcutContext::Pcb => "PCB",
        crate::keymap::ShortcutContext::Library => "Library",
        crate::keymap::ShortcutContext::Modal => "Modal",
        crate::keymap::ShortcutContext::TextInput => "Text Input",
        crate::keymap::ShortcutContext::CommandPalette => "Command Palette",
        crate::keymap::ShortcutContext::Placement => "Placement",
    }
}

fn title_case(value: &str) -> String {
    value
        .split(['_', '-', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn secondary_button_style(_: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => Color::from_rgb(0.22, 0.22, 0.26),
        _ => Color::from_rgb(0.18, 0.18, 0.21),
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: TEXT_PRI,
        border: Border {
            width: 1.0,
            color: SEP,
            radius: 3.0.into(),
        },
        ..button::Style::default()
    }
}

// ─── Component Classes editor ─────────────────────────────────

fn content_component_classes<'a>(
    classes: &'a [crate::fonts::ComponentClassEntry],
) -> Element<'a, PrefMsg> {
    let header = column![
        text("Default Component Classes").size(15).color(TEXT_PRI),
        text(
            "Seeds the class registry of newly-created libraries. \
              Per-library edits live inside each .snxlib's manifest \
              (forthcoming Library Properties pane); this list \
              controls only what new libraries inherit."
        )
        .size(11)
        .color(TEXT_MUT),
    ]
    .spacing(6);

    let column_header = row![
        container(text("Key").size(11).color(TEXT_MUT)).width(Length::FillPortion(2)),
        container(text("Label").size(11).color(TEXT_MUT)).width(Length::FillPortion(3)),
        container(Space::new()).width(80),
    ]
    .spacing(8)
    .padding([4, 0]);

    let mut rows: Vec<Element<'a, PrefMsg>> = Vec::with_capacity(classes.len());
    for (idx, entry) in classes.iter().enumerate() {
        let key_input = text_input("class_key", entry.key.as_str())
            .on_input(move |s| PrefMsg::ComponentClassEditKey { index: idx, key: s })
            .padding(5)
            .size(12)
            .width(Length::FillPortion(2));
        let label_input = text_input("Label", entry.label.as_str())
            .on_input(move |s| PrefMsg::ComponentClassEditLabel {
                index: idx,
                label: s,
            })
            .padding(5)
            .size(12)
            .width(Length::FillPortion(3));
        let remove_btn = button(
            container(text("Remove").size(11).color(Color::WHITE))
                .padding([4, 10])
                .center_x(Length::Fill),
        )
        .on_press(PrefMsg::ComponentClassRemove { index: idx })
        .style(move |_: &Theme, status: button::Status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => BTN_DANGER_HOV,
                _ => BTN_DANGER,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: Color::WHITE,
                border: Border {
                    radius: 3.0.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            }
        })
        .width(80);

        rows.push(
            row![key_input, label_input, remove_btn]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into(),
        );
    }

    let body: Element<'a, PrefMsg> = if rows.is_empty() {
        text("No classes defined. Click \"+ Add Class\" to add one or \"Reset to Defaults\" to restore the seed list.")
            .size(11)
            .color(TEXT_MUT)
            .into()
    } else {
        Column::with_children(rows).spacing(6).into()
    };

    let add_btn =
        button(container(text("+ Add Class").size(11).color(Color::WHITE)).padding([5, 12]))
            .on_press(PrefMsg::ComponentClassAdd)
            .style(move |_: &Theme, status: button::Status| {
                let bg = match status {
                    button::Status::Hovered | button::Status::Pressed => BTN_IMPORT_HOV,
                    _ => BTN_IMPORT,
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: Color::WHITE,
                    border: Border {
                        radius: 3.0.into(),
                        ..Border::default()
                    },
                    ..button::Style::default()
                }
            });

    let reset_btn =
        button(container(text("Reset to Defaults").size(11).color(TEXT_PRI)).padding([5, 12]))
            .on_press(PrefMsg::ComponentClassResetDefaults)
            .style(move |_: &Theme, status: button::Status| {
                let bg = match status {
                    button::Status::Hovered | button::Status::Pressed => {
                        Color::from_rgb(0.22, 0.22, 0.26)
                    }
                    _ => Color::from_rgb(0.18, 0.18, 0.21),
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: TEXT_PRI,
                    border: Border {
                        width: 1.0,
                        color: SEP,
                        radius: 3.0.into(),
                    },
                    ..button::Style::default()
                }
            });

    let toolbar = row![
        add_btn,
        Space::new().width(8),
        reset_btn,
        Space::new().width(Length::Fill),
    ]
    .spacing(0)
    .align_y(iced::Alignment::Center);

    column![header, column_header, body, Space::new().height(8), toolbar]
        .spacing(10)
        .padding(20)
        .into()
}
