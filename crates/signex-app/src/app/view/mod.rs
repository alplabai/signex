use iced::widget::{canvas, column, container, row, text_input};
use iced::{Element, Length};

pub(crate) mod dialogs;
pub(crate) mod translate;

mod context_menu_items;
mod context_menus;
mod context_submenu;
mod dialog_annotate;
mod dialog_annotate_preview;
mod dialog_widgets;
mod pdf_preview;
mod print_preview;

use super::*;

// ── Submenu chevron — single source of truth ─────────────────────────
//
// Right-pointing angle quote (U+203A), NOT the BLACK RIGHT-POINTING
// TRIANGLE (U+25B6) which Windows renders via the colour emoji font.
// Same glyph the menu_bar dropdowns use; the matching size below keeps
// every submenu launcher visually aligned across the whole app
// (canvas right-click, project-tree right-click, File/Edit/View menu).
const SUBMENU_ARROW: &str = "›";
const SUBMENU_ARROW_SIZE: f32 = 18.0;

/// Chrome strip search bar width in pixels.
pub(crate) const CHROME_SEARCH_BAR_WIDTH: f32 = 440.0;
/// Fixed gap between the chrome search bar's right edge and the
/// chrome controls (min/max/close).
pub(crate) const CHROME_SEARCH_BAR_RIGHT_GAP: f32 = 12.0;
/// One chrome control button (min / max / close) width — see
/// `chrome_btn` in `view_main_window_chrome`.
pub(crate) const CHROME_CONTROL_BTN_W: f32 = 46.0;
/// Total controls strip width — three buttons.
pub(crate) const CHROME_CONTROLS_W: f32 = CHROME_CONTROL_BTN_W * 3.0;
/// Minimum left padding between the menu bar's right edge and the
/// chrome search bar's left edge.
pub(crate) const CHROME_SEARCH_LEFT_GAP: f32 = 16.0;
/// Minimum right padding between the chrome search bar's right edge
/// and the window-controls strip.
pub(crate) const CHROME_SEARCH_RIGHT_GAP: f32 = 16.0;

