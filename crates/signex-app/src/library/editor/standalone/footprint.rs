//! Standalone `.snxfpt` footprint-editor document tab view builders.
//! Split from `library/editor/standalone.rs` as pure code motion.

use std::path::Path;

use iced::widget::{Space, container, row};
use iced::{Border, Element, Length, Theme, Widget};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::{button, raw, text};

use crate::app::FootprintDocument;
use crate::library::editor::footprint::canvas::FootprintCanvas;
use crate::library::editor::footprint::layers::FpLayer;
use crate::library::messages::{EditorMsg, FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

// ── Footprint ───────────────────────────────────────────────────────

/// Render the standalone Footprint editor for a `.snxfpt` tab. Mirrors
/// the in-Component Editor footprint surface (canvas + floating chrome)
/// but skips the right-column Body 3D / 3D preview / STEP
/// attach panel — those edit Component-level fields that live on the
/// Footprint primitive's `body_3d` and `step_attachment` slots and the
/// view tree for them is reused via the Component Editor surface.
/// Pure pad-layout standalone editing is what `.snxfpt` needs first.
pub fn view_footprint<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    _theme_id: signex_types::theme::ThemeId,
    i18n: &signex_i18n::I18n,
) -> Element<'a, LibraryMessage> {
    use crate::library::editor::footprint::state::EditorMode;
    // v0.16.2.2 — footprint canvas uses Altium PCB-editor colours
    // regardless of the active app theme. Background pure black,
    // grid dark navy. Pads / silk / courtyard already render via
    // `FpLayer::color()` which carries Altium-flavoured tints (red
    // FCu / blue BCu / off-white silk / yellow Edge.Cuts), so this
    // change rounds out the Altium look on the canvas chrome itself.
    // Symbol (schematic) canvas stays theme-driven — Altium's
    // schematic editor uses a different (cream-ish) palette.
    // The paper / charcoal ground is painted instead.
    let graphite = crate::library::editor::footprint::canvas::graphite_look_for(tokens);
    let (bg, grid) = (graphite.bg, graphite.dot);

    // The canvas fills the tab: layer visibility lives in the Layers dock
    // panel, and the cursor and pad summary in the global status bar
    // (`footprint::status`), so nothing sits under the canvas (#2262).
    let canvas_area = view_footprint_canvas(editor, tokens, bg, grid);

    // Active bar floats OVER the canvas via Stack so the canvas
    // drawing area extends edge-to-edge behind it instead of being
    // clipped under the bar's bottom. Mirrors Fusion 360 / Altium.
    // v0.14.2 — mode switcher is its own floating widget at the
    // canvas's top-LEFT, separate from the centered active bar.
    let mode_switcher =
        crate::library::editor::footprint::pads_active_bar::mode_switcher_overlay(editor, tokens);
    // v0.18.11.1 — canvas-overlay multi-footprint tab strip removed.
    // The Footprint Library left-dock panel (v0.18.8) renders the
    // same list (internal `file.footprints[i]`) with proper
    // Place/Add/Delete/Edit buttons, so the redundant on-canvas
    // chip was just visual noise. Keep `mode_switcher` (Sketch /
    // Pads / 3D) at top-right since it has no panel equivalent.

    // v0.18.14 — single unified active bar carries mode-keyed
    // tools (left half) + the eight Selection Filter pills (right
    // half) regardless of mode. Replaces the per-mode
    // pads_active_bar::view / sketch_mode::active_bar::view
    // mounting that lived here through v0.18.13.
    // v0.13 — Active bar moved to the app-view layer so it shares
    // the schematic's window-absolute coordinate system. The body
    // here renders just canvas and its overlays; the bar
    // (and its dropdown overlay) is layered on top in
    // `view_main_for`, which is where the Selection Filter presets
    // are consumed — this view no longer takes them.
    let stack = iced::widget::Stack::new()
        .width(Length::Fill)
        .height(Length::Fill)
        .push(canvas_area);
    match editor.state.mode {
        EditorMode::Sketch | EditorMode::Normal => stack
            .push(super::dof_chip::overlay(editor, tokens, i18n))
            .push(super::dof_chip::zoom_pill_overlay(editor, tokens))
            .push(mode_switcher)
            .boxed(),
        EditorMode::View3d => stack.push(mode_switcher).boxed(),
    }
}

