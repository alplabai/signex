//! Pin Map tab — displays the symbol-pin → footprint-pad binding for
//! the active component, flags mismatches, and lets the user override
//! specific pin/pad pairings.
//!
//! Per `v0.9-library-refactor-plan.md` §12 (WS-G), this tab operates on
//! the new `Symbol` / `Footprint` primitives plus the `Revision`-level
//! `pin_map_overrides: Vec<PinPadOverride>` list. An empty override list
//! means "1:1 by `number` string equality". Each override entry redirects
//! one pin to a non-default pad number.
//!
//! The view contains:
//! - top status banner ("✓ N/N matched" / "⚠ K unmatched")
//! - toolbar (Auto-Match by Number, Auto-Match by Name, Clear Overrides)
//! - scrollable matches table with inline override editor
//! - bottom validation warnings panel (power-pin-on-small-pad, NC pin
//!   connected, output pin on mask-only pad)
//!
//! WS-G: Pin Map

use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Border, Element, Length, Theme};
use signex_library::{Footprint, Pad, PinElectricalType, PinPadOverride, Symbol, SymbolPin};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::{ComponentEditorState, EditorAddress, PinMapTabState};

/// Result of resolving one symbol pin against a footprint + the
/// override list.
#[derive(Clone, Debug, PartialEq)]
pub struct PinPadMatch {
    pub symbol_pin: SymbolPin,
    /// `None` when no pad — default or overridden — exists.
    pub footprint_pad: Option<Pad>,
    /// `Some(pad_number)` when the user overrode the default 1:1
    /// mapping for this pin.
    pub override_target: Option<String>,
    pub status: MatchStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchStatus {
    /// Pin matched its default pad by number equality.
    Matched,
    /// Pin had no pad — neither default nor overridden — to bind to.
    Unmatched,
    /// Pin was explicitly redirected to a different pad number.
    Overridden,
}

/// Pin/pad bind algorithm: for each pin, find the override target if
/// present otherwise the pin number itself, then look up that pad on
/// the footprint. Pads with no symbol pin (e.g. mounting holes, EP)
/// don't appear in the result list — ERC reads the matches list, so
/// extra pads are out-of-scope for this tab.
pub fn compute_matches(
    sym: &Symbol,
    fp: &Footprint,
    overrides: &[PinPadOverride],
) -> Vec<PinPadMatch> {
    sym.pins
        .iter()
        .map(|pin| {
            let override_target = overrides
                .iter()
                .find(|o| o.symbol_pin_number == pin.number)
                .map(|o| o.footprint_pad_number.clone());
            let resolved = override_target
                .as_deref()
                .unwrap_or(pin.number.as_str());
            let footprint_pad = fp
                .pads
                .iter()
                .find(|p| p.number == resolved)
                .cloned();
            let status = match (footprint_pad.as_ref(), override_target.as_ref()) {
                (Some(_), Some(_)) => MatchStatus::Overridden,
                (Some(_), None) => MatchStatus::Matched,
                (None, _) => MatchStatus::Unmatched,
            };
            PinPadMatch {
                symbol_pin: pin.clone(),
                footprint_pad,
                override_target,
                status,
            }
        })
        .collect()
}

/// Validation message — surfaced in the bottom warnings panel.
#[derive(Clone, Debug, PartialEq)]
pub struct PinMapWarning {
    pub severity: WarningSeverity,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarningSeverity {
    /// Soft hint — non-blocking.
    Warn,
    /// Hard violation — blocks Commit in v1.x ERC.
    Error,
}

/// True when at least one of `pad.layers` looks like a copper layer.
/// signex-library's `LayerId` is a string-typed newtype that mirrors
/// Standard layer naming (`F.Cu`, `B.Cu`, `In1.Cu`, …); we use a suffix
/// match rather than a hard enum so user-defined inner-copper layers
/// flow through correctly.
fn pad_has_copper(pad: &Pad) -> bool {
    pad.layers.iter().any(|l| {
        let s = l.as_str();
        s.ends_with(".Cu") || s.eq_ignore_ascii_case("Cu")
    })
}

/// Run the v0.9 validation rules over the resolved match list. Three
/// rules per §12:
/// - Power pin on a pad smaller than 0.8 mm (likely under-spec for ≥1A)
/// - NC pin connected to a pad
/// - Output pin on a mask-only pad (pad has no copper layer)
pub fn validate(matches: &[PinPadMatch]) -> Vec<PinMapWarning> {
    const SMALL_PAD_THRESHOLD_MM: f64 = 0.8;
    let mut out = Vec::new();
    for m in matches {
        let pad = match m.footprint_pad.as_ref() {
            Some(p) => p,
            None => continue,
        };
        let pin_label = if m.symbol_pin.name.is_empty() {
            m.symbol_pin.number.clone()
        } else {
            format!("{} ({})", m.symbol_pin.name, m.symbol_pin.number)
        };
        match m.symbol_pin.electrical {
            PinElectricalType::Power => {
                if pad.size[0] < SMALL_PAD_THRESHOLD_MM
                    || pad.size[1] < SMALL_PAD_THRESHOLD_MM
                {
                    out.push(PinMapWarning {
                        severity: WarningSeverity::Warn,
                        message: format!(
                            "{} routed to {:.2}×{:.2}mm pad — under {:.1}mm rec for ≥1A",
                            pin_label,
                            pad.size[0],
                            pad.size[1],
                            SMALL_PAD_THRESHOLD_MM,
                        ),
                    });
                }
            }
            PinElectricalType::NotConnected => {
                out.push(PinMapWarning {
                    severity: WarningSeverity::Warn,
                    message: format!(
                        "{} marked NC but is connected to pad {}",
                        pin_label, pad.number
                    ),
                });
            }
            PinElectricalType::Output => {
                if !pad_has_copper(pad) {
                    out.push(PinMapWarning {
                        severity: WarningSeverity::Error,
                        message: format!(
                            "{} drives mask-only pad {} (no copper layer)",
                            pin_label, pad.number
                        ),
                    });
                }
            }
            _ => {}
        }
    }
    out
}

/// View entry-point — renders the Pin Map tab content. Follows the same
/// pattern as the other editor tabs: `&ComponentEditorState` for
/// per-window data, theme tokens for styling, `window_id` so the
/// emitted messages flow through the multi-window dispatcher.
///
/// Symbol/footprint resolution against the binding `symbol_ref` /
/// `footprint_ref` pair is the responsibility of the caller (WS-E will
/// thread the loaded primitives through the editor state). For the
/// Phase 1 wiring we accept the optional `(Symbol, Footprint)` pair as
/// inputs and degrade to an info card when either side is missing.
pub fn view<'a>(
    editor: &'a ComponentEditorState,
    primitives: Option<(&'a Symbol, &'a Footprint)>,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let (sym, fp) = match primitives {
        Some(p) => p,
        None => {
            let card = container(
                column![
                    text("Pin Map").size(14).color(text_c),
                    Space::new().height(8),
                    text("Symbol or footprint primitive not yet loaded.")
                        .size(11)
                        .color(muted),
                    text("(WS-E will thread Symbol + Footprint through the editor state.)")
                        .size(11)
                        .color(muted),
                ]
                .spacing(2),
            )
            .padding(14)
            .style(crate::styles::modal_card(tokens));
            return card.into();
        }
    };