impl Signex {
    pub fn view(&self, window_id: iced::window::Id) -> Element<'_, Message> {
        // Secondary windows (detached modals, future undocked tabs) render
        // just their own content — no menu / dock / canvas. The main
        // window's view_main drops any overlay whose modal is currently
        // detached so we don't double-render.
        if let Some(kind) = self.ui_state.windows.get(&window_id) {
            return match kind {
                super::state::WindowKind::DetachedModal(modal) => self.view_detached_modal(*modal),
                // Undocked tab = full duplicate of the main app view.
                // Shared Signex state means edits sync automatically; the
                // only difference between main and undocked is the OS
                // window id they render into.
                super::state::WindowKind::UndockedTab { .. } => self.view_main_for(window_id),
                super::state::WindowKind::DetachedPanel(kind) => {
                    let panel = crate::panels::view_panel(*kind, &self.document_state.panel_ctx)
                        .map(crate::dock::DockMessage::Panel)
                        .map(Message::Dock);
                    iced::widget::container(iced::widget::scrollable(panel))
                        .padding(8)
                        .into()
                }
                // Detached Component Preview window — render the same
                // editor surface as the inline tab. The editor state
                // is keyed by `EditorAddress(library_path, table,
                // row_id)` so the inline + detached cases share a
                // single state owner.
                super::state::WindowKind::ComponentEditor {
                    library_path,
                    table,
                    row_id,
                } => {
                    let tokens = &self.document_state.panel_ctx.tokens;
                    let address = crate::library::state::EditorAddress::new(
                        library_path.clone(),
                        table.clone(),
                        *row_id,
                    );
                    if let Some(editor) = self.library.editors.get(&address) {
                        crate::library::editor::view(editor, &self.library, tokens, address)
                            .map(Message::Library)
                    } else {
                        // Window mapping exists but the editor state
                        // has been dropped (rare race during teardown
                        // — the tab close path can run ahead of the
                        // OS window close). Render an empty container
                        // so the daemon doesn't panic.
                        iced::widget::container(iced::widget::Space::new()).into()
                    }
                }
            };
        }
        self.view_main_for(window_id)
    }

    /// Cursor-following translucent preview of a tab being dragged.
    /// Shape matches the real tab bar entry — rounded container with
    /// the title text, the ↗ undock indicator, and the × close icon —
    /// so it reads as "the tab itself is moving". The ghost is
    /// non-interactive; it just shows what the user is carrying.
    fn view_tab_drag_ghost(&self, title: &str) -> Element<'_, Message> {
        use iced::widget::{container, row, text};
        use signex_widgets::tab_pill::{AccentPosition, TabPill, TabPillStyle};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let active_bg = crate::styles::ti(tokens.hover);
        let accent = crate::styles::ti(tokens.accent);
        // Match the live tab look: same TabPill widget, accent
        // stripe at the bottom, top-rounded corners. The previous
        // ghost showed an old style with inline ↗ undock + × close
        // glyphs that were removed when the tab right-click menu
        // landed.
        let pill_style = TabPillStyle {
            fill: iced::Color {
                a: 0.88,
                ..active_bg
            },
            border: crate::styles::ti(tokens.border),
            accent,
            is_active: true,
            is_last: true,
            accent_position: AccentPosition::Bottom,
        };
        let inner =
            container(row![text(title.to_string()).size(11).color(text_c)]).padding([4, 10]);
        let pill = TabPill::new(inner, pill_style);
        // Anchor near the cursor (right + below) so the pointer
        // remains visible while the ghost trails it.
        let (cx, cy) = self.interaction_state.last_mouse_pos;
        super::view::translate::Translate::new(pill, (cx + 10.0, cy + 6.0)).into()
    }

    /// Altium-style Move Selection dialog. Two numeric inputs plus
    /// OK / Cancel. No header drag region on the body itself — the
    /// modal opens borderless so the OS-window-drag handler owns that.
    fn view_move_selection_body(&self) -> Element<'_, Message> {
        use iced::widget::{Space, button, column, container, row, text, text_input};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let ms = &self.ui_state.move_selection;
        let selection_count = self.interaction_state.active_canvas().selected.len();

        let header = iced::widget::mouse_area(
            container(
                row![
                    text("Move Selection").size(14).color(text_c),
                    Space::new().width(iced::Length::Fill),
                    self.view_close_x(Message::MoveSelection(MoveSelectionMsg::Close)),
                ]
                .align_y(iced::Alignment::Center),
            )
            .padding([10, 14])
            .style(crate::styles::modal_header_strip(tokens)),
        )
        .on_press(Message::Window(WindowMsg::StartDetachedWindowDrag(
            super::state::ModalId::MoveSelection,
        )))
        .interaction(iced::mouse::Interaction::Grab);

        let field = |label: &'static str, value: &str, msg: fn(String) -> Message| {
            column![
                text(label).size(10).color(text_muted),
                text_input("0.00", value)
                    .on_input(msg)
                    .padding([4, 8])
                    .size(12),
            ]
            .spacing(4)
        };

        let body = container(
            column![
                text(format!("{} item(s) selected", selection_count))
                    .size(11)
                    .color(text_muted),
                Space::new().height(12),
                row![
                    field("ΔX (mm)", &ms.dx, |s| {
                        Message::MoveSelection(MoveSelectionMsg::DxChanged(s))
                    }),
                    Space::new().width(14),
                    field("ΔY (mm)", &ms.dy, |s| {
                        Message::MoveSelection(MoveSelectionMsg::DyChanged(s))
                    }),
                ]
                .align_y(iced::Alignment::Start),
            ]
            .spacing(0),
        )
        .padding([14, 14]);

        let ok_enabled = selection_count > 0;
        let ok_bg = if ok_enabled {
            iced::Color::from_rgb(0.00, 0.47, 0.84)
        } else {
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04)
        };
        let ok_fg = if ok_enabled {
            iced::Color::WHITE
        } else {
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.4)
        };
        let mut ok_btn = button(container(text("Apply").size(11).color(ok_fg)).padding([4, 14]))
            .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(ok_bg)),
                border: iced::Border {
                    width: 0.0,
                    radius: 3.0.into(),
                    ..iced::Border::default()
                },
                text_color: ok_fg,
                ..iced::widget::button::Style::default()
            });
        if ok_enabled {
            ok_btn = ok_btn.on_press(Message::MoveSelection(MoveSelectionMsg::Apply));
        }

        let footer = container(
            row![
                Space::new().width(iced::Length::Fill),
                button(container(text("Cancel").size(11).color(text_c)).padding([4, 14]))
                    .on_press(Message::MoveSelection(MoveSelectionMsg::Close))
                    .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                        background: Some(iced::Background::Color(iced::Color::from_rgba(
                            1.0, 1.0, 1.0, 0.04
                        ),)),
                        border: iced::Border {
                            width: 1.0,
                            radius: 3.0.into(),
                            color: border_c,
                        },
                        text_color: text_c,
                        ..iced::widget::button::Style::default()
                    }),
                Space::new().width(8),
                ok_btn,
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([10, 14]);

        container(
            column![header, body, footer]
                .width(iced::Length::Fixed(420.0))
                .height(iced::Length::Fixed(240.0)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .into()
    }

    /// Compact X close button shared by the detached-modal bodies.
    fn view_close_x(&self, message: Message) -> Element<'_, Message> {
        use iced::widget::{button, container, text};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text_secondary);
        let border = crate::styles::ti(tokens.border);
        button(container(text("\u{00D7}".to_string()).size(14).color(text_c)).padding([0, 6]))
            .on_press(message)
            .style(
                move |_: &iced::Theme, status: iced::widget::button::Status| {
                    let bg = match status {
                        iced::widget::button::Status::Hovered => Some(iced::Background::Color(
                            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.1),
                        )),
                        _ => Some(iced::Background::Color(iced::Color::from_rgba(
                            1.0, 1.0, 1.0, 0.03,
                        ))),
                    };
                    iced::widget::button::Style {
                        background: bg,
                        border: iced::Border {
                            width: 1.0,
                            radius: 3.0.into(),
                            color: border,
                        },
                        text_color: text_c,
                        ..iced::widget::button::Style::default()
                    }
                },
            )
            .into()
    }

    /// Altium F5 Net Color palette — list of net labels with a per-net
    /// color picker. Ships with a 10-swatch palette; a full ColorPicker
    /// widget can replace it later without changing the message contract.
    fn view_net_color_palette_body(&self) -> Element<'_, Message> {
        use iced::widget::{Space, button, column, container, row, scrollable, text};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let header = iced::widget::mouse_area(
            container(
                row![
                    text("Net Colors").size(14).color(text_c),
                    Space::new().width(iced::Length::Fill),
                    self.view_close_x(Message::NetColor(NetColorMsg::Close)),
                ]
                .align_y(iced::Alignment::Center),
            )
            .padding([10, 14])
            .style(crate::styles::modal_header_strip(tokens)),
        )
        .on_press(Message::Window(WindowMsg::StartDetachedWindowDrag(
            super::state::ModalId::NetColorPalette,
        )))
        .interaction(iced::mouse::Interaction::Grab);

        // Gather unique net labels from the active snapshot.
        let mut nets: Vec<String> = self
            .interaction_state
            .canvas
            .active_snapshot()
            .map(|s| {
                s.labels
                    .iter()
                    .filter(|l| {
                        matches!(
                            l.label_type,
                            signex_types::schematic::LabelType::Net
                                | signex_types::schematic::LabelType::Global
                                | signex_types::schematic::LabelType::Hierarchical
                        )
                    })
                    .map(|l| l.text.clone())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect()
            })
            .unwrap_or_default();
        nets.sort();

        const PALETTE: &[(u8, u8, u8)] = &[
            (0xE0, 0x54, 0x54),
            (0xE0, 0xB0, 0x4A),
            (0x78, 0xC2, 0x6A),
            (0x42, 0xB8, 0xE0),
            (0x6F, 0x77, 0xE0),
            (0xB0, 0x6F, 0xE0),
            (0xE0, 0x6F, 0xB0),
            (0xC2, 0xA0, 0x78),
            (0x78, 0xC2, 0xA0),
            (0xA0, 0xA0, 0xA0),
        ];

        let mut rows_col = column![].spacing(4);
        if nets.is_empty() {
            rows_col = rows_col.push(
                text("No net labels on the active sheet.")
                    .size(11)
                    .color(text_muted),
            );
        } else {
            for net in nets {
                let current = self.ui_state.net_colors.get(&net).copied();
                let mut swatches = row![].spacing(4).align_y(iced::Alignment::Center);
                for (r, g, b) in PALETTE {
                    let is_current = current.is_some_and(|c| c.r == *r && c.g == *g && c.b == *b);
                    let swatch_color = iced::Color::from_rgb8(*r, *g, *b);
                    let border_w = if is_current { 2.0_f32 } else { 1.0_f32 };
                    let net_copy = net.clone();
                    let r_c = *r;
                    let g_c = *g;
                    let b_c = *b;
                    swatches =
                        swatches.push(
                            button(container(Space::new().width(14).height(14)).style(
                                move |_: &iced::Theme| container::Style {
                                    background: Some(iced::Background::Color(swatch_color)),
                                    border: iced::Border {
                                        width: border_w,
                                        radius: 2.0.into(),
                                        color: text_c,
                                    },
                                    ..container::Style::default()
                                },
                            ))
                            .on_press(Message::NetColor(NetColorMsg::Set {
                                net: net_copy.clone(),
                                color: Some(signex_types::theme::Color {
                                    r: r_c,
                                    g: g_c,
                                    b: b_c,
                                    a: 255,
                                }),
                            }))
                            .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                                background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
                                border: iced::Border::default(),
                                ..iced::widget::button::Style::default()
                            }),
                        );
                }
                // Clear-override button
                let net_clear = net.clone();
                swatches = swatches.push(
                    button(container(text("×").size(10).color(text_c)).padding([0, 6]))
                        .on_press(Message::NetColor(NetColorMsg::Set {
                            net: net_clear,
                            color: None,
                        }))
                        .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                            background: Some(iced::Background::Color(iced::Color::from_rgba(
                                1.0, 1.0, 1.0, 0.04,
                            ))),
                            border: iced::Border {
                                width: 1.0,
                                radius: 2.0.into(),
                                color: border_c,
                            },
                            text_color: text_c,
                            ..iced::widget::button::Style::default()
                        }),
                );

                rows_col = rows_col.push(
                    row![
                        text(net)
                            .size(11)
                            .color(text_c)
                            .width(iced::Length::FillPortion(2)),
                        swatches,
                    ]
                    .align_y(iced::Alignment::Center)
                    .padding([2, 8]),
                );
            }
        }

        container(
            column![
                header,
                container(scrollable(rows_col).height(iced::Length::Fill))
                    .padding([14, 14])
                    .height(iced::Length::Fill),
            ]
            .width(iced::Length::Fixed(520.0))
            .height(iced::Length::Fixed(480.0)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .into()
    }

    /// Altium-style Parameter Manager — a scrolling table listing every
    /// placed symbol with columns for reference / value / footprint and
    /// a "Parameter" column that reveals the union of custom fields
    /// across the design. Each cell is a text_input so the user can edit
    /// values inline. Changes route through Command::SetSymbolField so
    /// undo/redo / dirty-flagging behaves.
    fn view_parameter_manager_body(&self) -> Element<'_, Message> {
        use iced::widget::{Space, column, container, row, scrollable, text, text_input};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let header = iced::widget::mouse_area(
            container(
                row![
                    text("Parameters").size(14).color(text_c),
                    Space::new().width(iced::Length::Fill),
                    self.view_close_x(Message::ParameterManager(ParameterManagerMsg::Close)),
                ]
                .align_y(iced::Alignment::Center),
            )
            .padding([10, 14])
            .style(crate::styles::modal_header_strip(tokens)),
        )
        .on_press(Message::Window(WindowMsg::StartDetachedWindowDrag(
            super::state::ModalId::ParameterManager,
        )))
        .interaction(iced::mouse::Interaction::Grab);

        // Collect all parameter keys across symbols (besides the built-
        // in reference / value / footprint). Keeps the table compact —
        // only columns that someone actually uses show up.
        let Some(engine) = self.document_state.active_engine() else {
            return container(
                column![
                    header,
                    container(text("No active schematic.").size(11).color(text_muted))
                        .padding([14, 14]),
                ]
                .width(iced::Length::Fixed(900.0))
                .height(iced::Length::Fixed(560.0)),
            )
            .style(crate::styles::modal_card(tokens))
            .clip(true)
            .into();
        };
        let doc = engine.document();
        let mut keys: Vec<String> = doc
            .symbols
            .iter()
            .filter(|s| !s.is_power)
            .flat_map(|s| s.fields.keys().cloned())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        keys.sort();

        let header_row = {
            let mut r = row![
                text("Reference")
                    .size(10)
                    .color(text_muted)
                    .width(iced::Length::Fixed(100.0)),
                text("Value")
                    .size(10)
                    .color(text_muted)
                    .width(iced::Length::Fixed(160.0)),
                text("Footprint")
                    .size(10)
                    .color(text_muted)
                    .width(iced::Length::Fixed(200.0)),
            ];
            for k in &keys {
                r = r.push(
                    text(k.clone())
                        .size(10)
                        .color(text_muted)
                        .width(iced::Length::Fixed(140.0)),
                );
            }
            r.padding([4, 8])
        };

        let mut rows_col = column![].spacing(2);
        rows_col = rows_col.push(header_row);
        for sym in &doc.symbols {
            if sym.is_power {
                continue;
            }
            let mut r = row![
                text(sym.reference.clone())
                    .size(11)
                    .color(text_c)
                    .width(iced::Length::Fixed(100.0)),
                text(sym.value.clone())
                    .size(11)
                    .color(text_c)
                    .width(iced::Length::Fixed(160.0)),
                text(sym.footprint.clone())
                    .size(11)
                    .color(text_muted)
                    .width(iced::Length::Fixed(200.0)),
            ];
            for k in &keys {
                let v = sym.fields.get(k).cloned().unwrap_or_default();
                let sym_uuid = sym.uuid;
                let k_str = k.clone();
                r = r.push(
                    text_input("", &v)
                        .on_input(move |new_val| {
                            Message::ParameterManager(ParameterManagerMsg::Edit {
                                symbol_uuid: sym_uuid,
                                key: k_str.clone(),
                                value: new_val,
                            })
                        })
                        .padding([2, 6])
                        .size(11)
                        .width(iced::Length::Fixed(140.0)),
                );
            }
            rows_col = rows_col.push(r.padding([2, 8]));
        }

        container(
            column![
                header,
                container(
                    scrollable(rows_col)
                        .direction(scrollable::Direction::Both {
                            vertical: scrollable::Scrollbar::default(),
                            horizontal: scrollable::Scrollbar::default(),
                        })
                        .height(iced::Length::Fill),
                )
                .padding([14, 14])
                .height(iced::Length::Fill)
                .style(move |_: &iced::Theme| container::Style {
                    border: iced::Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: border_c,
                    },
                    ..container::Style::default()
                }),
            ]
            .width(iced::Length::Fixed(900.0))
            .height(iced::Length::Fixed(560.0)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .into()
    }

    /// Custom net-colour picker modal. Grid of quick-pick swatches on
    /// the left, precise R / G / B / hex on the right, live preview
    /// and OK / Cancel at the bottom. Ships with a 24-color palette
    /// matching the common Altium net-colour presets plus a handful of
    /// EDA-specific diagnostic colours.
    fn view_net_color_custom_picker(&self) -> Element<'_, Message> {
        use super::contracts::Channel;
        use iced::widget::{Space, button, column, container, row, text, text_input};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let draft = self.ui_state.net_color_custom.draft;

        // Expanded 48-swatch palette arranged as 6 cols × 8 rows so
        // the Quick Pick grid fills the modal's left column. First
        // three rows are "standard" hues, next three rows are shade
        // variants, and the last two rows hold greys / light pastels /
        // schematic-specific high-contrast hues.
        const PALETTE: &[(u8, u8, u8)] = &[
            // Row 1 — primaries (bright)
            (0xEF, 0x44, 0x44), // Red
            (0xF9, 0x73, 0x16), // Orange
            (0xEA, 0xB3, 0x08), // Yellow
            (0x22, 0xC5, 0x5E), // Green
            (0x06, 0xB6, 0xD4), // Cyan
            (0x3B, 0x82, 0xF6), // Blue
            // Row 2 — pinks + magentas + purples
            (0xF4, 0x72, 0xB6), // Pink 400
            (0xE1, 0x14, 0x8C), // Hot Pink
            (0xD9, 0x46, 0xEF), // Fuchsia
            (0xA8, 0x55, 0xF7), // Purple
            (0x8B, 0x5C, 0xF6), // Violet
            (0x6D, 0x28, 0xD9), // Indigo
            // Row 3 — greens + teals + lime
            (0x84, 0xCC, 0x16), // Lime
            (0x10, 0xB9, 0x81), // Emerald
            (0x14, 0xB8, 0xA6), // Teal
            (0x0E, 0xA5, 0xE9), // Sky
            (0x60, 0xA5, 0xFA), // Light Blue
            (0x2D, 0xD4, 0xBF), // Turquoise
            // Row 4 — dark variants
            (0x9F, 0x12, 0x39), // Wine
            (0xB4, 0x53, 0x09), // Rust
            (0xA1, 0x6A, 0x3C), // Brown
            (0x16, 0xA3, 0x4A), // Dark Green
            (0x15, 0x5E, 0x75), // Deep Cyan
            (0x1E, 0x40, 0xAF), // Deep Blue
            // Row 5 — extra dark / night hues
            (0x7F, 0x1D, 0x1D), // Deep Red
            (0x78, 0x35, 0x0F), // Auburn
            (0x5B, 0x21, 0xB6), // Royal Purple
            (0x3B, 0x0A, 0x45), // Eggplant
            (0x1E, 0x3A, 0x8A), // Navy
            (0x0F, 0x17, 0x2A), // Midnight
            // Row 6 — pastels
            (0xFE, 0xCA, 0xCA), // Pastel Red
            (0xFE, 0xD7, 0xAA), // Pastel Orange
            (0xFE, 0xF0, 0x8A), // Pastel Yellow
            (0xBB, 0xF7, 0xD0), // Pastel Green
            (0xA5, 0xF3, 0xFC), // Pastel Cyan
            (0xBF, 0xDB, 0xFE), // Pastel Blue
            // Row 7 — muted + desaturated
            (0x64, 0x74, 0x8B), // Slate
            (0x78, 0x71, 0x6C), // Stone
            (0x4B, 0x55, 0x63), // Dark Slate
            (0x9C, 0xA3, 0xAF), // Gray
            (0xD1, 0xD5, 0xDB), // Light Gray
            (0xFF, 0xFF, 0xFF), // White
            // Row 8 — schematic diagnostic colors
            (0xFF, 0x00, 0xFF), // Bright Magenta
            (0x00, 0xFF, 0xFF), // Bright Cyan
            (0xFF, 0xFF, 0x00), // Bright Yellow
            (0x00, 0xFF, 0x00), // Bright Green
            (0xFF, 0xA5, 0x00), // Bright Orange
            (0x1F, 0x23, 0x2A), // Ink
        ];

        let swatch_btn =
            |r: u8, g: u8, b: u8| -> Element<'_, Message> {
                let col = iced::Color::from_rgb8(r, g, b);
                let is_current = (draft.r - col.r).abs() < 0.01
                    && (draft.g - col.g).abs() < 0.01
                    && (draft.b - col.b).abs() < 0.01;
                let sw = iced::Color::from_rgb8(r, g, b);
                let border_w = if is_current { 2.0 } else { 1.0 };
                let border_col = if is_current {
                    iced::Color::WHITE
                } else {
                    iced::Color::from_rgba(0.2, 0.2, 0.22, 0.9)
                };
                button(container(Space::new().width(24).height(20)).style(
                    move |_: &iced::Theme| container::Style {
                        background: Some(iced::Background::Color(sw)),
                        border: iced::Border {
                            width: border_w,
                            radius: 3.0.into(),
                            color: border_col,
                        },
                        ..container::Style::default()
                    },
                ))
                .padding(0)
                .on_press(Message::NetColor(NetColorMsg::CustomDraft(col)))
                .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
                    border: iced::Border::default(),
                    ..iced::widget::button::Style::default()
                })
                .into()
            };

        // Build the 6 × 4 palette grid row by row.
        let mut palette_col = column![].spacing(6);
        for chunk in PALETTE.chunks(6) {
            let mut r_el = row![].spacing(6);
            for (r, g, b) in chunk {
                r_el = r_el.push(swatch_btn(*r, *g, *b));
            }
            palette_col = palette_col.push(r_el);
        }

        // RGB inputs — parse as u8, clamp on submit. Uses the
        // `draft` colour as the current value so swatch clicks and
        // text edits stay in sync.
        let channel_row =
            |label: &'static str, value: f32, chan: Channel| -> Element<'_, Message> {
                let v255 = (value * 255.0).round() as i32;
                row![
                    text(label)
                        .size(11)
                        .color(text_muted)
                        .width(iced::Length::Fixed(16.0)),
                    text_input("0", &v255.to_string())
                        .size(11)
                        .padding([3, 8])
                        .width(iced::Length::Fixed(70.0))
                        .on_input(move |s| Message::NetColor(NetColorMsg::CustomChannel(chan, s))),
                ]
                .align_y(iced::Alignment::Center)
                .spacing(6)
                .into()
            };

        let preview_hex = format!(
            "#{:02X}{:02X}{:02X}",
            (draft.r * 255.0).round() as u8,
            (draft.g * 255.0).round() as u8,
            (draft.b * 255.0).round() as u8,
        );
        let preview_box = container(Space::new().width(iced::Length::Fill).height(32)).style(
            move |_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(draft)),
                border: iced::Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border_c,
                },
                ..container::Style::default()
            },
        );

        let rgb_col = column![
            text("Custom RGB").size(11).color(text_c),
            Space::new().height(6),
            channel_row("R", draft.r, Channel::R),
            channel_row("G", draft.g, Channel::G),
            channel_row("B", draft.b, Channel::B),
            Space::new().height(10),
            preview_box,
            Space::new().height(4),
            text(preview_hex).size(10).color(text_muted),
        ]
        .spacing(4)
        .width(iced::Length::Fixed(150.0));

        let body = row![
            column![
                text("Quick Pick").size(11).color(text_c),
                Space::new().height(6),
                palette_col,
            ]
            .spacing(0)
            .width(iced::Length::Fill),
            Space::new().width(16),
            rgb_col,
        ];

        let footer = row![
            Space::new().width(iced::Length::Fill),
            button(container(text("Cancel").size(11).color(text_c)).padding([4, 14]),)
                .on_press(Message::NetColor(NetColorMsg::CustomShow(false)))
                .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.04,
                    ))),
                    border: iced::Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: border_c,
                    },
                    text_color: text_c,
                    ..iced::widget::button::Style::default()
                }),
            Space::new().width(8),
            button(
                container(text("Use Color").size(11).color(iced::Color::WHITE)).padding([4, 14]),
            )
            .on_press(Message::NetColor(NetColorMsg::CustomSubmit(draft)))
            .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgb(
                    0.00, 0.47, 0.84,
                ))),
                border: iced::Border {
                    width: 0.0,
                    radius: 3.0.into(),
                    ..iced::Border::default()
                },
                text_color: iced::Color::WHITE,
                ..iced::widget::button::Style::default()
            }),
        ]
        .align_y(iced::Alignment::Center);

        let card = container(
            column![
                container(
                    row![
                        text("Pick Net Color").size(13).color(text_c),
                        Space::new().width(iced::Length::Fill),
                        self.view_close_x(Message::NetColor(NetColorMsg::CustomShow(false))),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([10, 14])
                .style(crate::styles::toolbar_strip(
                    &self.document_state.panel_ctx.tokens
                )),
                container(body).padding([14, 14]),
                container(footer).padding([10, 14]),
            ]
            .width(iced::Length::Fixed(430.0)),
        )
        .style(crate::styles::context_menu(
            &self.document_state.panel_ctx.tokens,
        ));

        // Anchor below the Active Bar Net Color button (rightmost icon).
        let (ww, _wh) = self.ui_state.window_size;
        let card_w = 430.0;
        let x = ((ww - card_w) * 0.5).max(0.0);
        let y = crate::menu_bar::MENU_BAR_HEIGHT
            + if self.document_state.tabs.is_empty() {
                0.0
            } else {
                28.0
            }
            + 80.0;
        // Wrap in a mouse_area with on_press(Noop) so clicks inside the
        // card are captured and DON'T fall through to the dismiss
        // layer sitting beneath. Without this, clicking on the card's
        // background / between swatches closes the picker.
        let card_capturing = iced::widget::mouse_area(card).on_press(Message::Noop);
        super::view::translate::Translate::new(card_capturing, (x, y)).into()
    }

    /// Custom chrome for the borderless main window. Replaces the OS
    /// title bar with a 36 px strip:
    ///
    /// `[wordmark + menus] [drag] [search bar] [drag] [min│max│×]`
    ///
    /// The drag zones are the only mouse-area clickable regions — menu
    /// buttons, search, and window controls keep their own click
    /// handlers. Double-click on a drag zone toggles maximize.
    fn view_main_window_chrome<'a>(
        &self,
        menu_row: Element<'a, Message>,
        tokens: &signex_types::theme::ThemeTokens,
    ) -> Element<'a, Message> {
        use iced::widget::{Space, button, container, mouse_area, row, svg, text};
        use iced::{Alignment, Background, Border, Color, Length};

        // Window-control SVG icons resolved through `crate::icons` so the
        // accent sentinel in each SVG tints to the active theme at
        // fetch time.
        let theme_id = self.ui_state.theme_id;
        let h_min = crate::icons::icon_chrome_window_min(theme_id);
        let h_max = crate::icons::icon_chrome_window_max(theme_id);
        let h_close = crate::icons::icon_chrome_window_close(theme_id);
        let h_search = crate::icons::icon_chrome_search(theme_id);

        let text_c = crate::styles::ti(tokens.text);
        let muted_c = crate::styles::ti(tokens.text_secondary);
        let hover_c = crate::styles::ti(tokens.hover);
        let search_bg = crate::styles::ti(tokens.panel_bg);
        let search_border = crate::styles::ti(tokens.border);
        // Windows-native destructive red for the close hover — overrides
        // the theme hover so close reads as destructive at a glance.
        let close_hover = Color::from_rgba(0.78, 0.22, 0.22, 1.0);
        let btn_h = crate::menu_bar::MENU_BAR_HEIGHT;

        let chrome_btn = |handle: svg::Handle,
                          msg: Message,
                          hover_bg: Color,
                          hover_icon: Color|
         -> Element<'static, Message> {
            // 14×14 brings the X / – / □ glyphs up to native-Windows
            // chrome scale; the prior 10×10 left them visibly smaller
            // than the surrounding menu-bar text.
            let icon = svg(handle)
                .width(14)
                .height(14)
                .style(move |_: &iced::Theme, _| svg::Style {
                    color: Some(text_c),
                });
            button(
                container(icon)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center)
                    .align_y(iced::alignment::Vertical::Center),
            )
            .width(46)
            .height(btn_h)
            .padding(0)
            .on_press(msg)
            .style(move |_: &iced::Theme, status: button::Status| {
                let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
                button::Style {
                    background: if hovered {
                        Some(Background::Color(hover_bg))
                    } else {
                        None
                    },
                    text_color: if hovered { hover_icon } else { text_c },
                    border: Border::default(),
                    ..Default::default()
                }
            })
            .into()
        };

        let controls = row![
            chrome_btn(
                h_min.clone(),
                Message::Window(WindowMsg::MinimizeMainWindow),
                hover_c,
                text_c
            ),
            chrome_btn(
                h_max.clone(),
                Message::Window(WindowMsg::ToggleMaximizeMainWindow),
                hover_c,
                text_c,
            ),
            chrome_btn(
                h_close.clone(),
                Message::Window(WindowMsg::CloseMainWindow),
                close_hover,
                Color::WHITE,
            ),
        ];

        // Left-pad the menu row so the wordmark doesn't sit flush against
        // the window edge; controls stay flush-right so their hover boxes
        // touch the corner like in Windows' native chrome.
        let menu_padded = container(menu_row).padding(iced::Padding {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 8.0,
        });

        // Chrome-strip command palette input — VS Code-style fuzzy
        // search over commands, placed symbols, and project files. The
        // text_input is always rendered; the dropdown overlay is gated
        // on `command_palette.open` and rendered by `collect_overlays`.
        let search_icon =
            svg(h_search.clone())
                .width(12)
                .height(12)
                .style(move |_: &iced::Theme, _| svg::Style {
                    color: Some(muted_c),
                });
        let palette_input = text_input(
            "Search files, symbols, commands…",
            &self.ui_state.command_palette.query,
        )
        .id(crate::app::command_palette::COMMAND_PALETTE_INPUT_ID.clone())
        .on_input(|q| Message::CommandPalette(CommandPaletteMsg::QueryChanged(q)))
        .on_submit(Message::CommandPalette(CommandPaletteMsg::ExecuteSelected))
        .padding(iced::Padding::ZERO)
        .size(11)
        .width(Length::Fill)
        .style(
            move |_: &iced::Theme, _status: text_input::Status| text_input::Style {
                // Outer container owns the chrome border + bg, so the
                // input itself is transparent. Without this the input's
                // default frame paints on top of the container's
                // rounded rect and the corners look doubled.
                background: Background::Color(Color::TRANSPARENT),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 0.0.into(),
                },
                icon: text_c,
                placeholder: muted_c,
                value: text_c,
                selection: Color { a: 0.4, ..text_c },
            },
        );
        let search_bar: Element<'_, Message> = container(
            row![search_icon, palette_input]
                .spacing(8)
                .align_y(Alignment::Center),
        )
        .padding(iced::Padding {
            top: 0.0,
            right: 10.0,
            bottom: 0.0,
            left: 10.0,
        })
        .width(crate::app::view::CHROME_SEARCH_BAR_WIDTH)
        .height(28)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(search_bg)),
            border: Border {
                color: search_border,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..container::Style::default()
        })
        .into();

        // Drag zones on either side of the search bar. Double-click
        // toggles maximize (Windows title-bar convention).
        let drag_zone = || -> Element<'static, Message> {
            mouse_area(
                container(Space::new())
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .on_press(Message::Window(WindowMsg::StartMainWindowDrag))
            .on_double_click(Message::Window(WindowMsg::ToggleMaximizeMainWindow))
            .into()
        };

        // Original chrome layout — search bar centered between menu
        // and controls (slightly right of true window center because
        // the menu section is wider than the window-controls strip,
        // but visually fine and the layout draggability + redraw
        // characteristics are correct). Two Fill drag zones flank
        // the search bar so the entire strip outside the input and
        // controls is draggable.
        let inner = row![menu_padded, drag_zone(), search_bar, drag_zone(), controls]
            .width(Length::Fill)
            .align_y(Alignment::Center);

        container(inner)
            .width(Length::Fill)
            .height(btn_h)
            .style(crate::styles::toolbar_strip(tokens))
            .into()
    }

    fn view_preferences_body(&self) -> Element<'_, Message> {
        use crate::app::view::dialogs::{MODAL_CLOSE_X_HIT_W, MODAL_HEADER_HEIGHT};
        use iced::widget::{Space, Stack, column as col_widget, mouse_area, row};

        let ui = &self.ui_state;
        let dialog: Element<'_, Message> = crate::preferences::view_body(
            ui.preferences_nav,
            ui.preferences_draft_theme,
            ui.theme_id,
            &ui.preferences_draft_font,
            ui.preferences_draft_power_port_style,
            ui.preferences_draft_label_style,
            ui.preferences_draft_multisheet_style,
            ui.preferences_draft_grid_style,
            ui.preferences_draft_symbol_grid_size_mm,
            ui.preferences_draft_symbol_grid_style,
            ui.custom_theme.as_ref().map(|c| c.name.as_str()),
            ui.preferences_dirty,
            &ui.erc_severity_override,
            &self.library.settings,
            &self.document_state.panel_ctx.tokens,
            &ui.preferences_draft_component_classes,
            &ui.preferences_keymap_editor,
            &ui.preferences_keymap_status,
            &ui.preferences_keymap_search,
            ui.preferences_keymap_recorder.as_ref(),
            ui.theme_id,
        )
        .map(|m| Message::Preferences(PreferencesMsg::Inner(m)));

        // OS-level drag handle covering the header strip, minus the
        // close-X hit zone on the right. Press anywhere on the title
        // bar → SC_MOVE drag the borderless OS window. Without this,
        // `decorations: false` strips the OS title bar so there's
        // nothing else for the user to grab.
        let modal = super::state::ModalId::Preferences;
        let drag_layer: Element<'_, Message> = col_widget![
            row![
                mouse_area(
                    Space::new()
                        .width(Length::Fill)
                        .height(Length::Fixed(MODAL_HEADER_HEIGHT))
                )
                .on_press(Message::Window(WindowMsg::StartDetachedWindowDrag(modal)))
                .interaction(iced::mouse::Interaction::Grab),
                Space::new()
                    .width(Length::Fixed(MODAL_CLOSE_X_HIT_W))
                    .height(Length::Fixed(MODAL_HEADER_HEIGHT)),
            ]
            .width(Length::Fill),
            Space::new().width(Length::Fill).height(Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into();

        Stack::new().push(dialog).push(drag_layer).into()
    }

    fn view_detached_modal(&self, modal: super::state::ModalId) -> Element<'_, Message> {
        use super::state::ModalId;
        match modal {
            ModalId::AnnotateDialog => self.view_annotate_dialog_body(),
            ModalId::ErcDialog => self.view_erc_dialog_body(),
            ModalId::AnnotateResetConfirm => self.view_annotate_reset_confirm_body(),
            // Stubs — these modals don't yet have extractable bodies; fall
            // back to a placeholder so the window is non-empty until their
            // body helpers land.
            ModalId::MoveSelection => self.view_move_selection_body(),
            ModalId::NetColorPalette => self.view_net_color_palette_body(),
            ModalId::ParameterManager => self.view_parameter_manager_body(),
            ModalId::PrintPreview => self.view_print_preview_body(),
            ModalId::BomPreview => {
                // Stack the body underneath a 6 px edge-resize
                // overlay so the borderless OS window can be
                // resized by dragging its edges. Without this,
                // `decorations: false` strips the OS frame and
                // there's nothing to grab.
                let body = self.view_bom_preview_body();
                let resize_active = self
                    .document_state
                    .bom_preview
                    .as_ref()
                    .map(|p| p.column_resize.is_some())
                    .unwrap_or(false);
                let mut stack = iced::widget::Stack::new()
                    .push(body)
                    .push(Self::detached_modal_resize_overlay(modal));
                // While a column-resize drag is in flight, lay an
                // invisible mouse_area over the whole modal that
                // pins the cursor to ResizingHorizontally. Without
                // this, the cursor reverts to default the moment
                // it leaves the 4 px handle's hit zone — which
                // happens immediately on horizontal drag.
                if resize_active {
                    let overlay: Element<'_, Message> = iced::widget::mouse_area(
                        iced::widget::Space::new()
                            .width(Length::Fill)
                            .height(Length::Fill),
                    )
                    .on_release(Message::BomPreview(BomPreviewMsg::ColumnResizeEnd))
                    .interaction(iced::mouse::Interaction::ResizingHorizontally)
                    .into();
                    stack = stack.push(overlay);
                }
                stack.into()
            }
            ModalId::Preferences => self.view_preferences_body(),
            ModalId::FindReplace
            | ModalId::RenameDialog
            | ModalId::RemoveDialog
            | ModalId::ProjectOptions
            | ModalId::EnableVersionControl
            | ModalId::GridProperties
            | ModalId::SelectionFilterCustom => {
                iced::widget::container(iced::widget::text("Detached modal"))
                    .padding(20)
                    .into()
            }
        }
    }

    /// Same 6 px edge-resize overlay as the main window's, but
    /// emitting `StartDetachedModalResize { modal, direction }`
    /// so it dispatches to the right OS window. Used as a stack
    /// layer above the modal's body in `view_detached_modal`.
    fn detached_modal_resize_overlay<'a>(modal: super::state::ModalId) -> Element<'a, Message> {
        use iced::mouse::Interaction;
        use iced::widget::{Space, column, mouse_area, row};
        use iced::window::Direction;

        const EDGE: f32 = 6.0;

        let straight = move |direction: Direction,
                             cursor: Interaction,
                             horizontal: bool|
              -> Element<'a, Message> {
            let (w, h) = if horizontal {
                (Length::Fill, Length::Fixed(EDGE))
            } else {
                (Length::Fixed(EDGE), Length::Fill)
            };
            mouse_area(Space::new().width(w).height(h))
                .on_press(Message::Window(WindowMsg::StartDetachedModalResize {
                    modal,
                    direction,
                }))
                .interaction(cursor)
                .into()
        };
        let corner = move |direction: Direction, cursor: Interaction| -> Element<'a, Message> {
            mouse_area(
                Space::new()
                    .width(Length::Fixed(EDGE))
                    .height(Length::Fixed(EDGE)),
            )
            .on_press(Message::Window(WindowMsg::StartDetachedModalResize {
                modal,
                direction,
            }))
            .interaction(cursor)
            .into()
        };

        let top = straight(Direction::North, Interaction::ResizingVertically, true);
        let bottom = straight(Direction::South, Interaction::ResizingVertically, true);
        let left = straight(Direction::West, Interaction::ResizingHorizontally, false);
        let right = straight(Direction::East, Interaction::ResizingHorizontally, false);
        let nw = corner(Direction::NorthWest, Interaction::ResizingDiagonallyDown);
        let ne = corner(Direction::NorthEast, Interaction::ResizingDiagonallyUp);
        let sw = corner(Direction::SouthWest, Interaction::ResizingDiagonallyUp);
        let se = corner(Direction::SouthEast, Interaction::ResizingDiagonallyDown);

        let middle = row![
            left,
            Space::new().width(Length::Fill).height(Length::Fill),
            right
        ]
        .width(Length::Fill)
        .height(Length::Fill);

        column![
            row![nw, top, ne]
                .width(Length::Fill)
                .height(Length::Fixed(EDGE)),
            middle,
            row![sw, bottom, se]
                .width(Length::Fill)
                .height(Length::Fixed(EDGE)),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_main_for(&self, window_id: iced::window::Id) -> Element<'_, Message> {
        let ui = &self.ui_state;
        let document = &self.document_state;
        let interaction = &self.interaction_state;
        // Context-aware menu: each leaf gates on whether its action
        // makes sense in the current app state. `has_schematic` /
        // `has_selection` drive most entries; undo / redo consult
        // the engine's history so they grey out when empty.
        // v0.14.2: surface the active tab's primitive-editor kind to
        // the menu so File ▸ Save / Save As enable themselves for
        // `.snxsym` and `.snxfpt` standalone editor tabs.
        let active_tab_kind = document.tabs.get(document.active_tab).map(|t| &t.kind);
        let has_symbol_editor =
            matches!(active_tab_kind, Some(crate::app::TabKind::SymbolEditor(_)));
        let has_footprint_editor = matches!(
            active_tab_kind,
            Some(crate::app::TabKind::FootprintEditor(_))
        );

        let menu_ctx = crate::menu_bar::MenuContext {
            has_schematic: self.has_active_schematic(),
            has_pcb: self.has_active_pcb(),
            has_project: document.active_project.is_some(),
            has_selection: !interaction.canvas_for_window(window_id).selected.is_empty(),
            can_undo: document
                .engine_for_window(window_id, ui)
                .map(|e| e.can_undo())
                .unwrap_or(false),
            can_redo: document
                .engine_for_window(window_id, ui)
                .map(|e| e.can_redo())
                .unwrap_or(false),
            has_symbol_editor,
            has_footprint_editor,
            // Secondary windows (detached modal, undocked tab) borrow
            // the main window's scale. Good enough until per-window
            // scale tracking lands — it's only wrong if the user drags
            // a secondary window onto a monitor with a different DPI.
            scale_factor: ui.main_window_scale,
            active_keymap: Some(ui.active_keymap.clone()),
        };
        let menu_row = menu_bar::view(&document.panel_ctx.tokens, menu_ctx).map(Message::Menu);

        let left_has_panels = document.dock.has_panels(PanelPosition::Left);
        let right_has_panels = document.dock.has_panels(PanelPosition::Right);
        let bottom_has_panels = document.dock.has_panels(PanelPosition::Bottom);
        let left_collapsed = document.dock.is_collapsed(PanelPosition::Left);
        let right_collapsed = document.dock.is_collapsed(PanelPosition::Right);
        let bottom_collapsed = document.dock.is_collapsed(PanelPosition::Bottom);

        let left = self.view_dock_panel(
            PanelPosition::Left,
            left_has_panels,
            left_collapsed,
            ui.left_width,
        );
        let left_handle = self.view_resize_handle(
            DragTarget::LeftPanel,
            left_has_panels && !left_collapsed,
            true,
        );
        let center = self.view_center(window_id);
        let right_handle = self.view_resize_handle(
            DragTarget::RightPanel,
            right_has_panels && !right_collapsed,
            true,
        );
        let right = self.view_dock_panel(
            PanelPosition::Right,
            right_has_panels,
            right_collapsed,
            ui.right_width,
        );

        let center_row = row![left, left_handle, center, right_handle, right];
        let bottom_handle = self.view_resize_handle(
            DragTarget::BottomPanel,
            bottom_has_panels && !bottom_collapsed,
            false,
        );
        let bottom = self.view_dock_panel_h(
            PanelPosition::Bottom,
            bottom_has_panels,
            bottom_collapsed,
            ui.bottom_height,
        );

        let status = status_bar::view(
            ui.cursor_x,
            ui.cursor_y,
            ui.grid_visible,
            ui.snap_enabled,
            ui.zoom,
            ui.unit,
            &interaction.current_tool,
            ui.grid_size_mm,
            &interaction.canvas_for_window(window_id).selected,
            &document.panel_ctx.tokens,
            document.inflight_git_commits.len(),
        )
        .map(|req| Message::Ui(UiMsg::StatusBar(req)));

        // Partition tabs across windows: main owns every tab that isn't
        // currently rendered by an undocked-tab window; each undocked
        // window owns exactly its one tab. Closing a tab in one window
        // can no longer reach tabs that belong to the other.
        let all_undocked_paths: std::collections::HashSet<std::path::PathBuf> = ui
            .windows
            .values()
            .filter_map(|kind| match kind {
                super::state::WindowKind::UndockedTab { path, .. } => Some(path.clone()),
                _ => None,
            })
            .collect();
        let is_main_window = ui.main_window_id == Some(window_id);

        // Main window is borderless: wordmark + menus + drag + search +
        // min/max/close in a single 36 px row. Undocked tab windows keep
        // their OS chrome and use the plain styled strip.
        let top_chrome: Element<'_, Message> = if is_main_window {
            self.view_main_window_chrome(menu_row, &document.panel_ctx.tokens)
        } else {
            menu_bar::wrap_plain(menu_row, &document.panel_ctx.tokens)
        };
        let mut main = column![top_chrome];
        let visible_paths: std::collections::HashSet<std::path::PathBuf> = if is_main_window {
            document
                .tabs
                .iter()
                .map(|t| t.path.clone())
                .filter(|p| !all_undocked_paths.contains(p))
                .collect()
        } else {
            match ui.windows.get(&window_id) {
                Some(super::state::WindowKind::UndockedTab { path, .. }) => {
                    std::iter::once(path.clone()).collect()
                }
                _ => std::collections::HashSet::new(),
            }
        };
        // Reserve the tab strip's vertical footprint regardless of
        // whether any document is open — opening the first document
        // would otherwise shift the entire chrome down by ~24 px,
        // which feels jarring. The 1 px chrome separator stays
        // visible too so the menu row always reads as a distinct
        // band above the tab strip.
        main = main.push(
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(1)
                .style(crate::styles::chrome_separator(&document.panel_ctx.tokens)),
        );
        if !document.tabs.is_empty() && !visible_paths.is_empty() {
            // Resolve "really dragging" — Some only after the
            // cursor has travelled past a 6 px threshold from
            // the press origin. Without this, every click-to-
            // switch armed the drag state and flipped the
            // cursor to Grabbing instantaneously, plus flashed
            // the drag ghost.
            const DRAG_THRESHOLD_PX: f32 = 6.0;
            let dragging = ui.tab_dragging.and_then(|(idx, ox, oy)| {
                let (mx, my) = interaction.last_mouse_pos;
                let dx = mx - ox;
                let dy = my - oy;
                if dx * dx + dy * dy > DRAG_THRESHOLD_PX * DRAG_THRESHOLD_PX {
                    Some(idx)
                } else {
                    None
                }
            });
            main = main.push(
                tab_bar::view(
                    &document.tabs,
                    document.active_tab,
                    dragging,
                    &visible_paths,
                    &document.panel_ctx.tokens,
                )
                .map(move |msg| Message::Tab { window_id, msg }),
            );
        } else {
            // Empty placeholder strip with the same metrics as
            // tab_bar::view: 2 px outer padding + 22 px tall inner
            // pill = 26 px total. Without this the chrome jumps
            // when the first tab opens.
            let placeholder = container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(26)
                .style(crate::styles::toolbar_strip(&document.panel_ctx.tokens));
            main = main.push(placeholder);
        }
        let main = main
            .push(center_row)
            .push(bottom_handle)
            .push(bottom)
            .push(status);

        // Borderless window needs its own edge-resize hit zones — the OS
        // frame would normally handle this, but `decorations: false`
        // removes WS_THICKFRAME on Windows. Tab windows keep OS
        // decorations so they skip the overlay entirely. The overlay is
        // applied later as a Stack layer over `main` so the content
        // keeps its natural origin and overlay y-coordinates stay
        // correct.
        let main: Element<'_, Message> = main.into();

        // v0.13 — `has_active_bar` is now true for ANY editor tab
        // that mounts an active bar (schematic / footprint /
        // symbol library) so the layers Stack mounts and the bar
        // layer fires from `view_main_for` regardless of editor
        // kind.
        let active_tab_kind_any = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|t| &t.kind);
        let has_footprint_bar = matches!(
            active_tab_kind_any,
            Some(crate::app::TabKind::FootprintEditor(_))
        );
        let has_symbol_bar = matches!(
            active_tab_kind_any,
            Some(crate::app::TabKind::SymbolEditor(_))
        );
        let has_active_bar = self.has_active_schematic() || has_footprint_bar || has_symbol_bar;
        let dragging_tab = ui.tab_dragging.is_some();
        let needs_overlay = has_active_bar
            || interaction.editing_text.is_some()
            || interaction.context_menu.is_some()
            || interaction.project_tree_context_menu.is_some()
            || interaction.tab_context_menu.is_some()
            || interaction.active_bar_menu.is_some()
            || interaction.canvas.placement_paused
            || self
                .document_state
                .tabs
                .get(self.document_state.active_tab)
                .and_then(|tab| tab.kind.as_footprint_editor())
                .and_then(|path| self.document_state.footprint_editors.get(path))
                .map(|ed| {
                    ed.state.placement_paused
                        || ed.state.active_bar_menu.is_some()
                        || ed.state.move_by_modal.is_some()
                })
                .unwrap_or(false)
            || ui.panel_list_open
            || ui.find_replace.open
            || ui.preferences_open
            || ui.keyboard_shortcuts_open
            || ui.first_run_tour_open
            || ui.rename_dialog.is_some()
            || ui.remove_dialog.is_some()
            || ui.project_close_confirm.is_some()
            || ui.app_quit_confirm.is_some()
            || ui.project_options.is_some()
            || ui.enable_version_control.is_some()
            || ui.grid_properties.is_some()
            || ui.selection_filter_custom.is_some()
            || interaction.grid_picker.is_some()
            || document.bom_preview.is_some()
            || ui.annotate_dialog_open
            || ui.annotate_reset_confirm
            || ui.erc_dialog_open
            || !document.dock.floating.is_empty()
            || dragging_tab
            || ui.net_color_custom.show
            // Library-side modals (New Component, Place Component picker,
            // Pick Symbol/Footprint primitive picker) can be triggered
            // from non-canvas contexts — e.g. the Library Browser tab's
            // Add Component button. Without these flags the overlay
            // Stack would never be built and the modal layer in
            // collect_overlays would silently no-op.
            || self.library.new_component.is_some()
            || self.library.picker.is_some()
            || self.library.primitive_picker.is_some()
            || self.library.close_library_confirm.is_some()
            || self.library.document_options.is_some()
            || self.library.recovery.is_some()
            || self.library.create_options.is_some()
            || self.library.library_updates.is_some()
            || self
                .library
                .library_browsers
                .values()
                .any(|s| s.edit_modal.is_some() || s.delete_confirm.is_some())
            || ui.command_palette.open
            // Hover tooltip — needs the overlay stack to render even
            // when no other modal is open; otherwise `view_hover_tooltip`
            // produces an Element that's silently dropped on every
            // frame the user hovers a symbol over a bare canvas.
            // Mirrors the needs_overlay-predicate-gates-modal pattern.
            || (interaction.hover_symbol_uuid.is_some()
                && interaction
                    .hover_started_at
                    .is_some_and(|t| t.elapsed() >= std::time::Duration::from_millis(700)));

        if needs_overlay {
            let mut overlays = self.collect_overlays();
            // Tab drag ghost: only renders once the cursor has
            // travelled past the same 6 px threshold the cursor
            // gating uses (`tab_bar::view`). Mirrors that gate
            // here so press-without-move keeps the ghost off.
            if let Some((tab_idx, ox, oy)) = ui.tab_dragging
                && let Some(tab) = document.tabs.get(tab_idx)
            {
                const DRAG_GHOST_THRESHOLD_PX: f32 = 6.0;
                let (mx, my) = interaction.last_mouse_pos;
                let dx = mx - ox;
                let dy = my - oy;
                if dx * dx + dy * dy > DRAG_GHOST_THRESHOLD_PX * DRAG_GHOST_THRESHOLD_PX {
                    overlays.push(self.view_tab_drag_ghost(&tab.title));
                }
            }
            let mut stack = iced::widget::Stack::new().push(main);
            // Resize edges sit above the content but below functional
            // overlays (Active Bar, menus, modals) so the 6 px border
            // strip doesn't eat clicks on those.
            if is_main_window {
                stack = stack.push(Self::resize_edges_overlay());
            }
            for overlay in overlays {
                stack = stack.push(overlay);
            }
            stack.into()
        } else if is_main_window {
            iced::widget::Stack::new()
                .push(main)
                .push(Self::resize_edges_overlay())
                .into()
        } else {
            main.into()
        }
    }

    /// Full-window-sized Stack overlay that anchors 6 px resize hit
    /// zones at the borderless main window's edges and corners. Clicks
    /// on the edges call `iced::window::drag_resize` via
    /// `StartMainWindowResize`; anywhere in the middle is an empty
    /// `Space` so events fall through to the content layer below.
    ///
    /// Used as a stack layer over `main` rather than as a structural
    /// wrapper, so the content keeps its natural y-origin and overlay
    /// coordinates (Active Bar, text edit, net-colour picker) stay
    /// correct without a +EDGE correction everywhere.
    fn resize_edges_overlay<'a>() -> Element<'a, Message> {
        use iced::mouse::Interaction;
        use iced::widget::{Space, column, mouse_area, row};
        use iced::window::Direction;

        const EDGE: f32 = 6.0;

        let straight =
            |direction: Direction, cursor: Interaction, horizontal: bool| -> Element<'a, Message> {
                let (w, h) = if horizontal {
                    (Length::Fill, Length::Fixed(EDGE))
                } else {
                    (Length::Fixed(EDGE), Length::Fill)
                };
                mouse_area(Space::new().width(w).height(h))
                    .on_press(Message::Window(WindowMsg::StartMainWindowResize(direction)))
                    .interaction(cursor)
                    .into()
            };

        let corner = |direction: Direction, cursor: Interaction| -> Element<'a, Message> {
            mouse_area(
                Space::new()
                    .width(Length::Fixed(EDGE))
                    .height(Length::Fixed(EDGE)),
            )
            .on_press(Message::Window(WindowMsg::StartMainWindowResize(direction)))
            .interaction(cursor)
            .into()
        };

        let top = straight(Direction::North, Interaction::ResizingVertically, true);
        let bottom = straight(Direction::South, Interaction::ResizingVertically, true);
        let left = straight(Direction::West, Interaction::ResizingHorizontally, false);
        let right = straight(Direction::East, Interaction::ResizingHorizontally, false);
        let nw = corner(Direction::NorthWest, Interaction::ResizingDiagonallyDown);
        let ne = corner(Direction::NorthEast, Interaction::ResizingDiagonallyUp);
        let sw = corner(Direction::SouthWest, Interaction::ResizingDiagonallyUp);
        let se = corner(Direction::SouthEast, Interaction::ResizingDiagonallyDown);

        // Middle row: left/right edges frame a Fill/Fill empty Space so
        // the whole overlay is window-sized and the centre passes
        // clicks through.
        let middle = row![
            left,
            Space::new().width(Length::Fill).height(Length::Fill),
            right
        ]
        .width(Length::Fill)
        .height(Length::Fill);

        column![
            row![nw, top, ne]
                .width(Length::Fill)
                .height(Length::Fixed(EDGE)),
            middle,
            row![sw, bottom, se]
                .width(Length::Fill)
                .height(Length::Fixed(EDGE)),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_dock_panel(
        &self,
        pos: PanelPosition,
        has_panels: bool,
        collapsed: bool,
        size: f32,
    ) -> Element<'_, Message> {
        let panel = self
            .document_state
            .dock
            .view_region(pos, &self.document_state.panel_ctx, &self.library)
            .map(Message::Dock);
        let width = if !has_panels {
            0.0
        } else if collapsed {
            28.0
        } else {
            size
        };
        container(panel)
            .width(width)
            .height(Length::Fill)
            .style(crate::styles::panel_region(
                &self.document_state.panel_ctx.tokens,
            ))
            .into()
    }

    fn view_dock_panel_h(
        &self,
        pos: PanelPosition,
        has_panels: bool,
        collapsed: bool,
        size: f32,
    ) -> Element<'_, Message> {
        let panel = self
            .document_state
            .dock
            .view_region(pos, &self.document_state.panel_ctx, &self.library)
            .map(Message::Dock);
        let height = if !has_panels {
            0.0
        } else if collapsed {
            28.0
        } else {
            size
        };
        container(panel)
            .width(Length::Fill)
            .height(height)
            .style(crate::styles::panel_region(
                &self.document_state.panel_ctx.tokens,
            ))
            .into()
    }

    fn view_resize_handle(
        &self,
        target: DragTarget,
        visible: bool,
        horizontal: bool,
    ) -> Element<'_, Message> {
        let size = if visible { 5 } else { 0 };
        let handle_container = if horizontal {
            container(iced::widget::Space::new())
                .width(size)
                .height(Length::Fill)
                .style(crate::styles::resize_handle(
                    &self.document_state.panel_ctx.tokens,
                ))
        } else {
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(size)
                .style(crate::styles::resize_handle(
                    &self.document_state.panel_ctx.tokens,
                ))
        };
        let interaction = if horizontal {
            iced::mouse::Interaction::ResizingHorizontally
        } else {
            iced::mouse::Interaction::ResizingVertically
        };
        iced::widget::mouse_area(handle_container)
            .interaction(interaction)
            .on_press(Message::Ui(UiMsg::DragStart(target)))
            .into()
    }

    fn view_center(&self, window_id: iced::window::Id) -> Element<'_, Message> {
        let is_main = self.ui_state.main_window_id == Some(window_id);

        // When the active tab is a Component Preview, render the
        // editor inside the main window's content pane. The same
        // surface lights up via the `WindowKind::ComponentEditor`
        // branch in `view()` when the user undocks the tab into its
        // own OS window.
        if is_main
            && let Some(active_tab) = self.document_state.tabs.get(self.document_state.active_tab)
            && let Some(editor_id) = active_tab.kind.as_component_editor()
        {
            let tokens = &self.document_state.panel_ctx.tokens;
            let address = crate::library::state::EditorAddress::new(
                editor_id.library_path.clone(),
                editor_id.table.clone(),
                editor_id.row_id,
            );
            return if let Some(editor) = self.library.editors.get(&address) {
                crate::library::editor::view(editor, &self.library, tokens, address)
                    .map(Message::Library)
            } else {
                container(
                    column![
                        iced::widget::text("Component Editor — state not yet loaded")
                            .size(13)
                            .color(crate::styles::ti(tokens.text_secondary)),
                    ]
                    .spacing(4)
                    .align_x(iced::Alignment::Center),
                )
                .center(Length::Fill)
                .style(crate::styles::panel_region(tokens))
                .into()
            };
        }

        // Standalone primitive editor tabs. `.snxsym` / `.snxfpt`
        // open as main-window document tabs alongside `.snxsch` /
        // `.snxpcb`. Lookup is path-keyed via
        // `DocumentState.symbol_editors` / `footprint_editors`.
        if is_main
            && let Some(active_tab) = self.document_state.tabs.get(self.document_state.active_tab)
        {
            if let Some(path) = active_tab.kind.as_symbol_editor()
                && let Some(editor) = self.document_state.symbol_editors.get(path)
            {
                let panel_ctx = &self.document_state.panel_ctx;
                // Per-library display settings — Altium-style
                // Document Options. Resolve the `.snxlib/` ancestor
                // of the symbol's path so every primitive editor
                // opened from the same library shares the same
                // grid / unit / background. Lone-file edits (no
                // mounted library) get safe defaults.
                let display = self
                    .library
                    .containing_library(path)
                    .map(|lib| lib.display)
                    .unwrap_or_default();
                let theme_id = self.ui_state.theme_id;
                return crate::library::editor::standalone::view_symbol(
                    editor, panel_ctx, display, theme_id, path,
                )
                .map(Message::Library);
            }
            if let Some(path) = active_tab.kind.as_footprint_editor()
                && let Some(editor) = self.document_state.footprint_editors.get(path)
            {
                let tokens = &self.document_state.panel_ctx.tokens;
                let theme_id = self.ui_state.theme_id;
                let custom_presets = &self.interaction_state.custom_filter_presets;
                return crate::library::editor::standalone::view_footprint(
                    editor,
                    tokens,
                    theme_id,
                    custom_presets,
                )
                .map(Message::Library);
            }
            // Library Browser tab — `.snxlib` opened as a main-window
            // tab. Per-tab state lives in
            // `LibraryState.library_browsers` keyed by the same path
            // that lives on `TabInfo.path`.
            if let Some(path) = active_tab.kind.as_library_browser() {
                let tokens = &self.document_state.panel_ctx.tokens;
                if let Some(browser) = self.library.library_browsers.get(path) {
                    return crate::library::browser::view(path, &self.library, browser, tokens)
                        .map(Message::Library);
                } else {
                    // Fallback when somehow the browser-state map is
                    // out of sync with the tabs vector. Keeps the tab
                    // renderable rather than crashing.
                    return container(
                        iced::widget::text("Library Browser — state not yet loaded")
                            .size(13)
                            .color(crate::styles::ti(tokens.text_secondary)),
                    )
                    .center(Length::Fill)
                    .style(crate::styles::panel_region(tokens))
                    .into();
                }
            }
        }

        let has_schematic = if is_main {
            self.has_active_schematic()
        } else {
            // An undocked tab window renders if its path still has a
            // live engine in the HashMap. Falls back to the main
            // predicate when the window has already been dropped from
            // the windows map (mid-close frame).
            self.document_state
                .engine_for_window(window_id, &self.ui_state)
                .is_some()
        };
        if has_schematic {
            // Canvas events from non-main windows need to carry the
            // window_id through to the dispatch layer so the right
            // per-window canvas receives the mutation. Keyboard
            // shortcuts that synthesize `Message::CanvasEvent` keep
            // targeting the main canvas unchanged.
            let base: Element<'_, Message> =
                canvas(self.interaction_state.canvas_for_window(window_id))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into();
            if is_main {
                base
            } else {
                base.map(move |msg| match msg {
                    Message::CanvasEvent(event) => {
                        Message::CanvasEventInWindow { window_id, event }
                    }
                    other => other,
                })
            }
        } else if self.has_active_pcb() {
            canvas(&self.interaction_state.pcb_canvas)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            // Distinguish "nothing loaded at all" from "project loaded,
            // but no document picked yet" — the second case is what
            // the user sees right after opening a .standard_pro before
            // clicking any node in the project tree.
            let (title, hint) = if self.document_state.active_project.is_some() {
                (
                    "No document selected",
                    "Choose a schematic or PCB from the project tree".to_string(),
                )
            } else {
                let open_shortcut = self.keymap_shortcut_label("open_document", "Ctrl+O");
                (
                    "No document open",
                    format!("Open a project with File > Open or {open_shortcut}"),
                )
            };
            container(
                column![
                    iced::widget::text(title).size(14).color(crate::styles::ti(
                        self.document_state.panel_ctx.tokens.text_secondary
                    )),
                    iced::widget::text(hint).size(11).color(crate::styles::ti(
                        self.document_state.panel_ctx.tokens.text_secondary
                    )),
                ]
                .spacing(8)
                .align_x(iced::Alignment::Center),
            )
            .center(Length::Fill)
            .style(crate::styles::panel_region(
                &self.document_state.panel_ctx.tokens,
            ))
            .into()
        }
    }

    /// Hover tooltip card showing the placed symbol's designator,
    /// value, footprint, and library id. Only paints after the cursor
    /// has dwelled on a Symbol hit for >= 250 ms — gates impulsive
    /// motion from popping the card. Returns None when the gate
    /// hasn't tripped, when no schematic is active, or when the
    /// uuid no longer resolves (e.g. the symbol was deleted while
    /// the dwell timer was running).
    fn view_hover_tooltip(&self) -> Option<Element<'_, Message>> {
        use iced::widget::{column, container, text};
        use iced::{Background, Border, Color};

        let interaction = &self.interaction_state;
        let uuid = interaction.hover_symbol_uuid?;
        let started = interaction.hover_started_at?;
        if started.elapsed() < std::time::Duration::from_millis(700) {
            return None;
        }
        let (sx, sy) = interaction.hover_screen_pos?;
        let active_path = self.document_state.active_path.as_ref()?;
        let engine = self.document_state.engines.get(active_path)?;
        let symbol = engine.document().symbols.iter().find(|s| s.uuid == uuid)?;

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let muted_c = crate::styles::ti(tokens.text_secondary);
        let panel_bg = crate::styles::ti(tokens.panel_bg);
        let border_c = crate::styles::ti(tokens.border);
        // Match the schematic's own font (Iosevka by default; whatever
        // the user picked under Preferences ▸ Canvas Font) so the
        // tooltip reads as "this is data from the canvas" rather than
        // floating Roboto chrome. We reuse the app's `IOSEVKA` font
        // constant for canvas text so the lookup
        // hits the embedded TTF (not whatever the system fontconfig
        // falls back to for `Family::Name`).
        let canvas_font_name: &str = &self.document_state.panel_ctx.canvas_font_name;
        let canvas_font = if canvas_font_name == crate::fonts::DEFAULT_CANVAS_FONT
            || canvas_font_name.is_empty()
        {
            crate::render_config::IOSEVKA
        } else {
            crate::fonts::iced_font_for_family(canvas_font_name)
        };

        // Single row style — label in a fixed gutter, value fills the
        // remaining card width and wraps onto further lines on its
        // own when the payload is long. Keeps the rhythm uniform
        // across short (Designator / Value) and long (Footprint /
        // Library) fields without an inline-vs-stacked split.
        const CARD_W: f32 = 260.0;
        const LABEL_W: f32 = 60.0;
        let field = |label: &'static str, value: String| -> Element<'_, Message> {
            iced::widget::row![
                text(label)
                    .font(canvas_font)
                    .size(11)
                    .color(muted_c)
                    .width(LABEL_W),
                text(value)
                    .font(canvas_font)
                    .size(12)
                    .color(text_c)
                    .width(Length::Fill),
            ]
            .spacing(4)
            .width(Length::Fill)
            .align_y(iced::Alignment::Start)
            .into()
        };

        let mut rows: Vec<Element<'_, Message>> = Vec::with_capacity(4);
        rows.push(field("Designator", symbol.reference.clone()));
        if !symbol.value.is_empty() {
            rows.push(field("Value", symbol.value.clone()));
        }
        if !symbol.footprint.is_empty() {
            rows.push(field("Footprint", symbol.footprint.clone()));
        }
        if !symbol.lib_id.is_empty() {
            rows.push(field("Library", symbol.lib_id.clone()));
        }

        let card = container(column(rows).spacing(3))
            .padding(iced::Padding {
                top: 8.0,
                right: 14.0,
                bottom: 8.0,
                left: 10.0,
            })
            .width(Length::Fixed(CARD_W))
            .style(move |_: &iced::Theme| container::Style {
                background: Some(Background::Color(panel_bg)),
                border: Border {
                    color: border_c,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..container::Style::default()
            });

        // Offset to bottom-right so the card never sits directly under
        // the cursor — keeps the underlying symbol visible and avoids
        // hover flicker when the tooltip itself enters the cursor's
        // hover rectangle.
        const OFFSET: f32 = 16.0;
        let (ww, wh) = self.ui_state.window_size;
        // Conservative card-size estimate for edge clamping. The
        // actual size depends on font metrics so this is a guess
        // intended to keep the card on-screen near the right/bottom
        // edges; the iced layout will still render at its true size.
        const ESTIMATED_W: f32 = CARD_W;
        const ESTIMATED_H: f32 = 110.0;
        let mut x = sx + OFFSET;
        let mut y = sy + OFFSET;
        if x + ESTIMATED_W > ww {
            x = (sx - OFFSET - ESTIMATED_W).max(0.0);
        }
        if y + ESTIMATED_H > wh {
            y = (sy - OFFSET - ESTIMATED_H).max(0.0);
        }
        Some(super::view::translate::Translate::new(card, (x, y)).into())
    }

    /// Result list for the chrome-strip command palette. Anchored
    /// below the chrome strip; scoring + ranking happens in the
    /// `command_palette` module so this view stays a thin renderer.
    fn view_command_palette_dropdown(&self) -> Element<'_, Message> {
        use crate::app::command_palette::{
            CommandSource, MAX_RESULTS, build_catalog, rank_results,
        };
        use iced::widget::{Space, button, column, container, row, scrollable, text};
        use iced::{Alignment, Background, Border, Color};

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let muted_c = crate::styles::ti(tokens.text_secondary);
        let panel_bg = crate::styles::ti(tokens.panel_bg);
        let border_c = crate::styles::ti(tokens.border);
        let accent_c = crate::styles::ti(tokens.accent);
        let hover_c = crate::styles::ti(tokens.hover);

        let catalog = build_catalog(self);
        let ranked = rank_results(&catalog, &self.ui_state.command_palette.query);
        let total = ranked.len();
        let selected = self
            .ui_state
            .command_palette
            .selected_index
            .min(total.saturating_sub(1));

        let mut rows: Vec<Element<'_, Message>> = Vec::with_capacity(MAX_RESULTS.min(total));
        for (display_idx, &(catalog_idx, _score)) in ranked.iter().take(MAX_RESULTS).enumerate() {
            let entry = &catalog[catalog_idx];
            let is_active = display_idx == selected;
            let row_bg = if is_active {
                Some(Background::Color(hover_c))
            } else {
                None
            };
            let source_label = match entry.source {
                CommandSource::Command => "Command",
                CommandSource::Symbol => "Symbol",
                CommandSource::File => "File",
            };
            let label_col = column![
                text(entry.label.clone()).size(12).color(text_c),
                text(if entry.detail.is_empty() {
                    String::new()
                } else {
                    entry.detail.clone()
                })
                .size(10)
                .color(muted_c),
            ]
            .spacing(2)
            .width(Length::Fill);
            let row_inner = row![label_col, text(source_label).size(10).color(muted_c),]
                .spacing(10)
                .align_y(Alignment::Center);
            let btn = button(row_inner)
                .width(Length::Fill)
                .padding([6, 12])
                .on_press(Message::CommandPalette(CommandPaletteMsg::Select(
                    display_idx,
                )))
                .style(move |_: &iced::Theme, status: button::Status| {
                    let bg = match status {
                        button::Status::Hovered | button::Status::Pressed => {
                            Some(Background::Color(hover_c))
                        }
                        _ => row_bg,
                    };
                    button::Style {
                        background: bg,
                        border: Border {
                            width: if is_active { 1.0 } else { 0.0 },
                            radius: 3.0.into(),
                            color: if is_active {
                                accent_c
                            } else {
                                Color::TRANSPARENT
                            },
                        },
                        text_color: text_c,
                        ..button::Style::default()
                    }
                });
            rows.push(btn.into());
        }

        let body: Element<'_, Message> = if total == 0 {
            container(text("No results").size(12).color(muted_c))
                .padding([12, 14])
                .width(Length::Fill)
                .into()
        } else {
            let list = column(rows).spacing(2).padding(4);
            scrollable(list).height(Length::Shrink).into()
        };

        // Footer when there are more matches than we render.
        let footer: Element<'_, Message> = if total > MAX_RESULTS {
            container(
                text(format!(
                    "{} more results — refine query",
                    total - MAX_RESULTS
                ))
                .size(10)
                .color(muted_c),
            )
            .padding([4, 14])
            .width(Length::Fill)
            .into()
        } else {
            Space::new().height(0).into()
        };

        // Card width matches the chrome search bar exactly so the
        // dropdown reads as an extension of the input rather than a
        // floating popup that happens to be nearby.
        let card_w = CHROME_SEARCH_BAR_WIDTH;
        let card = container(column![body, footer])
            .width(card_w)
            .max_height(360.0)
            .padding(0)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(Background::Color(panel_bg)),
                border: Border {
                    color: border_c,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..container::Style::default()
            });

        let (ww, _wh) = self.ui_state.window_size;
        // Track the chrome search bar's actual layout position so the
        // dropdown lines up with the input. The chrome row is
        // `[menu, drag_fill, search, drag_fill, controls]`; the two
        // Fill drag zones split the leftover evenly, so the search
        // bar starts at `menu_w + leftover/2`.
        let menu_w = crate::menu_bar::approx_menu_bar_width();
        let leftover = (ww - menu_w - card_w - CHROME_CONTROLS_W).max(0.0);
        let x = (menu_w + leftover / 2.0).max(8.0);
        let y = crate::menu_bar::MENU_BAR_HEIGHT + 4.0;
        super::view::translate::Translate::new(card, (x, y)).into()
    }

    fn dismiss_layer(on_press: Message) -> Element<'static, Message> {
        // Opaque semi-transparent backdrop that blocks interaction with
        // underlying content. Left-click anywhere on it triggers the
        // dismiss message.
        //
        // We intentionally do *not* wire `on_right_press` — iced's
        // `mouse_area` would `capture_event()` the right-press and
        // prevent the underlying canvas from starting a pan. Instead
        // the canvas itself owns the right-press (its pan gesture) and
        // closes the context menu once the pan actually starts moving
        // (see `canvas/mod.rs`'s `CursorMoved` handler, which fires
        // `ContextMenuMsg::Close` the moment `pan_moved` flips on).
        const BACKDROP_OPACITY: f32 = 0.55;
        iced::widget::mouse_area(
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        0.0,
                        0.0,
                        0.0,
                        BACKDROP_OPACITY,
                    ))),
                    ..container::Style::default()
                }),
        )
        .on_press(on_press)
        .into()
    }

    fn collect_overlays(&self) -> Vec<Element<'_, Message>> {
        let ui = &self.ui_state;
        let document = &self.document_state;
        let interaction = &self.interaction_state;
        let mut layers = Vec::new();

        // Export-error modal — appears when PDF / netlist / BOM export
        // hits a user-actionable failure (write permission, invalid path,
        // empty schematic). Dismiss via OK button or clicking outside.
        if document.export_error.is_some() {
            layers.push(Self::dismiss_layer(Message::Export(
                ExportMsg::DismissError,
            )));
            layers.push(self.view_export_error());
        }

        // Print preview overlay — Altium parity: opens as a separate OS
        // window (see `handle_print_preview_requested → handle_detach_modal`)
        // so it can be dragged outside the app's client area. Only fall
        // back to the in-window overlay if the OS window failed to open.
        let preview_detached = ui.windows.values().any(|kind| {
            matches!(
                kind,
                super::state::WindowKind::DetachedModal(super::state::ModalId::PrintPreview)
            )
        });
        if document.preview.is_some() && !preview_detached {
            layers.push(self.view_print_preview());
        }

        // BOM preview overlay — same detach-first pattern as Print Preview.
        let bom_detached = ui.windows.values().any(|kind| {
            matches!(
                kind,
                super::state::WindowKind::DetachedModal(super::state::ModalId::BomPreview)
            )
        });
        if document.bom_preview.is_some() && !bom_detached {
            layers.push(self.view_bom_preview());
        }

        // Custom net-colour picker. Bespoke modal (not the iced_aw
        // ColorPicker) because the user needs a quick-pick palette +
        // precise RGB inputs side-by-side.
        if ui.net_color_custom.show {
            layers.push(Self::dismiss_layer(Message::NetColor(
                NetColorMsg::CustomShow(false),
            )));
            layers.push(self.view_net_color_custom_picker());
        }

        // Blocking modals must own the overlay stack. If we keep adding
        // tool/menu overlays after these, they can end up visually above
        // the modal and make the dialog look broken.
        let has_blocking_modal = document.export_error.is_some()
            || document.preview.is_some()
            || ui.net_color_custom.show;
        if has_blocking_modal {
            return layers;
        }

        // Altium-style pause overlay: big centered "Placement Paused" card
        // with a Resume button. Clicking Resume clears `pre_placement`,
        // un-pauses the canvas, and drops back to the active placement tool
        // so the user can keep dropping objects with the edited properties.
        // v0.13 — Also fires when a footprint editor's placement is paused
        // so TAB during pad/via/string placement surfaces the same overlay.
        let footprint_paused = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|tab| tab.kind.as_footprint_editor())
            .and_then(|path| self.document_state.footprint_editors.get(path))
            .map(|ed| ed.state.placement_paused)
            .unwrap_or(false);
        if interaction.canvas.placement_paused || footprint_paused {
            let tokens = &document.panel_ctx.tokens;
            let panel_bg = crate::styles::ti(tokens.panel_bg);
            let text_c = crate::styles::ti(tokens.text);
            let accent_c = crate::styles::ti(tokens.accent);
            let border_c = crate::styles::ti(tokens.border);
            let card = container(
                column![
                    iced::widget::text("⏸").size(64).color(accent_c),
                    iced::widget::text("Placement Paused")
                        .size(16)
                        .color(text_c),
                    iced::widget::text(
                        "Editing properties in the panel. Click Resume to keep placing."
                    )
                    .size(11)
                    .color(text_c),
                    iced::widget::Space::new().height(6.0),
                    iced::widget::button(
                        iced::widget::text("Resume Placement")
                            .size(12)
                            .color(iced::Color::WHITE)
                    )
                    .padding([6, 18])
                    .on_press(Message::Tool(ToolMessage::ResumePlacement))
                    .style(iced::widget::button::primary),
                ]
                .spacing(8)
                .align_x(iced::Alignment::Center),
            )
            .padding(24)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(iced::Color {
                    a: 0.92,
                    ..panel_bg
                })),
                border: iced::Border {
                    color: border_c,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..container::Style::default()
            });
            layers.push(
                container(card)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .into(),
            );
        }

        if self.has_active_schematic() {
            let y_offset: f32 = crate::menu_bar::MENU_BAR_HEIGHT
                + if document.tabs.is_empty() { 0.0 } else { 28.0 };
            // Active Bar overlay is only painted on the main window, so
            // the main canvas's selection set is the right gate.
            let bar_has_selection = !interaction.canvas.selected.is_empty();
            let bar_has_net_colors = !ui.net_colors.is_empty();
            let bar = crate::active_bar::view_bar(
                interaction.current_tool,
                interaction.draw_mode,
                &interaction.last_tool,
                &document.panel_ctx.tokens,
                self.ui_state.theme_id,
                bar_has_selection,
                bar_has_net_colors,
            )
            .map(Message::ActiveBar);
            layers.push(
                column![
                    iced::widget::Space::new().height(y_offset + 4.0),
                    container(bar)
                        .width(Length::Fill)
                        .align_x(iced::alignment::Horizontal::Center),
                ]
                .into(),
            );
        }

        // v0.13 — footprint editor active bar mounted at the SAME
        // app-view layer as the schematic's. Earlier the bar lived
        // inside the standalone editor body's canvas Stack, which
        // gave it canvas-relative coordinates that drifted from the
        // schematic's window-absolute coordinates. Mounting both at
        // the layers Stack with identical `Space::height(y_offset +
        // 4.0)` math guarantees pixel-identical screen y.
        if let Some(active_tab) = self.document_state.tabs.get(self.document_state.active_tab)
            && let Some(path) = active_tab.kind.as_footprint_editor()
            && let Some(editor) = self.document_state.footprint_editors.get(path)
        {
            let y_offset: f32 = crate::menu_bar::MENU_BAR_HEIGHT
                + if document.tabs.is_empty() { 0.0 } else { 28.0 };
            let theme_id = self.ui_state.theme_id;
            let tokens = &document.panel_ctx.tokens;
            // Task 6 — footprint-native presets (`SelectionFilterKind`),
            // not the schematic `custom_filter_presets`.
            let footprint_presets = &interaction.footprint_filter_presets;
            // Mount BYTE-FOR-BYTE same as the schematic: build items,
            // call `signex_widgets::active_bar::view` directly, then
            // `.map(...)` then wrap in container().width(Fill).align_x(
            // Center). Dropdown overlay is a separate layer pushed
            // after the bar.
            let bar_items = crate::library::editor::footprint::unified_active_bar::bar_items(
                editor, theme_id, tokens,
            );
            let bar = signex_widgets::active_bar::view(bar_items, tokens).map(Message::Library);
            layers.push(
                column![
                    iced::widget::Space::new().height(y_offset + 4.0),
                    container(bar)
                        .width(Length::Fill)
                        .align_x(iced::alignment::Horizontal::Center),
                ]
                .into(),
            );
            // Position the dropdown panel directly below the bar's
            // bottom edge. Bar-height = 28 button + 6 padding + 2
            // border = 36; plus the 4 px top margin from the column
            // above = 40 px tall block. `y_offset + 40` is the bar's
            // bottom; add a 2 px gap so the dropdown visually
            // touches without overlapping the border.
            let dropdown_top: u16 = (y_offset as u16).saturating_add(42);
            if let Some(overlay) =
                crate::library::editor::footprint::unified_active_bar::dropdown_overlay(
                    editor,
                    theme_id,
                    tokens,
                    footprint_presets,
                    dropdown_top,
                    self.ui_state.window_size.0,
                )
            {
                layers.push(overlay.map(Message::Library));
            }

            // v0.26 — right-click context menu overlay for the
            // footprint canvas. Sits above the active-bar dropdown so
            // a long-press menu is occluded by — never under — its
            // own dismiss layer. Window-absolute (x, y) come from the
            // canvas''s ButtonReleased(Right) handler.
            if let Some(menu_state) = editor.state.context_menu.as_ref()
                && let Some(card) =
                    crate::library::editor::footprint::context_menu::view_context_menu(
                        editor,
                        tokens,
                        path,
                        document.pad_clipboard.is_some(),
                        &self.ui_state.active_keymap,
                    )
            {
                // Dismiss layer — left-click anywhere outside closes
                // the menu. Right-press passes through to the canvas
                // (so a right-drag-to-pan gesture starts pan motion +
                // closes the menu via the CursorMoved threshold).
                let close_msg = Message::Library(
                    crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                        path: path.to_path_buf(),
                        msg: crate::library::messages::PrimitiveEdit::Footprint(
                            crate::library::messages::FootprintEditorMsg::CloseContextMenu,
                        ),
                    },
                );
                layers.push(Self::dismiss_layer(close_msg));
                let card_msg = card.map(Message::Library);
                let (ww, wh) = self.ui_state.window_size;
                // Conservative footprint estimate so the card stays on
                // screen near right / bottom edges.
                let est_menu_w: f32 = 220.0;
                let est_menu_h: f32 = 320.0;
                let edge_margin: f32 = 4.0;
                let x = if menu_state.x + est_menu_w + edge_margin > ww {
                    (ww - est_menu_w - edge_margin).max(0.0)
                } else {
                    menu_state.x
                };
                let y = if menu_state.y + est_menu_h + edge_margin > wh {
                    (menu_state.y - est_menu_h).max(0.0)
                } else {
                    menu_state.y
                };
                layers.push(super::view::translate::Translate::new(card_msg, (x, y)).into());
            }

            // v0.14 — typed-delta "Move Selection By X, Y…" modal.
            // Sits above everything else in this block (bar, dropdown,
            // context menu) — it's a blocking dialog once open. Gated
            // into `needs_overlay` above via `move_by_modal.is_some()`.
            if let Some(card) =
                crate::library::editor::footprint::move_by_modal::view_move_by_modal(editor, tokens)
            {
                let close_msg = Message::Library(
                    crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                        path: path.to_path_buf(),
                        msg: crate::library::messages::PrimitiveEdit::Footprint(
                            crate::library::messages::FootprintEditorMsg::MoveByCancel,
                        ),
                    },
                );
                layers.push(Self::dismiss_layer(close_msg));
                layers.push(
                    container(card.map(Message::Library))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .center_x(Length::Fill)
                        .center_y(Length::Fill)
                        .into(),
                );
            }
        }

        // v0.13 — symbol library editor active bar mounted at the
        // SAME app-view layer as the schematic / footprint bars.
        if let Some(active_tab) = self.document_state.tabs.get(self.document_state.active_tab)
            && let Some(path) = active_tab.kind.as_symbol_editor()
            && let Some(editor) = self.document_state.symbol_editors.get(path)
        {
            let y_offset: f32 = crate::menu_bar::MENU_BAR_HEIGHT
                + if document.tabs.is_empty() { 0.0 } else { 28.0 };
            let theme_id = self.ui_state.theme_id;
            let tokens = &document.panel_ctx.tokens;
            // Same byte-for-byte structure as the footprint + schematic
            // mounts. Direct call to `signex_widgets::active_bar::view`
            // — the unified widget's view_with_overlay path is
            // bypassed at this site so the chain matches schematic.
            let bar_items = crate::library::editor::symbol::active_bar::bar_items(editor, theme_id);
            let bar = signex_widgets::active_bar::view(bar_items, tokens).map(Message::Library);
            layers.push(
                column![
                    iced::widget::Space::new().height(y_offset + 4.0),
                    container(bar)
                        .width(Length::Fill)
                        .align_x(iced::alignment::Horizontal::Center),
                ]
                .into(),
            );
            let dropdown_top: u16 = (y_offset as u16).saturating_add(42);
            if let Some(overlay) = crate::library::editor::symbol::active_bar::dropdown_overlay(
                editor,
                theme_id,
                tokens,
                dropdown_top,
            ) {
                layers.push(overlay.map(Message::Library));
            }
        }

        if self.has_active_schematic()
            && let Some(ref edit_state) = interaction.editing_text
        {
            let text = edit_state.text.clone();
            // Convert object world position → window-absolute screen position.
            // The canvas Program publishes its latest camera into this Cell each
            // frame — that's the only way to read it from outside the Program.
            let (cam_off_x, cam_off_y, cam_scale) = interaction.canvas.live_camera.get();
            let canvas_local_x = edit_state.world_x as f32 * cam_scale + cam_off_x;
            let canvas_local_y = edit_state.world_y as f32 * cam_scale + cam_off_y;
            // Canvas top-left within the window: menu bar + tab bar above,
            // left dock + left resize handle (5px when shown) to the side.
            let tabs_h: f32 = if document.tabs.is_empty() { 0.0 } else { 28.0 };
            let y_canvas_origin: f32 = crate::menu_bar::MENU_BAR_HEIGHT + tabs_h;
            let has_left = document.dock.has_panels(PanelPosition::Left);
            let left_col = document.dock.is_collapsed(PanelPosition::Left);
            let left_dock_w: f32 = if !has_left {
                0.0
            } else if left_col {
                28.0
            } else {
                ui.left_width
            };
            let left_handle_w: f32 = if has_left && !left_col { 5.0 } else { 0.0 };
            let x_canvas_origin: f32 = left_dock_w + left_handle_w;
            // Font size in pixels matches the rendered label (10 pt ≈ 1.8 mm).
            let font_px = (cam_scale * 1.8).clamp(10.0, 64.0);
            // Estimate width from text length to keep the input snug.
            let approx_w =
                ((edit_state.text.chars().count() as f32 + 2.0) * font_px * 0.62).max(60.0);
            // Offset the input so the baseline sits on top of the label text.
            let abs_x = x_canvas_origin + canvas_local_x - 2.0;
            let abs_y = y_canvas_origin + canvas_local_y - font_px - 2.0;
            let paper_c = crate::styles::ti(document.panel_ctx.tokens.paper);
            let text_c = crate::styles::ti(document.panel_ctx.tokens.text);
            let accent_c = crate::styles::ti(document.panel_ctx.tokens.accent);
            layers.push(
                column![
                    iced::widget::Space::new().height(abs_y.max(0.0)),
                    row![
                        iced::widget::Space::new().width(abs_x.max(0.0)),
                        container(
                            iced::widget::text_input("", &text)
                                .on_input(|t| Message::TextEdit(TextEditMsg::Changed(t)))
                                .on_submit(Message::TextEdit(TextEditMsg::Submit))
                                .size(font_px)
                                .padding([1, 2])
                                .width(approx_w)
                                .style(move |_: &iced::Theme, _status: iced::widget::text_input::Status| {
                                    iced::widget::text_input::Style {
                                        background: iced::Background::Color(paper_c),
                                        border: iced::Border {
                                            color: accent_c,
                                            width: 1.0,
                                            radius: 0.0.into(),
                                        },
                                        icon: text_c,
                                        placeholder: text_c,
                                        value: text_c,
                                        selection: accent_c,
                                    }
                                }),
                        ),
                    ],
                ]
                .into(),
            );
        }

        if let Some(ab_menu) = interaction.active_bar_menu {
            let has_selection = !interaction.canvas.selected.is_empty();
            let has_net_colors = !ui.net_colors.is_empty();
            let dropdown = crate::active_bar::view_dropdown(
                ab_menu,
                &document.panel_ctx.tokens,
                &interaction.selection_filters,
                &interaction.custom_filter_presets,
                self.ui_state.theme_id,
                has_selection,
                has_net_colors,
            )
            .map(Message::ActiveBar);
            let x_off = crate::active_bar::dropdown_x_offset(ab_menu);
            // Bar: MENU_BAR_HEIGHT + tabs + 4 top-margin + bar-height ≈ bottom of bar.
            // Bar-height = 28 button + 6 vertical padding + 2 border = 36, plus 4
            // top margin = 40. Add a small gap so the dropdown visually touches.
            let ab_y: f32 = crate::menu_bar::MENU_BAR_HEIGHT
                + if document.tabs.is_empty() { 0.0 } else { 28.0 }
                + 40.0;
            let bar_w: f32 = crate::active_bar::BAR_WIDTH_PX;
            let (ww, _) = ui.window_size;
            let adjusted_x = x_off + (ww - bar_w) / 2.0;

            layers.push(Self::dismiss_layer(Message::ActiveBar(
                crate::active_bar::ActiveBarMsg::CloseMenus,
            )));
            // Absolute-position the dropdown with Translate so the
            // column can auto-size to its widest label. The old
            // column+row+Space wrapping forced a fixed-width column
            // which clipped labels like "Elliptical Arc".
            layers
                .push(super::view::translate::Translate::new(dropdown, (adjusted_x, ab_y)).into());
        }

        if let Some(ref ctx_menu) = interaction.context_menu {
            let menu = self.view_context_menu();
            // Clamp the menu inside the window so a click near the
            // right/bottom edge doesn't push it off-screen. Estimate
            // the menu's footprint conservatively from the maximum
            // possible row count (≈ 22 rows × 22 px + padding) and
            // CONTEXT_MENU_WIDTH; flip-up / flip-left when the click
            // lands too close to an edge.
            let (win_w, win_h) = self.ui_state.window_size;
            let menu_w = Self::CONTEXT_MENU_WIDTH as f32;
            let est_menu_h: f32 = 22.0 * 22.0 + 8.0;
            let edge_margin: f32 = 4.0;
            let x = if ctx_menu.x + menu_w + edge_margin > win_w {
                (win_w - menu_w - edge_margin).max(0.0)
            } else {
                ctx_menu.x
            };
            let y = if ctx_menu.y + est_menu_h + edge_margin > win_h {
                (ctx_menu.y - est_menu_h).max(0.0)
            } else {
                ctx_menu.y
            };
            layers.push(Self::dismiss_layer(Message::ContextMenu(
                ContextMenuMsg::Close,
            )));
            layers.push(
                column![
                    iced::widget::Space::new().height(y),
                    row![
                        iced::widget::Space::new().width(x),
                        menu,
                        iced::widget::Space::new().width(Length::Fill),
                    ]
                    .width(Length::Fill),
                ]
                .into(),
            );
            // Submenu (Place / Align) — pop to the right of the parent
            // menu (or left if the right edge would overflow), and
            // align its top to the launcher row's y-position so the
            // first submenu item sits next to the row that opened it.
            if let Some(submenu_kind) = interaction.context_submenu {
                let submenu = self.view_context_submenu(submenu_kind);
                // Wrap in mouse_area so on_enter/on_exit on the panel
                // can extend the close timer when the cursor crosses
                // from the launcher into the submenu and back.
                let submenu = iced::widget::mouse_area(submenu)
                    .on_enter(Message::ContextMenu(ContextMenuMsg::SubmenuEnterPanel))
                    .on_exit(Message::ContextMenu(ContextMenuMsg::SubmenuLeavePanel));
                let submenu_w = menu_w;
                let sub_x = if x + menu_w + submenu_w + edge_margin > win_w {
                    (x - submenu_w).max(0.0)
                } else {
                    x + menu_w
                };
                // Approximate launcher-row y inside the parent menu.
                // Each ctx_menu_item_* row is ≈ 22 px tall (text + 4 px
                // top + 4 px bottom + a tiny line-height fudge); the
                // separator is rendered as a 1 px line. The numbers
                // below come from counting rows above each launcher in
                // `view_context_menu`.
                const ROW_H: f32 = 22.0;
                const SEP_H: f32 = 1.0;
                const TOP_PAD: f32 = 4.0;
                let launcher_y = match submenu_kind {
                    // Above Place: 3 always-visible rows + 1 separator.
                    ContextSubmenu::Place => TOP_PAD + 3.0 * ROW_H + SEP_H,
                    // Align is only shown when something is selected;
                    // above Align: the same 3 rows + 1 sep, then
                    // Place / Part Actions / Sheet Actions / References.
                    ContextSubmenu::Align => TOP_PAD + 7.0 * ROW_H + SEP_H,
                    // AddNewToProject only fires from the project-tree
                    // menu, never from the canvas menu — fall through
                    // to a safe placeholder if the state somehow leaks
                    // (no submenu rendered, just a 0-offset).
                    ContextSubmenu::AddNewToProject => 0.0,
                };
                let sub_y = (y + launcher_y - 4.0).max(0.0);
                layers.push(
                    column![
                        iced::widget::Space::new().height(sub_y),
                        row![
                            iced::widget::Space::new().width(sub_x),
                            submenu,
                            iced::widget::Space::new().width(Length::Fill),
                        ]
                        .width(Length::Fill),
                    ]
                    .into(),
                );
            }
        }

        // Document-tab right-click menu. Rendered before the project-
        // tree menu since the two are mutually exclusive — only one of
        // them can be open at a time, and opening one closes the
        // others (see ContextMenuMsg::ShowTab).
        if let Some(ref tab_ctx) = interaction.tab_context_menu {
            let menu = self.view_tab_context_menu(tab_ctx);
            // Conservative footprint matches the project-tree menu so
            // the two visually align.
            let menu_w = Self::CONTEXT_MENU_WIDTH as f32;
            let est_menu_h: f32 = 5.0 * 22.0 + 8.0;
            let (win_w, win_h) = ui.window_size;
            let edge_margin: f32 = 4.0;
            let x = if tab_ctx.x + menu_w + edge_margin > win_w {
                (win_w - menu_w - edge_margin).max(0.0)
            } else {
                tab_ctx.x
            };
            let y = if tab_ctx.y + est_menu_h + edge_margin > win_h {
                (tab_ctx.y - est_menu_h).max(0.0)
            } else {
                tab_ctx.y
            };
            layers.push(Self::dismiss_layer(Message::ContextMenu(
                ContextMenuMsg::CloseTab,
            )));
            layers.push(
                column![
                    iced::widget::Space::new().height(y),
                    row![
                        iced::widget::Space::new().width(x),
                        menu,
                        iced::widget::Space::new().width(Length::Fill),
                    ]
                    .width(Length::Fill),
                ]
                .into(),
            );
        }

        // Projects-panel tree right-click menu. Rendered here (after the
        // canvas context menu) so the canvas menu's dismiss layer does
        // not cover this one — the two are mutually exclusive in
        // practice since `ContextMenuMsg::ShowProjectTree` nulls out
        // `context_menu` before opening.
        if let Some(ref tree_ctx) = interaction.project_tree_context_menu {
            let menu = self.view_project_tree_context_menu(tree_ctx);
            // Conservative footprint: at most 6 rows × 22 px + 8 px
            // padding. Width matches the canvas menu so the two look
            // consistent.
            let menu_w = Self::CONTEXT_MENU_WIDTH as f32;
            let est_menu_h: f32 = 6.0 * 22.0 + 8.0;
            let (win_w, win_h) = ui.window_size;
            let edge_margin: f32 = 4.0;
            let x = if tree_ctx.x + menu_w + edge_margin > win_w {
                (win_w - menu_w - edge_margin).max(0.0)
            } else {
                tree_ctx.x
            };
            let y = if tree_ctx.y + est_menu_h + edge_margin > win_h {
                (tree_ctx.y - est_menu_h).max(0.0)
            } else {
                tree_ctx.y
            };
            layers.push(Self::dismiss_layer(Message::ContextMenu(
                ContextMenuMsg::CloseProjectTree,
            )));
            layers.push(
                column![
                    iced::widget::Space::new().height(y),
                    row![
                        iced::widget::Space::new().width(x),
                        menu,
                        iced::widget::Space::new().width(Length::Fill),
                    ]
                    .width(Length::Fill),
                ]
                .into(),
            );
            // Adjacent submenu (currently only AddNewToProject opens
            // from this menu). Mirrors the canvas-menu submenu logic
            // above — pop to the right of the parent (or left if the
            // right edge would overflow), align top to the launcher
            // row's y inside the parent menu.
            if let Some(ContextSubmenu::AddNewToProject) = interaction.context_submenu {
                let submenu = self.view_context_submenu(ContextSubmenu::AddNewToProject);
                let submenu = iced::widget::mouse_area(submenu)
                    .on_enter(Message::ContextMenu(ContextMenuMsg::SubmenuEnterPanel))
                    .on_exit(Message::ContextMenu(ContextMenuMsg::SubmenuLeavePanel));
                let submenu_w = menu_w;
                let sub_x = if x + menu_w + submenu_w + edge_margin > win_w {
                    (x - submenu_w).max(0.0)
                } else {
                    x + menu_w
                };
                // Launcher position inside the project-tree menu:
                // `Make Project Available Online...` (row 0)
                // `Validate Project`                 (row 1)
                // `Add New to Project ›`             (row 2) ← target
                // → top + 2 rows, no separator above the launcher.
                const ROW_H: f32 = 22.0;
                const TOP_PAD: f32 = 4.0;
                let launcher_y = TOP_PAD + 2.0 * ROW_H;
                let sub_y = (y + launcher_y - 4.0).max(0.0);
                layers.push(
                    column![
                        iced::widget::Space::new().height(sub_y),
                        row![
                            iced::widget::Space::new().width(sub_x),
                            submenu,
                            iced::widget::Space::new().width(Length::Fill),
                        ]
                        .width(Length::Fill),
                    ]
                    .into(),
                );
            }
        }

        // v0.18.10 — Altium-style grid picker popup. Floats at the
        // cursor when `G` is pressed in a footprint editor; rows are
        // the standard 1mil…2.5mm ladder. Outside-click and Esc both
        // dismiss via `Message::Ui(UiMsg::GridPickerClose)`.
        if let Some(ref picker) = interaction.grid_picker {
            let menu = self.view_grid_picker_menu();
            let menu_w: f32 = 200.0;
            let est_menu_h: f32 = 13.0 * 22.0 + 8.0; // 13 rows + padding
            let (win_w, win_h) = ui.window_size;
            let edge_margin: f32 = 4.0;
            let x = if picker.x + menu_w + edge_margin > win_w {
                (win_w - menu_w - edge_margin).max(0.0)
            } else {
                picker.x
            };
            let y = if picker.y + est_menu_h + edge_margin > win_h {
                (picker.y - est_menu_h).max(0.0)
            } else {
                picker.y
            };
            layers.push(Self::dismiss_layer(Message::Ui(UiMsg::GridPickerClose)));
            layers.push(
                column![
                    iced::widget::Space::new().height(y),
                    row![
                        iced::widget::Space::new().width(x),
                        menu,
                        iced::widget::Space::new().width(Length::Fill),
                    ]
                    .width(Length::Fill),
                ]
                .into(),
            );
        }

        if ui.panel_list_open {
            let text_c = crate::styles::ti(document.panel_ctx.tokens.text);
            let text_muted = crate::styles::ti(document.panel_ctx.tokens.text_secondary);
            let has_sch = document.panel_ctx.has_schematic;
            let has_pcb = document.panel_ctx.has_pcb;
            // Build a lookup of currently-open panel kinds so each row
            // can show a ✓ mark. A panel counts as "open" if it lives in
            // any dock region, floats on top, or owns a detached OS
            // window.
            let docked: std::collections::HashSet<crate::panels::PanelKind> = [
                crate::dock::PanelPosition::Left,
                crate::dock::PanelPosition::Right,
                crate::dock::PanelPosition::Bottom,
            ]
            .iter()
            .flat_map(|pos| document.dock.panel_kinds(*pos).to_vec())
            .collect();
            let floating: std::collections::HashSet<crate::panels::PanelKind> =
                document.dock.floating.iter().map(|fp| fp.kind).collect();
            let detached: std::collections::HashSet<crate::panels::PanelKind> = ui
                .windows
                .values()
                .filter_map(|w| match w {
                    super::state::WindowKind::DetachedPanel(k) => Some(*k),
                    _ => None,
                })
                .collect();
            let is_open = |k: crate::panels::PanelKind| {
                docked.contains(&k) || floating.contains(&k) || detached.contains(&k)
            };
            let panel_items: Vec<Element<'_, Message>> = crate::panels::ALL_PANELS
                .iter()
                .filter(|&&kind| {
                    (!kind.needs_schematic() || has_sch) && (!kind.needs_pcb() || has_pcb)
                })
                .map(|&kind| {
                    // Altium parity: a leading ✓ column marks open panels
                    // so the user can see at a glance which ones are
                    // already somewhere on screen. Clicking an open panel
                    // still fires OpenPanel — the dock brings it forward.
                    let check = if is_open(kind) { "\u{2713}" } else { "" };
                    iced::widget::button(
                        iced::widget::row![
                            iced::widget::container(
                                iced::widget::text(check.to_string())
                                    .size(11)
                                    .color(text_muted),
                            )
                            .width(Length::Fixed(16.0)),
                            iced::widget::text(kind.label().to_string())
                                .size(11)
                                .color(text_c),
                        ]
                        .align_y(iced::Alignment::Center),
                    )
                    .padding([4, 12])
                    .width(Length::Fill)
                    .on_press(Message::Overlay(OverlayMsg::OpenPanel(kind)))
                    .style(crate::styles::menu_item(&document.panel_ctx.tokens))
                    .into()
                })
                .collect();

            // Drop the scrollable wrapper — the list fits the window at
            // full height (15-ish panels × 21 px each = ~315 px) and a
            // menu-style popup reads cleaner without a scrollbar.
            let popup = container(column(panel_items).spacing(0).width(210))
                .padding([6, 0])
                .style(crate::styles::context_menu(&document.panel_ctx.tokens));

            layers.push(Self::dismiss_layer(Message::Overlay(
                OverlayMsg::TogglePanelList,
            )));
            // Anchor the popup directly above the "Panels" button in the
            // bottom-right of the status bar. Approx: popup 210 px wide,
            // 22 px per row × visible rows + 12 px vertical padding.
            // Status bar sits at y = wh - 22, so we place the popup so
            // its bottom edge lands just above it.
            let (ww, wh) = ui.window_size;
            let visible_rows = crate::panels::ALL_PANELS
                .iter()
                .filter(|&&k| (!k.needs_schematic() || has_sch) && (!k.needs_pcb() || has_pcb))
                .count() as f32;
            let popup_w = 210.0_f32;
            let popup_h = visible_rows * 22.0 + 12.0;
            let left = (ww - popup_w - 10.0).max(0.0);
            let top = (wh - popup_h - 26.0).max(0.0);
            layers.push(translate::Translate::new(Element::from(popup), (left, top)).into());
        }

        if let Some(fp) = document.dock.floating.iter().find(|fp| fp.dragging) {
            let (ww, wh) = ui.window_size;
            let zone = 120.0;
            let cx = fp.x + fp.width / 2.0;
            let cy = fp.y + fp.height / 4.0;
            let zone_style = crate::styles::dock_zone_highlight(&document.panel_ctx.tokens);
            if cx < zone {
                layers.push(
                    container(iced::widget::Space::new())
                        .width(ui.left_width)
                        .height(Length::Fill)
                        .style(zone_style)
                        .into(),
                );
            } else if cx > ww - zone {
                layers.push(
                    row![
                        iced::widget::Space::new().width(Length::Fill),
                        container(iced::widget::Space::new())
                            .width(ui.right_width)
                            .height(Length::Fill)
                            .style(zone_style),
                    ]
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
                );
            } else if cy > wh - zone {
                layers.push(
                    column![
                        iced::widget::Space::new().height(Length::Fill),
                        container(iced::widget::Space::new())
                            .width(Length::Fill)
                            .height(ui.bottom_height)
                            .style(zone_style),
                    ]
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
                );
            }
        }

        for i in 0..document.dock.floating.len() {
            if let Some(panel_widget) =
                document
                    .dock
                    .view_floating_panel(i, &document.panel_ctx, &self.library)
            {
                let fp = &document.dock.floating[i];
                // No clamp — panels follow Altium behaviour and may be
                // dragged anywhere, even past the window edge. The OS clips
                // at the window boundary; within that, Translate renders
                // the panel at fp.(x, y) without resizing it.
                layers.push(
                    translate::Translate::new(panel_widget.map(Message::Dock), (fp.x, fp.y)).into(),
                );
            }
        }

        // Preferences renders inline only if it hasn't been detached
        // into its own OS window. Open-flow auto-detaches via
        // `handle_preferences_open_requested`, so this in-window path
        // is the fallback when the detach failed (e.g. window manager
        // refused to spawn a new window).
        let prefs_detached = ui.windows.values().any(|kind| {
            matches!(
                kind,
                super::state::WindowKind::DetachedModal(super::state::ModalId::Preferences)
            )
        });
        if ui.preferences_open && !prefs_detached {
            let pref_view = crate::preferences::view(
                ui.preferences_nav,
                ui.preferences_draft_theme,
                ui.theme_id,
                &ui.preferences_draft_font,
                ui.preferences_draft_power_port_style,
                ui.preferences_draft_label_style,
                ui.preferences_draft_multisheet_style,
                ui.preferences_draft_grid_style,
                ui.preferences_draft_symbol_grid_size_mm,
                ui.preferences_draft_symbol_grid_style,
                ui.custom_theme.as_ref().map(|c| c.name.as_str()),
                ui.preferences_dirty,
                &ui.erc_severity_override,
                &self.library.settings,
                &document.panel_ctx.tokens,
                &ui.preferences_draft_component_classes,
                &ui.preferences_keymap_editor,
                &ui.preferences_keymap_status,
                &ui.preferences_keymap_search,
                ui.preferences_keymap_recorder.as_ref(),
                ui.theme_id,
            )
            .map(|m| Message::Preferences(PreferencesMsg::Inner(m)));
            layers.push(pref_view);
        }

        if ui.find_replace.open {
            let dialog = crate::find_replace::view(&ui.find_replace, &document.panel_ctx.tokens)
                .map(Message::FindReplaceMsg);
            layers.push(dialog);
        }

        if ui.keyboard_shortcuts_open {
            layers.push(crate::keyboard_shortcuts_modal::view(
                &document.panel_ctx.tokens,
                ui.theme_id,
                &ui.keymap_profiles,
            ));
        }

        if ui.first_run_tour_open {
            layers.push(crate::first_run_tour::view(&document.panel_ctx.tokens));
        }

        if ui.rename_dialog.is_some() {
            layers.push(self.view_rename_dialog());
        }
        if ui.remove_dialog.is_some() {
            layers.push(self.view_remove_dialog());
        }
        if ui.project_close_confirm.is_some() {
            layers.push(self.view_project_close_confirm());
        }
        if ui.app_quit_confirm.is_some() {
            layers.push(self.view_app_quit_confirm());
        }
        if ui.project_options.is_some() {
            layers.push(self.view_project_options_dialog());
        }
        if ui.enable_version_control.is_some() {
            layers.push(self.view_enable_version_control_dialog());
        }
        if ui.grid_properties.is_some() {
            layers.push(self.view_grid_properties_dialog());
        }
        if ui.selection_filter_custom.is_some() {
            layers.push(self.view_selection_filter_custom_dialog());
        }

        // Skip overlay rendering for any modal whose detached OS window
        // owns the view. Without this guard the user sees the modal in
        // both the main window and the popped-out window at the same
        // time.
        let modal_detached = |m: super::state::ModalId| -> bool {
            ui.windows
                .values()
                .any(|kind| matches!(kind, super::state::WindowKind::DetachedModal(x) if *x == m))
        };

        if ui.annotate_dialog_open && !modal_detached(super::state::ModalId::AnnotateDialog) {
            layers.push(self.view_annotate_dialog());
        }
        if ui.annotate_reset_confirm && !modal_detached(super::state::ModalId::AnnotateResetConfirm)
        {
            layers.push(self.view_annotate_reset_confirm());
        }
        if ui.erc_dialog_open && !modal_detached(super::state::ModalId::ErcDialog) {
            layers.push(self.view_erc_dialog());
        }

        // v0.9 Library — picker modal overlay. Centered + dismiss-on-
        // ESC handled via the close X. Modal-detached path lands in
        // Phase 2 once Library overlays opt into the modal-id system.
        if let Some(picker) = self.library.picker.as_ref() {
            let card =
                crate::library::picker::view(&self.library, picker, &document.panel_ctx.tokens)
                    .map(Message::Library);
            // Wrap the centered card on a dim backdrop.
            let backdrop = container(card)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_: &iced::Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        0.0, 0.0, 0.0, 0.45,
                    ))),
                    ..Default::default()
                });
            layers.push(backdrop.into());
        }

        // New Component modal — same overlay shape as the picker.
        // Opened by File ▸ Library ▸ New Component… and from the
        // project tree's library-node right-click menu.
        if let Some(nc) = self.library.new_component.as_ref() {
            // Class registry is per-library: use the picked library's
            // manifest classes. Falls back to the user's prefs default
            // when no library is selected yet (the dropdown still
            // disables until a library is picked, but this keeps the
            // type signatures honest).
            let library_classes: Vec<crate::fonts::ComponentClassEntry> = nc
                .library_idx
                .and_then(|i| self.library.open_libraries.get(i))
                .and_then(|lib| self.library.set.get(lib.library_id))
                .map(|adapter| {
                    adapter
                        .library_classes()
                        .into_iter()
                        .map(|c| crate::fonts::ComponentClassEntry {
                            key: c.key,
                            label: c.label,
                        })
                        .collect()
                })
                .unwrap_or_default();
            let classes_to_show = if library_classes.is_empty() {
                self.ui_state.component_classes.clone()
            } else {
                library_classes
            };
            // v0.13 — New Component modal removed. "Add Component"
            // appends a draft row directly to the Library Browser's
            // inline-editable table; the user types the PN in the
            // table cell and picks the symbol / footprint via the
            // Properties panel that surfaces when the row is
            // selected. The unused `card` here is kept compiled to
            // keep the form's `view` function exercised — once the
            // dispatcher migration to "append-row-direct" lands
            // properly, the whole `library.new_component` state +
            // its messages can be pruned.
            let _ = (nc, classes_to_show);
        }

        // F25 (2026-05-03) — Edit Component Details modal removed.
        // Row click selects → Properties panel surfaces detail.
        // Per-component custom parameters are gone; every value
        // lives in a table column. Render branch retained behind
        // `EDIT_MODAL_ENABLED` for one release; prune the supporting
        // state + dispatchers in a follow-up cleanup pass.
        const EDIT_MODAL_ENABLED: bool = false;
        #[allow(clippy::overly_complex_bool_expr)]
        for (lib_path, browser_state) in &self.library.library_browsers {
            if EDIT_MODAL_ENABLED && let Some(edit) = browser_state.edit_modal.as_ref() {
                // Class registry is per-library — read from the
                // editing library's manifest. Falls back to the
                // user's prefs default when the library has no
                // classes registered yet (older libraries created
                // before the per-library registry shipped).
                let row_classes: Vec<crate::fonts::ComponentClassEntry> = self
                    .library
                    .library_at(lib_path)
                    .and_then(|lib| self.library.set.get(lib.library_id))
                    .map(|adapter| {
                        adapter
                            .library_classes()
                            .into_iter()
                            .map(|c| crate::fonts::ComponentClassEntry {
                                key: c.key,
                                label: c.label,
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let row_classes = if row_classes.is_empty() {
                    self.ui_state.component_classes.clone()
                } else {
                    row_classes
                };
                let card = crate::library::edit_row_modal::view(
                    lib_path.as_path(),
                    edit,
                    &document.panel_ctx.tokens,
                    row_classes,
                )
                .map(Message::Library);
                let backdrop = container(card)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .style(|_: &iced::Theme| iced::widget::container::Style {
                        background: Some(iced::Background::Color(iced::Color::from_rgba(
                            0.0, 0.0, 0.0, 0.45,
                        ))),
                        ..Default::default()
                    });
                layers.push(backdrop.into());
                break; // only one edit modal at a time
            }
        }

        // Delete Selected confirm modal (Deliverable D).
        for (lib_path, browser_state) in &self.library.library_browsers {
            if let Some(confirm) = browser_state.delete_confirm.as_ref() {
                let card = crate::library::edit_row_modal::view_delete_confirm(
                    lib_path.as_path(),
                    confirm,
                    &document.panel_ctx.tokens,
                )
                .map(Message::Library);
                let backdrop = container(card)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .style(|_: &iced::Theme| iced::widget::container::Style {
                        background: Some(iced::Background::Color(iced::Color::from_rgba(
                            0.0, 0.0, 0.0, 0.45,
                        ))),
                        ..Default::default()
                    });
                layers.push(backdrop.into());
                break;
            }
        }

        // Primitive picker (Pick Symbol / Pick Footprint).
        if let Some(picker) = self.library.primitive_picker.as_ref() {
            let card = crate::library::primitive_picker::view(
                &self.library,
                picker,
                &document.panel_ctx.tokens,
            )
            .map(Message::Library);
            let backdrop = container(card)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_: &iced::Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        0.0, 0.0, 0.0, 0.45,
                    ))),
                    ..Default::default()
                });
            layers.push(backdrop.into());
        }

        // Tools ▸ Document Options modal — Altium SchLib parity.
        if let Some(state) = self.library.document_options.as_ref() {
            let card = crate::library::document_options::view(state, &document.panel_ctx.tokens)
                .map(Message::Library);
            let backdrop = container(card)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_: &iced::Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        0.0, 0.0, 0.0, 0.45,
                    ))),
                    ..Default::default()
                });
            layers.push(backdrop.into());
        }

        // "Library Options" modal — Stage 11 of
        // `v0.9-snxlib-as-file-plan.md`. Pops between the New Library
        // Save-As dialog and the actual `LocalGitAdapter::init` so the
        // user can opt into Git LFS for binary 3D models before
        // anything hits disk.
        if let Some(state) = self.library.create_options.as_ref() {
            let card = crate::library::create_options::view(state, &document.panel_ctx.tokens)
                .map(Message::Library);
            let backdrop = container(card)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_: &iced::Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        0.0, 0.0, 0.0, 0.45,
                    ))),
                    ..Default::default()
                });
            layers.push(backdrop.into());
        }

        // Close-Library — Unsaved Drafts confirm modal.
        if let Some(confirm) = self.library.close_library_confirm.as_ref() {
            let card = crate::library::close_prompt::view(
                &self.library,
                confirm,
                &document.panel_ctx.tokens,
            )
            .map(Message::Library);
            let backdrop = container(card)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_: &iced::Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        0.0, 0.0, 0.0, 0.45,
                    ))),
                    ..Default::default()
                });
            layers.push(backdrop.into());
        }

        // Library recovery dialog (Stage 10). Surfaces missing-snxlib,
        // missing-.git, and broken primitive bindings as user-facing
        // modals instead of silent log lines.
        if let Some(dialog) = self.library.recovery.as_ref() {
            let card = crate::library::recovery::view(dialog, &document.panel_ctx.tokens)
                .map(Message::Library);
            let backdrop = container(card)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_: &iced::Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        0.0, 0.0, 0.0, 0.45,
                    ))),
                    ..Default::default()
                });
            layers.push(backdrop.into());
        }

        // Command palette dropdown (Ctrl+Shift+P). Rendered last so it
        // sits above every other modal layer; click-outside dismisses
        // via the standard dismiss_layer pattern.
        if ui.command_palette.open {
            layers.push(Self::dismiss_layer(Message::CommandPalette(
                CommandPaletteMsg::Close,
            )));
            layers.push(self.view_command_palette_dropdown());
        }

        // Hover tooltip — appears 250 ms after the cursor lands on a
        // placed symbol. Uses no `dismiss_layer` because the tooltip
        // is purely informational; click-through is desirable so a
        // visible card never blocks the user's next click on the
        // canvas. Symbol-only by design — wires/labels carry no
        // library metadata worth surfacing.
        if let Some(tooltip) = self.view_hover_tooltip() {
            layers.push(tooltip);
        }

        // "Library Updates Available" modal (Stage 16 §3.5).
        // Opened on schematic open under Team workflow mode when a
        // placed Symbol's `library_version` drifts from the source
        // row's current `ComponentRow.version`. The corresponding
        // `needs_overlay` predicate above must include
        // `library_updates.is_some()` — without it the click target
        // never paints (memory: needs_overlay-predicate-gates-modal).
        if let Some(state) = self.library.library_updates.as_ref() {
            let card = crate::library::updates_dialog::view(state, &document.panel_ctx.tokens)
                .map(Message::Library);
            let backdrop = container(card)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_: &iced::Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        0.0, 0.0, 0.0, 0.45,
                    ))),
                    ..Default::default()
                });
            layers.push(backdrop.into());
        }

        layers
    }
}