/// Minimal Sketch-mode top bar — replaces the layer-heavy
/// `view_footprint_toolbar` so the workspace reads as "you are now
/// editing the sketch, not the pads". Carries a "Sketch" title, an
/// **Exit Sketch** button (returns to Normal mode), and Save.
/// v0.14.2 — fixed top strip rendered above the active bar in every
/// mode. Hosts the mode segmented control + Save button. Stays the
/// same height across mode switches so the active bar's vertical
/// position is constant.
#[expect(
    dead_code,
    reason = "standalone footprint chrome superseded by the Active Bar; kept for one migration cycle"
)]
fn view_footprint_top_strip<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    use crate::library::editor::footprint::state::EditorMode;
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    let accent = theme_ext::to_color(&tokens.accent);

    // Segmented control — three connected segments. Active segment
    // paints with the accent background; inactive segments stay
    // muted with a hover affordance.
    let mode_segment =
        |label: &'static str, target: EditorMode, active: bool| -> Element<'a, LibraryMessage> {
            let path = editor.path.clone();
            let label_color = if active { iced::Color::WHITE } else { text_c };
            button(
                text(raw(label))
                    .size(TS.body)
                    .color(label_color)
                    .align_x(iced::alignment::Horizontal::Center),
            )
            .padding([SP.s, SP.xl])
            .on_press(LibraryMessage::PrimitiveEditorEvent {
                path,
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SetMode(target)),
            })
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: if active {
                    Some(iced::Background::Color(accent))
                } else {
                    Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.04,
                    )))
                },
                border: Border {
                    width: 1.0,
                    radius: crate::styles::RADIUS.into(),
                    color: if active { accent } else { border },
                    ..Border::default()
                },
                ..iced::widget::button::Style::default()
            })
            .boxed()
        };

    let mode = editor.state.mode;
    let mode_segments = row![
        text(raw("Mode")).size(TS.body).color(muted),
        Space::new().width(SP.m),
        mode_segment(
            "Pads",
            EditorMode::Normal,
            matches!(mode, EditorMode::Normal)
        ),
        mode_segment(
            "Sketch",
            EditorMode::Sketch,
            matches!(mode, EditorMode::Sketch)
        ),
        mode_segment("3D", EditorMode::View3d, matches!(mode, EditorMode::View3d)),
    ]
    .spacing(SP.xs)
    .align_y(iced::Alignment::Center);

    let save_path = editor.path.clone();
    let save_btn = button(
        text(raw(if editor.is_dirty() { "Save *" } else { "Save" }))
            .size(TS.body)
            .color(text_c),
    )
    .padding([SP.s, SP.l])
    .on_press(LibraryMessage::PrimitiveEditorEvent {
        path: save_path,
        msg: PrimitiveEdit::Save,
    })
    .style(move |_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            1.0, 1.0, 1.0, 0.04,
        ))),
        border: Border {
            width: 1.0,
            radius: crate::styles::RADIUS.into(),
            color: border,
            ..Border::default()
        },
        ..iced::widget::button::Style::default()
    });

    let auto_fit_path = editor.path.clone();
    let auto_fit_on = editor.state.auto_fit_courtyard;
    let auto_fit_label = if auto_fit_on {
        "Auto-fit Courtyard \u{2713}"
    } else {
        "Auto-fit Courtyard"
    };
    let auto_fit_btn = button(text(raw(auto_fit_label)).size(TS.body).color(text_c))
        .padding([SP.s, SP.l])
        .on_press(LibraryMessage::PrimitiveEditorEvent {
            path: auto_fit_path,
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ToggleAutoFit),
        })
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            border: Border {
                width: 1.0,
                radius: crate::styles::RADIUS.into(),
                color: border,
                ..Border::default()
            },
            ..iced::widget::button::Style::default()
        });

    // A copy of a stock footprint can be made again from the editor (#813).
    let copy_again: Element<'a, LibraryMessage> = if editor.file.copied_from.is_some() {
        crate::library::modal_chrome::secondary_btn(
            "Copy Again",
            LibraryMessage::StockCopy(
                crate::library::stock_copy::StockCopyMsg::CopyAgainRequested(editor.path.clone()),
            ),
            tokens,
        )
    } else {
        Space::new().boxed()
    };

    let row_widget = row![
        mode_segments,
        Space::new().width(Length::Fill),
        copy_again,
        auto_fit_btn,
        Space::new().width(SP.m),
        save_btn,
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);

    container(row_widget)
        .padding([SP.m, SP.l])
        .style(crate::styles::tab_bar_strip(tokens))
        .boxed()
}