    // Build the per-frame match list and validation list. Both are
    // cheap (linear scans over pin lists) and avoid threading the
    // owned data through the iced view tree as references — every
    // child element gets owned `String`s where needed.
    let matches = compute_matches(sym, fp, &editor.draft.pin_map_overrides);
    let warnings = validate(&matches);

    let banner = status_banner(&matches, tokens);
    let bar = toolbar(tokens, &address);
    let table_inner = matches_table(matches, &editor.pin_map, tokens, &address);
    let warnings_panel = validation_panel(&warnings, tokens);

    container(
        column![
            banner,
            Space::new().height(6),
            bar,
            Space::new().height(8),
            container(scrollable(table_inner).height(Length::Fill))
                .style(move |_: &Theme| iced::widget::container::Style {
                    border: Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: border,
                    },
                    ..Default::default()
                })
                .padding(4)
                .height(Length::Fill),
            Space::new().height(6),
            warnings_panel,
        ]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .into()
}

fn status_banner<'a>(
    matches: &[PinPadMatch],
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let total = matches.len();
    let unmatched = matches
        .iter()
        .filter(|m| m.status == MatchStatus::Unmatched)
        .count();
    let matched = total - unmatched;
    let (label, fill) = if unmatched == 0 {
        (
            format!("\u{2713} {matched}/{total} matched"),
            iced::Color::from_rgba(0.20, 0.55, 0.30, 0.18),
        )
    } else {
        (
            format!("\u{26A0} {unmatched} unmatched"),
            iced::Color::from_rgba(0.78, 0.55, 0.10, 0.20),
        )
    };
    let text_c = theme_ext::text_primary(tokens);
    container(text(label).size(12).color(text_c))
        .padding([6, 12])
        .width(Length::Fill)
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(fill)),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: theme_ext::border_color(tokens),
            },
            ..Default::default()
        })
        .into()
}

