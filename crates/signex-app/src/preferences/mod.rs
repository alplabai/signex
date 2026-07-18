//! Preferences dialog — Altium-style modal with left navigation + right content.
//!
//! Opened via Tools > Preferences (or keyboard shortcut).
//! Left side: tree of settings categories.
//! Right side: settings panel for the selected category.

use crate::render_config::{GridStyle, LabelStyle, MultisheetStyle, PinSelectionMode, PowerPortStyle};
use iced::widget::{
    Column, Space, button, column, container, row, scrollable, svg, text,
};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_types::theme::ThemeId;

use crate::app::view::dialogs::{
    MODAL_CLOSE_X_HIT_H, MODAL_CLOSE_X_HIT_W, MODAL_CLOSE_X_HOVER, MODAL_CLOSE_X_ICON,
    MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE,
};
use crate::styles::MODAL_CORNER_RADIUS;

mod appearance;
mod component_classes;
mod distributors;
mod erc;
mod keymap;
mod widgets;

use appearance::content_appearance;
use component_classes::content_component_classes;
use distributors::content_library_distributors;
use erc::content_erc;
use keymap::content_keyboard_shortcuts;

// Shared style / text / button helpers live in `widgets`; re-export them
// at their original module-local visibility so the shell here and every
// section submodule keep resolving them through `use super::*`.
pub(in crate::preferences) use widgets::{
    danger_button_style, h_sep, primary_button_style, secondary_button_style, section_title,
    success_button_style, text_muted, text_primary, text_warning,
};

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
    /// Update the symbol-editor pin-selection mode (persisted on change).
    DraftSymbolPinSelection(PinSelectionMode),
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
    /// Live search query for the shortcut table (case-insensitive filter
    /// on label / command id / trigger). Pure view state — the handler
    /// just stores it and never marks the draft dirty.
    KeymapSearchChanged(String),
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
    draft_symbol_pin_selection: PinSelectionMode,
    custom_name: Option<&'a str>,
    dirty: bool,
    erc_overrides: &'a std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
    distributor_settings: &'a crate::library::state::DistributorSettings,
    panel_tokens: &'a signex_types::theme::ThemeTokens,
    draft_component_classes: &'a [crate::fonts::ComponentClassEntry],
    keymap_editor: &'a crate::keymap::KeymapEditorModel,
    keymap_status: &'a str,
    keymap_search: &'a str,
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
        draft_symbol_pin_selection,
        custom_name,
        dirty,
        erc_overrides,
        distributor_settings,
        panel_tokens,
        draft_component_classes,
        keymap_editor,
        keymap_status,
        keymap_search,
        keymap_recorder,
        theme_id,
    );

    container(
        column![
            Space::new().height(Length::Fill),
            row![
                Space::new().width(Length::Fill),
                // In-app overlay: keep a centred, capped card (the detached
                // window path uses `view_body` directly and fills the window).
                container(dialog)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .max_width(DLG_W)
                    .max_height(DLG_H),
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
    draft_symbol_pin_selection: PinSelectionMode,
    custom_name: Option<&'a str>,
    dirty: bool,
    erc_overrides: &'a std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
    distributor_settings: &'a crate::library::state::DistributorSettings,
    panel_tokens: &'a signex_types::theme::ThemeTokens,
    draft_component_classes: &'a [crate::fonts::ComponentClassEntry],
    keymap_editor: &'a crate::keymap::KeymapEditorModel,
    keymap_status: &'a str,
    keymap_search: &'a str,
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
        draft_symbol_pin_selection,
        custom_name,
        dirty,
        erc_overrides,
        distributor_settings,
        panel_tokens,
        draft_component_classes,
        keymap_editor,
        keymap_status,
        keymap_search,
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
    draft_symbol_pin_selection: PinSelectionMode,
    custom_name: Option<&'a str>,
    dirty: bool,
    erc_overrides: &'a std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
    distributor_settings: &'a crate::library::state::DistributorSettings,
    panel_tokens: &'a signex_types::theme::ThemeTokens,
    draft_component_classes: &'a [crate::fonts::ComponentClassEntry],
    keymap_editor: &'a crate::keymap::KeymapEditorModel,
    keymap_status: &'a str,
    keymap_search: &'a str,
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
                .style(text_primary),
            Space::new().width(Length::Fill),
            close_btn(theme_id),
        ]
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .height(MODAL_HEADER_HEIGHT)
    .padding(MODAL_HEADER_PADDING)
    .style(move |theme: &Theme| container::Style {
        background: Some(Background::Color(theme.extended_palette().background.weak.color)),
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
            .style(move |theme: &Theme| container::Style {
                background: Some(Background::Color(theme.extended_palette().background.strong.color)),
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
            draft_symbol_pin_selection,
            custom_name,
            erc_overrides,
            distributor_settings,
            panel_tokens,
            draft_component_classes,
            keymap_editor,
            keymap_status,
            keymap_search,
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
            .style(move |theme: &Theme| container::Style {
                background: Some(Background::Color(theme.extended_palette().background.strong.color)),
                ..container::Style::default()
            })
            .into()
    };

    let mut col_items: Vec<Element<'a, PrefMsg>> = vec![header.into(), h_divider(), body.into()];
    if let Some(footer) = footer_opt {
        col_items.push(h_divider());
        col_items.push(footer);
    }

    // Fill the container we are given: as an in-app overlay `view()` wraps us
    // in a fixed DLG_W×DLG_H card; as the detached Preferences window content
    // this fills the OS window so the modal grows/shrinks with a window resize
    // instead of staying a fixed 960×660 island.
    container(Column::with_children(col_items).spacing(0))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |theme: &Theme| container::Style {
            background: Some(Background::Color(theme.extended_palette().background.base.color)),
            border: Border {
                width: 1.0,
                radius: MODAL_CORNER_RADIUS.into(),
                color: theme.extended_palette().background.strong.color,
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
                container(text(group.to_uppercase()).size(9).style(text_muted))
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
        .style(move |theme: &Theme| container::Style {
            background: Some(Background::Color(theme.extended_palette().background.weak.color)),
            ..container::Style::default()
        })
        .into()
}

fn nav_item<'a>(item: PrefNav, active: PrefNav) -> Element<'a, PrefMsg> {
    let is_active = item == active;

    button(
        container(
            row![
                text(item.label())
                    .size(12)
                    .style(move |theme: &Theme| text::Style {
                        color: Some(if is_active {
                            theme.extended_palette().primary.base.text
                        } else {
                            theme.extended_palette().background.base.text
                        }),
                    }),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([6, 12])
        .width(Length::Fill),
    )
    .padding(0)
    .width(Length::Fill)
    .on_press(PrefMsg::Nav(item))
    .style(move |theme: &Theme, status: button::Status| {
        let palette = theme.extended_palette();
        let bg = match (is_active, status) {
            (true, _) => Some(Background::Color(palette.primary.base.color)),
            (false, button::Status::Hovered) => {
                Some(Background::Color(palette.background.strong.color))
            }
            _ => None,
        };
        button::Style {
            background: bg,
            border: Border::default(),
            text_color: if is_active {
                palette.primary.base.text
            } else {
                palette.background.base.text
            },
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
    draft_symbol_pin_selection: PinSelectionMode,
    custom_name: Option<&'a str>,
    erc_overrides: &'a std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
    distributor_settings: &'a crate::library::state::DistributorSettings,
    panel_tokens: &'a signex_types::theme::ThemeTokens,
    draft_component_classes: &'a [crate::fonts::ComponentClassEntry],
    keymap_editor: &'a crate::keymap::KeymapEditorModel,
    keymap_status: &'a str,
    keymap_search: &'a str,
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
            draft_symbol_pin_selection,
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
            content_keyboard_shortcuts(keymap_editor, keymap_status, keymap_search, keymap_recorder)
        }
        PrefNav::ComponentClasses => content_component_classes(draft_component_classes),
    };

    container(scrollable(inner).width(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(0)
        .style(move |theme: &Theme| container::Style {
            background: Some(Background::Color(theme.extended_palette().background.base.color)),
            ..container::Style::default()
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
                // Resolve against the theme so the glyph stays legible on a
                // light header; the red hover footprint contrasts with the
                // near-white/near-black text colour on both palettes.
                .style(move |theme: &Theme, _| svg::Style {
                    color: Some(theme.extended_palette().background.base.text),
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
        text("● Unsaved changes").size(11).style(text_warning),
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
            .style(move |theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(Background::Color(palette.background.weak.color)),
                    border: Border {
                        width: 1.0,
                        color: palette.background.strong.color,
                        radius: iced::border::Radius::default()
                            .bottom_left(MODAL_CORNER_RADIUS)
                            .bottom_right(MODAL_CORNER_RADIUS),
                    },
                    ..container::Style::default()
                }
            })
            .into(),
    )
}

fn save_btn<'a>() -> Element<'a, PrefMsg> {
    button(text("Save").size(12))
        .padding([6, 20])
        .on_press(PrefMsg::Save)
        .style(success_button_style)
        .into()
}

fn discard_btn<'a>() -> Element<'a, PrefMsg> {
    button(text("Discard & Close").size(12))
        .padding([6, 16])
        .on_press(PrefMsg::DiscardAndClose)
        .style(danger_button_style)
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