#[expect(
    dead_code,
    reason = "standalone footprint chrome superseded by the Active Bar; kept for one migration cycle"
)]
fn view_footprint_sketch_toolbar<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    use crate::library::editor::footprint::state::EditorMode;
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    let accent = theme_ext::to_color(&tokens.accent);

    let exit_path = editor.path.clone();
    let exit_btn = button(
        row![
            text(raw("\u{2715}")).size(TS.body).color(text_c),
            Space::new().width(SP.s),
            text(raw("Exit Sketch")).size(TS.body).color(text_c),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([SP.s, SP.l])
    .on_press(LibraryMessage::PrimitiveEditorEvent {
        path: exit_path,
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SetMode(EditorMode::Normal)),
    })
    .style(move |_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            accent.r, accent.g, accent.b, 0.18,
        ))),
        border: Border {
            width: 1.0,
            radius: crate::styles::RADIUS.into(),
            color: iced::Color { a: 0.6, ..accent },
            ..Border::default()
        },
        ..iced::widget::button::Style::default()
    });

    let save_path = editor.path.clone();
    let save_btn = button(
        text(raw(if editor.is_dirty() { "Save *" } else { "Save" }))
            .size(TS.body)
            .color(text_c),
    )
    .padding([SP.s, SP.l])
    .on_press(LibraryMessage::PrimitiveEditorEvent {
        path: save_path,
        msg: PrimitiveEdit::Save,
    })
    .style(move |_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            1.0, 1.0, 1.0, 0.04,
        ))),
        border: Border {
            width: 1.0,
            radius: crate::styles::RADIUS.into(),
            color: border,
            ..Border::default()
        },
        ..iced::widget::button::Style::default()
    });

    let row_widget = row![
        text(raw("Sketch")).size(TS.title).color(text_c),
        text(raw("·")).size(TS.title).color(muted),
        text(raw("authoring parametric geometry"))
            .size(TS.body)
            .color(muted),
        Space::new().width(Length::Fill),
        save_btn,
        Space::new().width(SP.m),
        exit_btn,
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);

    container(row_widget)
        .padding([SP.m, SP.l])
        .style(crate::styles::tab_bar_strip(tokens))
        .boxed()
}