fn toolbar<'a>(
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let btn = |label: &'static str, msg: EditorMsg| {
        let text_c = theme_ext::text_primary(tokens);
        let border = theme_ext::border_color(tokens);
        button(container(text(label).size(11).color(text_c)).padding([4, 12]))
            .on_press(LibraryMessage::EditorEvent {
                library_path: address.library_path.clone(),
                component_id: address.component_id,
                msg,
            })
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let bg = match status {
                    iced::widget::button::Status::Hovered => Some(iced::Background::Color(
                        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.08),
                    )),
                    _ => Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.04,
                    ))),
                };
                iced::widget::button::Style {
                    background: bg,
                    text_color: text_c,
                    border: Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: border,
                    },
                    ..iced::widget::button::Style::default()
                }
            })
    };
    row![
        btn("Auto-Match by Number", EditorMsg::PinMapAutoMatchByNumber),
        Space::new().width(6),
        btn("Auto-Match by Name", EditorMsg::PinMapAutoMatchByName),
        Space::new().width(6),
        btn("Clear Overrides", EditorMsg::PinMapClearOverrides),
    ]
    .align_y(iced::Alignment::Center)
    .into()
}

fn matches_table<'a>(
    matches: Vec<PinPadMatch>,
    pin_map: &'a PinMapTabState,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);

    let header = row![
        text("Pin#")
            .size(10)
            .color(muted)
            .width(Length::Fixed(48.0)),
        text("Name")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(2)),
        text("Type")
            .size(10)
            .color(muted)
            .width(Length::Fixed(96.0)),
        text("\u{2192}").size(10).color(muted).width(Length::Fixed(20.0)),
        text("Pad#")
            .size(10)
            .color(muted)
            .width(Length::Fixed(56.0)),
        text("Pad Size (mm)")
            .size(10)
            .color(muted)
            .width(Length::Fixed(110.0)),
        text("Status")
            .size(10)
            .color(muted)
            .width(Length::Fixed(96.0)),
        text("")
            .size(10)
            .width(Length::FillPortion(2)),
    ]
    .padding([4, 8]);

    let mut col = column![header].spacing(2);
    for m in matches {
        col = col.push(match_row(m, pin_map, tokens, address, text_c, muted));
    }
    col.into()
}