#[expect(
    dead_code,
    reason = "standalone footprint chrome superseded by the Active Bar; kept for one migration cycle"
)]
fn view_footprint_toolbar<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);
    let muted = theme_ext::text_secondary(tokens);

    let layers = editor.state.layer_visibility;
    let auto_fit_on = editor.state.auto_fit_courtyard;

    let mut row_widget = row![text(raw("Layers:")).size(TS.body).color(muted)]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center);

    for layer in FpLayer::ORDER {
        let on = layers.get(*layer);
        let swatch = layer.color();
        let label_color = if on { text_c } else { muted };
        let toggle_path = editor.path.clone();
        let layer_standard = layer.standard_name().to_string();
        let pill = button(
            row![
                container(text(raw("")).size(TS.body))
                    .width(Length::Fixed(SP.m))
                    .height(Length::Fixed(SP.m))
                    .style(move |_: &Theme| iced::widget::container::Style {
                        background: Some(iced::Background::Color(swatch)),
                        border: Border {
                            width: 1.0,
                            radius: 2.0.into(),
                            color: iced::Color { a: 0.5, ..swatch },
                            ..Border::default()
                        },
                        ..iced::widget::container::Style::default()
                    }),
                text(raw(layer.label())).size(TS.body).color(label_color),
            ]
            .spacing(SP.m)
            .align_y(iced::Alignment::Center),
        )
        .padding([SP.s, SP.m])
        .on_press(LibraryMessage::PrimitiveEditorEvent {
            path: toggle_path,
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ToggleLayer(layer_standard)),
        })
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: if on {
                Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.05,
                )))
            } else {
                Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.01,
                )))
            },
            border: Border {
                width: 1.0,
                radius: crate::styles::RADIUS.into(),
                color: if on {
                    swatch
                } else {
                    iced::Color { a: 0.5, ..border }
                },
                ..Border::default()
            },
            ..iced::widget::button::Style::default()
        });
        row_widget = row_widget.push(pill.boxed());
    }

    let auto_fit_path = editor.path.clone();
    let auto_fit_label = if auto_fit_on {
        "Auto-fit Courtyard \u{2713}"
    } else {
        "Auto-fit Courtyard"
    };
    let auto_fit_btn = button(text(raw(auto_fit_label)).size(TS.body).color(text_c))
        .padding([SP.s, SP.m])
        .on_press(LibraryMessage::PrimitiveEditorEvent {
            path: auto_fit_path,
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ToggleAutoFit),
        })
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.02,
            ))),
            border: Border {
                width: 1.0,
                radius: crate::styles::RADIUS.into(),
                color: border,
                ..Border::default()
            },
            ..iced::widget::button::Style::default()
        });

    let save_path = editor.path.clone();
    let save_btn = button(
        text(raw(if editor.is_dirty() { "Save *" } else { "Save" }))
            .size(TS.body)
            .color(text_c),
    )
    .padding([SP.s, SP.m])
    .on_press(LibraryMessage::PrimitiveEditorEvent {
        path: save_path,
        msg: PrimitiveEdit::Save,
    })
    .style(move |_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            1.0, 1.0, 1.0, 0.02,
        ))),
        border: Border {
            width: 1.0,
            radius: crate::styles::RADIUS.into(),
            color: border,
            ..Border::default()
        },
        ..iced::widget::button::Style::default()
    });

    // v0.13.1 — Sketch mode toggle. Three pills (Normal / Sketch /
    // 3D View) sit between the layer toggles and the auto-fit /
    // save buttons. The active pill is highlighted via the same
    // pattern the layer toggles use.
    let mode_pill = |label: &'static str,
                     target: crate::library::editor::footprint::state::EditorMode,
                     active: bool| {
        let path = editor.path.clone();
        let label_color = if active { text_c } else { muted };
        button(text(raw(label)).size(TS.body).color(label_color))
            .padding([SP.s, SP.m])
            .on_press(LibraryMessage::PrimitiveEditorEvent {
                path,
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SetMode(target)),
            })
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: if active {
                    Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.08,
                    )))
                } else {
                    Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.02,
                    )))
                },
                border: Border {
                    width: 1.0,
                    radius: crate::styles::RADIUS.into(),
                    color: border,
                    ..Border::default()
                },
                ..iced::widget::button::Style::default()
            })
    };
    let mode = editor.state.mode;
    let mode_row = row![
        text(raw("Mode:")).size(TS.body).color(muted),
        mode_pill(
            "Normal",
            crate::library::editor::footprint::state::EditorMode::Normal,
            matches!(
                mode,
                crate::library::editor::footprint::state::EditorMode::Normal
            ),
        ),
        mode_pill(
            "Sketch",
            crate::library::editor::footprint::state::EditorMode::Sketch,
            matches!(
                mode,
                crate::library::editor::footprint::state::EditorMode::Sketch
            ),
        ),
        mode_pill(
            "3D View",
            crate::library::editor::footprint::state::EditorMode::View3d,
            matches!(
                mode,
                crate::library::editor::footprint::state::EditorMode::View3d
            ),
        ),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);

    row_widget = row_widget
        .push(Space::new().width(Length::Fill).boxed())
        .push(mode_row.boxed())
        .push(Space::new().width(SP.m).boxed())
        .push(auto_fit_btn.boxed())
        .push(Space::new().width(SP.m).boxed())
        .push(save_btn.boxed());

    container(row_widget)
        .padding([SP.m, SP.l])
        .style(crate::styles::tab_bar_strip(tokens))
        .boxed()
}

pub(super) fn view_footprint_canvas<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    bg: iced::Color,
    grid: iced::Color,
) -> Element<'a, LibraryMessage> {
    let border = theme_ext::border_color(tokens);

    // The canvas program publishes `LibraryMessage::EditorEvent { ...,
    // msg: EditorMsg::Footprint* }`; we translate those into the
    // standalone primitive-editor envelope via `Element::map`. The
    // `EditorAddress` we stamp on the program is a sentinel — its
    // `library_path` field is the tab path (so dirty-tracking still
    // resolves), and the `table` / `row_id` are nil-shaped since
    // standalone tabs don't carry a Component.
    let address = crate::library::state::EditorAddress::new(
        editor.path.clone(),
        String::new(),
        signex_library::RowId::from_uuid(uuid::Uuid::nil()),
    );
    let prog = FootprintCanvas {
        state: &editor.state,
        address,
        bg_color: bg,
        grid_color: grid,
        cache: &editor.canvas_cache,
        sketch: editor.primitive().sketch.as_ref(),
        silk_f: editor.primitive().silk_f.as_slice(),
        silk_b: editor.primitive().silk_b.as_slice(),
        zoom_step: editor.zoom_step,
        trackpad_pan: editor.trackpad_pan,
        wheel_mode: editor.wheel_mode,
    };
    let canvas_widget: Element<'a, LibraryMessage> = iced::widget::canvas(prog)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed();
    let path = editor.path.clone();
    let translated = canvas_widget.map(move |msg| translate_footprint_canvas_msg(msg, &path));

    container(translated)
        .padding(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(bg)),
            border: Border {
                width: 1.0,
                radius: 0.0.into(),
                color: border,
                ..Border::default()
            },
            ..iced::widget::container::Style::default()
        })
        .boxed()
}