fn match_row<'a>(
    m: PinPadMatch,
    pin_map: &'a PinMapTabState,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
    text_c: iced::Color,
    muted: iced::Color,
) -> Element<'a, LibraryMessage> {
    let pin = m.symbol_pin;
    let footprint_pad = m.footprint_pad;
    let status = m.status;
    let (status_glyph, status_color) = match status {
        MatchStatus::Matched => ("\u{2713}", iced::Color::from_rgb(0.40, 0.78, 0.40)),
        MatchStatus::Overridden => ("\u{21AA}", iced::Color::from_rgb(0.40, 0.65, 0.95)),
        MatchStatus::Unmatched => ("\u{2717}", iced::Color::from_rgb(0.95, 0.40, 0.35)),
    };
    let pad_number = footprint_pad
        .as_ref()
        .map(|p| p.number.clone())
        .unwrap_or_else(|| "—".to_string());
    let pad_size = footprint_pad
        .as_ref()
        .map(|p| format!("{:.2}×{:.2}", p.size[0], p.size[1]))
        .unwrap_or_else(|| "—".to_string());

    let type_label = electrical_label(pin.electrical);

    let expanded = pin_map.expanded_row.as_deref() == Some(pin.number.as_str());

    let inline_action: Element<'_, LibraryMessage> = if expanded {
        // Inline editor — text_input + Save / Cancel.
        let buf = pin_map.override_buf.clone();
        let pin_number_for_save = pin.number.clone();
        let save = button(
            container(text("Save").size(10).color(iced::Color::WHITE)).padding([3, 10]),
        )
        .on_press(LibraryMessage::EditorEvent {
            library_path: address.library_path.clone(),
            component_id: address.component_id,
            msg: EditorMsg::PinMapAddOverride {
                pin: pin_number_for_save,
                pad: pin_map.override_buf.clone(),
            },
        })
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgb(
                0.00, 0.47, 0.84,
            ))),
            text_color: iced::Color::WHITE,
            border: Border {
                width: 0.0,
                radius: 3.0.into(),
                ..Border::default()
            },
            ..iced::widget::button::Style::default()
        });
        let cancel = button(
            container(text("Cancel").size(10).color(text_c)).padding([3, 10]),
        )
        .on_press(LibraryMessage::EditorEvent {
            library_path: address.library_path.clone(),
            component_id: address.component_id,
            msg: EditorMsg::PinMapCancelOverrideEdit,
        })
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            text_color: text_c,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: theme_ext::border_color(tokens),
            },
            ..iced::widget::button::Style::default()
        });
        let pin_for_input = pin.number.clone();
        let lib_path_for_input = address.library_path.clone();
        let component_id_for_input = address.component_id;
        let input = text_input("pad #", &buf)
            .padding([3, 6])
            .size(11)
            .width(Length::Fixed(80.0))
            .on_input(move |s| LibraryMessage::EditorEvent {
                library_path: lib_path_for_input.clone(),
                component_id: component_id_for_input,
                msg: EditorMsg::PinMapOverrideBufChanged {
                    pin: pin_for_input.clone(),
                    value: s,
                },
            });
        row![input, Space::new().width(4), save, Space::new().width(4), cancel]
            .align_y(iced::Alignment::Center)
            .into()
    } else if status == MatchStatus::Unmatched
        || status == MatchStatus::Overridden
    {
        // Compact "[override]" trigger — also offered on overridden
        // rows so the user can re-target.
        let pin_number_for_open = pin.number.clone();
        let label = if status == MatchStatus::Overridden {
            "Re-override"
        } else {
            "Override"
        };
        let trigger = button(container(text(label).size(10).color(text_c)).padding([3, 10]))
            .on_press(LibraryMessage::EditorEvent {
                library_path: address.library_path.clone(),
                component_id: address.component_id,
                msg: EditorMsg::PinMapOpenOverrideEdit(pin_number_for_open),
            })
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.04,
                ))),
                text_color: text_c,
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: theme_ext::border_color(tokens),
                },
                ..iced::widget::button::Style::default()
            });
        if status == MatchStatus::Overridden {
            let pin_number_for_remove = pin.number.clone();
            let remove = button(
                container(text("Remove").size(10).color(text_c)).padding([3, 10]),
            )
            .on_press(LibraryMessage::EditorEvent {
                library_path: address.library_path.clone(),
                component_id: address.component_id,
                msg: EditorMsg::PinMapRemoveOverride {
                    pin: pin_number_for_remove,
                },
            })
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.04,
                ))),
                text_color: text_c,
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: theme_ext::border_color(tokens),
                },
                ..iced::widget::button::Style::default()
            });
            row![trigger, Space::new().width(4), remove]
                .align_y(iced::Alignment::Center)
                .into()
        } else {
            trigger.into()
        }
    } else {
        // Matched rows have no action.
        Element::from(text("").size(10).color(muted))
    };

    container(
        row![
            text(pin.number.clone())
                .size(11)
                .color(text_c)
                .width(Length::Fixed(48.0)),
            text(pin.name.clone())
                .size(11)
                .color(text_c)
                .width(Length::FillPortion(2)),
            text(type_label)
                .size(11)
                .color(muted)
                .width(Length::Fixed(96.0)),
            text("\u{2192}").size(11).color(muted).width(Length::Fixed(20.0)),
            text(pad_number)
                .size(11)
                .color(text_c)
                .width(Length::Fixed(56.0)),
            text(pad_size)
                .size(11)
                .color(muted)
                .width(Length::Fixed(110.0)),
            text(status_glyph)
                .size(13)
                .color(status_color)
                .width(Length::Fixed(96.0)),
            container(inline_action).width(Length::FillPortion(2)),
        ]
        .align_y(iced::Alignment::Center)
        .padding([3, 8]),
    )
    .style(move |_: &Theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            1.0, 1.0, 1.0, 0.02,
        ))),
        border: Border {
            width: 1.0,
            radius: 2.0.into(),
            color: iced::Color::from_rgba(1.0, 1.0, 1.0, 0.06),
        },
        ..Default::default()
    })
    .into()
}

fn validation_panel<'a>(
    warnings: &[PinMapWarning],
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);
    if warnings.is_empty() {
        return container(
            text("No validation issues.").size(10).color(muted),
        )
        .padding([4, 8])
        .into();
    }
    let mut col = column![text(format!("Validation ({})", warnings.len()))
        .size(11)
        .color(text_c)]
    .spacing(2);
    for w in warnings {
        let (glyph, color) = match w.severity {
            WarningSeverity::Warn => (
                "\u{26A0}",
                iced::Color::from_rgb(0.85, 0.65, 0.20),
            ),
            WarningSeverity::Error => (
                "\u{2716}",
                iced::Color::from_rgb(0.95, 0.40, 0.35),
            ),
        };
        col = col.push(
            row![
                text(glyph).size(11).color(color),
                Space::new().width(6),
                text(w.message.clone()).size(11).color(text_c),
            ]
            .align_y(iced::Alignment::Center),
        );
    }
    container(col)
        .padding(8)
        .width(Length::Fill)
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.02,
            ))),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..Default::default()
        })
        .into()
}

fn electrical_label(t: PinElectricalType) -> &'static str {
    match t {
        PinElectricalType::Input => "Input",
        PinElectricalType::Output => "Output",
        PinElectricalType::Bidirectional => "Bidir",
        PinElectricalType::Power => "Power",
        PinElectricalType::Passive => "Passive",
        PinElectricalType::OpenCollector => "OpenColl",
        PinElectricalType::OpenEmitter => "OpenEmit",
        PinElectricalType::NotConnected => "NC",
        PinElectricalType::Tristate => "Tri-state",
        PinElectricalType::Unspecified => "—",
        // PinElectricalType is #[non_exhaustive] — guard against
        // future variants by labelling them as unknown rather than
        // failing to compile.
        _ => "?",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use signex_library::{
        Footprint, LayerId, Pad, PadKind, PadShape, PinElectricalType, PinOrientation, Polygon,
        Symbol, SymbolPin,
    };
    use uuid::Uuid;

    fn fixture_symbol(pins: Vec<(&str, &str, PinElectricalType)>) -> Symbol {
        let now = Utc::now();
        Symbol {
            uuid: Uuid::now_v7(),
            name: "TEST_SYM".into(),
            anchor: [0.0, 0.0],
            pins: pins
                .into_iter()
                .map(|(num, name, kind)| SymbolPin {
                    number: num.into(),
                    name: name.into(),
                    electrical: kind,
                    position: [0.0, 0.0],
                    orientation: PinOrientation::Right,
                    length: 2.54,
                })
                .collect(),
            graphics: Vec::new(),
            schematic_params: Default::default(),
            created: now,
            updated: now,
        }
    }

    fn fixture_pad(number: &str, size: [f64; 2], layers: Vec<LayerId>) -> Pad {
        Pad {
            number: number.into(),
            kind: PadKind::Smd,
            shape: PadShape::Rect,
            size,
            position: [0.0, 0.0],
            rotation: 0.0,
            layers,
            drill: None,
            solder_mask_margin: None,
            paste_margin: None,
        }
    }

    fn fixture_footprint(pads: Vec<Pad>) -> Footprint {
        let now = Utc::now();
        Footprint {
            uuid: Uuid::now_v7(),
            name: "TEST_FP".into(),
            anchor: [0.0, 0.0],
            pads,
            courtyard: Polygon::default(),
            silk_f: Vec::new(),
            silk_b: Vec::new(),
            fab_f: Vec::new(),
            fab_b: Vec::new(),
            body_3d: Default::default(),
            step_attachment: None,
            pcb_params: Default::default(),
            created: now,
            updated: now,
        }
    }

    /// Identity case: pins "1".."3" all match pads of the same numbers
    /// with no overrides.
    #[test]
    fn compute_matches_identity_all_matched() {
        let sym = fixture_symbol(vec![
            ("1", "IN", PinElectricalType::Input),
            ("2", "OUT", PinElectricalType::Output),
            ("3", "GND", PinElectricalType::Power),
        ]);
        let fp = fixture_footprint(vec![
            fixture_pad("1", [1.2, 1.2], vec![LayerId::new("F.Cu"), LayerId::new("F.Mask")]),
            fixture_pad("2", [1.2, 1.2], vec![LayerId::new("F.Cu"), LayerId::new("F.Mask")]),
            fixture_pad("3", [1.2, 1.2], vec![LayerId::new("F.Cu"), LayerId::new("F.Mask")]),
        ]);
        let result = compute_matches(&sym, &fp, &[]);
        assert_eq!(result.len(), 3);
        for m in &result {
            assert_eq!(m.status, MatchStatus::Matched);
            assert!(m.footprint_pad.is_some());
            assert!(m.override_target.is_none());
        }
    }

    /// Override case: pin "9" redirected to pad "EP" — status should be
    /// `Overridden` and the resolved pad must be the EP pad.
    #[test]
    fn compute_matches_override_redirects_to_ep() {
        let sym = fixture_symbol(vec![
            ("1", "VCC", PinElectricalType::Power),
            ("9", "PAD", PinElectricalType::Passive),
        ]);
        let fp = fixture_footprint(vec![
            fixture_pad("1", [1.0, 1.0], vec![LayerId::new("F.Cu"), LayerId::new("F.Mask")]),
            fixture_pad("EP", [3.0, 3.0], vec![LayerId::new("F.Cu"), LayerId::new("F.Mask")]),
        ]);
        let overrides = vec![PinPadOverride::new("9", "EP")];
        let result = compute_matches(&sym, &fp, &overrides);
        assert_eq!(result.len(), 2);
        // Pin 1 — default match.
        assert_eq!(result[0].status, MatchStatus::Matched);
        assert_eq!(result[0].override_target, None);
        // Pin 9 — overridden to EP.
        assert_eq!(result[1].status, MatchStatus::Overridden);
        assert_eq!(
            result[1].override_target.as_deref(),
            Some("EP"),
            "override target should be the redirected pad number"
        );
        assert_eq!(
            result[1].footprint_pad.as_ref().map(|p| p.number.as_str()),
            Some("EP")
        );
    }

    /// Missing pad case: symbol has pin "9" but footprint has no matching
    /// pad and no override is set — status must be `Unmatched`.
    #[test]
    fn compute_matches_missing_pad_yields_unmatched() {
        let sym = fixture_symbol(vec![
            ("1", "VCC", PinElectricalType::Power),
            ("9", "EP", PinElectricalType::Passive),
        ]);
        let fp = fixture_footprint(vec![fixture_pad(
            "1",
            [1.0, 1.0],
            vec![LayerId::new("F.Cu"), LayerId::new("F.Mask")],
        )]);
        let result = compute_matches(&sym, &fp, &[]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].status, MatchStatus::Matched);
        assert_eq!(result[1].status, MatchStatus::Unmatched);
        assert!(result[1].footprint_pad.is_none());
        assert!(result[1].override_target.is_none());
    }

    /// Extra-pad case: footprint has pad "EP" with no symbol pin — the
    /// extra pad simply doesn't appear in the matches list. ERC reads
    /// the matches list, so unbound pads (mounting holes, EPs without
    /// thermal pin) are out-of-scope for the Pin Map view.
    #[test]
    fn compute_matches_extra_pad_not_listed() {
        let sym = fixture_symbol(vec![("1", "IN", PinElectricalType::Input)]);
        let fp = fixture_footprint(vec![
            fixture_pad("1", [1.0, 1.0], vec![LayerId::new("F.Cu"), LayerId::new("F.Mask")]),
            fixture_pad("EP", [2.0, 2.0], vec![LayerId::new("F.Cu"), LayerId::new("F.Mask")]),
        ]);
        let result = compute_matches(&sym, &fp, &[]);
        assert_eq!(result.len(), 1, "only symbol pins drive matches");
        assert_eq!(result[0].symbol_pin.number, "1");
        assert!(
            !result.iter().any(|m| m
                .footprint_pad
                .as_ref()
                .is_some_and(|p| p.number == "EP")),
            "extra pad must not appear in matches"
        );
    }

    /// Override pointing at a non-existent pad still classifies as
    /// `Unmatched` (not `Overridden`) — guard against the user typing
    /// a stale pad number after the footprint changes.
    #[test]
    fn compute_matches_override_to_unknown_pad_is_unmatched() {
        let sym = fixture_symbol(vec![("9", "PAD", PinElectricalType::Passive)]);
        let fp = fixture_footprint(vec![fixture_pad(
            "1",
            [1.0, 1.0],
            vec![LayerId::new("F.Cu"), LayerId::new("F.Mask")],
        )]);
        let overrides = vec![PinPadOverride::new("9", "DOES_NOT_EXIST")];
        let result = compute_matches(&sym, &fp, &overrides);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].status, MatchStatus::Unmatched);
        assert!(result[0].footprint_pad.is_none());
    }

    /// Validation: power pin on an under-spec pad warns.
    #[test]
    fn validate_warns_power_pin_on_small_pad() {
        let sym = fixture_symbol(vec![("1", "VCC", PinElectricalType::Power)]);
        let fp = fixture_footprint(vec![fixture_pad(
            "1",
            [0.6, 0.6],
            vec![LayerId::new("F.Cu"), LayerId::new("F.Mask")],
        )]);
        let matches = compute_matches(&sym, &fp, &[]);
        let warnings = validate(&matches);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].severity, WarningSeverity::Warn);
        assert!(warnings[0].message.contains("VCC"));
    }

    /// Validation: NC pin connected to a pad warns.
    #[test]
    fn validate_warns_nc_pin_connected() {
        let sym = fixture_symbol(vec![("4", "NC", PinElectricalType::NotConnected)]);
        let fp = fixture_footprint(vec![fixture_pad(
            "4",
            [1.0, 1.0],
            vec![LayerId::new("F.Cu"), LayerId::new("F.Mask")],
        )]);
        let matches = compute_matches(&sym, &fp, &[]);
        let warnings = validate(&matches);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].severity, WarningSeverity::Warn);
        assert!(warnings[0].message.contains("NC"));
    }

    /// Validation: output pin on a mask-only pad (no copper layer) is
    /// an error.
    #[test]
    fn validate_errors_output_on_mask_only_pad() {
        let sym = fixture_symbol(vec![("3", "Y", PinElectricalType::Output)]);
        let fp = fixture_footprint(vec![fixture_pad(
            "3",
            [1.0, 1.0],
            vec![LayerId::new("F.Mask")],
        )]);
        let matches = compute_matches(&sym, &fp, &[]);
        let warnings = validate(&matches);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].severity, WarningSeverity::Error);
        assert!(warnings[0].message.contains("mask-only"));
    }
}