/// Translate a message emitted by the footprint canvas widget
/// (`canvas::Program<LibraryMessage>` — see `canvas/mod.rs`) into the
/// standalone `.snxfpt` tab's primitive-editor envelope. The canvas
/// only ever emits `EditorEvent { msg: EditorMsg::Footprint(_), .. }`
/// (every message it builds goes through that wrapper — see e.g.
/// `canvas/input/tools.rs`'s `select_msg`), so that shape becomes a
/// real, path-carrying `PrimitiveEditorEvent`. This routing decision
/// used to live inline in the `.map()` closure at the call site, where
/// nothing could unit-test it; it's a free function here so a
/// regression (a special-cased variant routed to `PrimitiveEdit::Save`
/// ahead of the general arm) is directly testable.
fn translate_footprint_canvas_msg(msg: LibraryMessage, path: &Path) -> LibraryMessage {
    match msg {
        LibraryMessage::EditorEvent {
            msg: EditorMsg::Footprint(msg),
            ..
        } => LibraryMessage::PrimitiveEditorEvent {
            path: path.to_path_buf(),
            msg: PrimitiveEdit::Footprint(msg),
        },
        other => {
            // Enforced, not just documented: a future non-Footprint
            // variant would otherwise pass through as a bare
            // `EditorEvent` carrying the sentinel address built in
            // `view_footprint_canvas` (empty `table`, nil `row_id`),
            // which `handle_editor_event`
            // (`app/dispatch/library/mod.rs`) hands to a path-keyed
            // lookup that cannot match a standalone tab — the message
            // would be silently dropped instead of erroring.
            debug_assert!(
                false,
                "footprint canvas emitted a non-Footprint message: {other:?}"
            );
            other
        }
    }
}

#[cfg(test)]
mod tests {
    //! Issue #180 — `translate_footprint_canvas_msg` is the canvas-to-
    //! dispatcher bridge and is private to this module, so it can only
    //! be exercised from an in-crate unit test (an integration test
    //! under `tests/` has no visibility into it: `standalone::footprint`
    //! is a private submodule re-exporting only `view_footprint`).
    use super::*;
    use std::path::PathBuf;

    fn wrap(msg: FootprintEditorMsg) -> LibraryMessage {
        LibraryMessage::EditorEvent {
            library_path: PathBuf::new(),
            table: String::new(),
            row_id: signex_library::RowId::from_uuid(uuid::Uuid::nil()),
            msg: EditorMsg::Footprint(msg),
        }
    }

    #[test]
    fn sketch_placement_tab_routes_to_footprint_not_save() {
        let path = PathBuf::from("t.snxfpt");
        let out = translate_footprint_canvas_msg(
            wrap(FootprintEditorMsg::SketchPlacementInputTab),
            &path,
        );
        match out {
            LibraryMessage::PrimitiveEditorEvent {
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchPlacementInputTab),
                ..
            } => {}
            other => panic!(
                "expected SketchPlacementInputTab to pass through to the footprint \
                 dispatcher (which cycles the placement-input field), got {other:?}"
            ),
        }
    }

    // Every other Footprint variant already passed through uniformly;
    // pin that down too so a future special-case regresses loudly.
    #[test]
    fn sketch_placement_char_routes_to_footprint_uniformly() {
        let path = PathBuf::from("t.snxfpt");
        let out = translate_footprint_canvas_msg(
            wrap(FootprintEditorMsg::SketchPlacementInputChar('5')),
            &path,
        );
        match out {
            LibraryMessage::PrimitiveEditorEvent {
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchPlacementInputChar('5')),
                ..
            } => {}
            other => panic!("expected uniform Footprint passthrough, got {other:?}"),
        }
    }
}
