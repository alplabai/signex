//! Side-by-side visual diff card.
//!
//! Drives:
//!
//! 1. Two `Canvas` previews (symbol + footprint) side-by-side, prev
//!    on the left, selected on the right.
//! 2. A parameter table — added rows green, removed rows red, changed
//!    rows show `old → new`.
//! 3. Compact supplier add/remove rows + lifecycle banner.
//!
//! All data comes from `signex_library::diff::diff_revisions` which
//! WS-D shipped. The diff is recomputed each frame because Iced
//! immediate-mode repaints and the cost is negligible — Standard
//! footprints in the wild peak in the low hundreds of nodes.

use iced::widget::{Space, canvas, column, container, row, text};
use iced::{Border, Element, Length, Theme};
use signex_library::diff::{
    FootprintDiff, LifecycleDiff, ParameterDiff, SupplierDiff, SymbolDiff, diff_revisions,
};
// Re-exports kept above so the helper signatures are obvious.
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::super::messages::LibraryMessage;
use super::super::super::state::ComponentEditorState;
use super::footprint_canvas::{FootprintDiffCanvas, extract_pads};
use super::symbol_canvas::{Side, SymbolDiffCanvas, extract_pins};

/// Render the diff card. When the user hasn't picked a revision (or
/// only one revision exists), we render the empty-state hint instead.
pub fn view<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let border = theme_ext::border_color(tokens);

    let Some(selected_version) = editor.history_selected else {
        return empty_state(
            "Pick a revision above to see the diff against its predecessor.",
            tokens,
        );
    };

    let revs = &editor.component.revisions;
    let Some(next_idx) = revs.iter().position(|r| r.version == selected_version) else {
        return empty_state(
            "Selected revision is no longer in the component history.",
            tokens,
        );
    };
    if next_idx == 0 {
        return empty_state(
            "This is the very first revision — no predecessor to diff against.",
            tokens,
        );
    }
    let prev = &revs[next_idx - 1];
    let next = &revs[next_idx];
    let diff = diff_revisions(prev, next);

    // Pre-build child elements with owned diff data so they don't
    // borrow `diff` past this function's stack frame.
    let header_label: Element<'a, LibraryMessage> = diff_header_label(prev, next, tokens);
    let lifecycle: Element<'a, LibraryMessage> = lifecycle_banner(diff.lifecycle.clone(), tokens);

    let header: Element<'a, LibraryMessage> =
        row![header_label, Space::new().width(Length::Fill), lifecycle,]
            .align_y(iced::Alignment::Center)
            .into();

    let symbol_left = labeled_canvas(
        prev.version.to_string(),
        symbol_canvas_for(prev, next, &diff.symbol, Side::Prev),
        tokens,
    );
    let symbol_right = labeled_canvas(
        next.version.to_string(),
        symbol_canvas_for(prev, next, &diff.symbol, Side::Next),
        tokens,
    );
    let symbol_row: Element<'a, LibraryMessage> =
        row![symbol_left, Space::new().width(8), symbol_right,].into();

    let fp_left = labeled_canvas(
        prev.version.to_string(),
        footprint_canvas_for(prev, next, &diff.footprint, Side::Prev),
        tokens,
    );
    let fp_right = labeled_canvas(
        next.version.to_string(),
        footprint_canvas_for(prev, next, &diff.footprint, Side::Next),
        tokens,
    );
    let footprint_row: Element<'a, LibraryMessage> =
        row![fp_left, Space::new().width(8), fp_right,].into();

    let params_section: Element<'a, LibraryMessage> = parameter_table(diff.parameters, tokens);
    let suppliers_section: Element<'a, LibraryMessage> = supplier_rows(diff.suppliers, tokens);

    let body: Element<'a, LibraryMessage> = column![
        header,
        Space::new().height(8),
        section_label("Symbol", tokens),
        symbol_row,
        Space::new().height(10),
        section_label("Footprint", tokens),
        footprint_row,
        Space::new().height(10),
        section_label("Parameters", tokens),
        params_section,
        Space::new().height(10),
        section_label("Suppliers", tokens),
        suppliers_section,
    ]
    .spacing(0)
    .into();

    container(iced::widget::scrollable(container(body).padding(10)))
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.02,
            ))),
            border: Border {
                width: 1.0,
                radius: 4.0.into(),
                color: border,
            },
            ..Default::default()
        })
        .height(Length::Fill)
        .width(Length::Fill)
        .into()
}

fn diff_header_label<'a>(
    prev: &'a signex_library::Revision,
    next: &'a signex_library::Revision,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    text(format!("Diff: v{}  →  v{}", prev.version, next.version))
        .size(13)
        .color(text_c)
        .into()
}

fn empty_state<'a>(msg: &'a str, tokens: &'a ThemeTokens) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    container(
        column![
            text("Visual diff").size(13).color(muted),
            Space::new().height(6),
            text(msg).size(11).color(muted),
        ]
        .spacing(0),
    )
    .padding(14)
    .style(move |_: &Theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            1.0, 1.0, 1.0, 0.02,
        ))),
        border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: border,
        },
        ..Default::default()
    })
    .into()
}

fn section_label<'a>(label: &'a str, tokens: &'a ThemeTokens) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    container(text(label).size(11).color(muted))
        .padding([2, 0])
        .into()
}

/// Labeled canvas pane — small `vX.Y` caption above a fixed-height
/// preview. Caller passes the version label so this helper doesn't
/// have to borrow the `Revision`. The canvas itself sends `()` so we
/// `.map(|_| Noop)` it into the LibraryMessage stream — clicks on
/// the diff preview are intentionally inert in Phase 1.
fn labeled_canvas<'a, P: 'static + canvas::Program<(), Theme>>(
    version_label: String,
    program: P,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    let label_el: Element<'a, LibraryMessage> = text(format!("v{version_label}"))
        .size(10)
        .color(muted)
        .into();
    let canvas_el: Element<'a, ()> = container(
        canvas(program)
            .width(Length::Fill)
            .height(Length::Fixed(110.0)),
    )
    .padding(2)
    .style(move |_: &Theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            0.0, 0.0, 0.0, 0.20,
        ))),
        border: Border {
            width: 1.0,
            radius: 3.0.into(),
            color: border,
        },
        ..Default::default()
    })
    .into();
    let canvas_el: Element<'a, LibraryMessage> = canvas_el.map(|_| LibraryMessage::Noop);
    column![label_el, canvas_el]
        .spacing(2)
        .width(Length::FillPortion(1))
        .into()
}

fn symbol_canvas_for(
    prev: &signex_library::Revision,
    next: &signex_library::Revision,
    diff: &SymbolDiff,
    side: Side,
) -> SymbolDiffCanvas {
    let pins = match side {
        Side::Prev => extract_pins(&prev.schematic.symbol.sexpr),
        Side::Next => extract_pins(&next.schematic.symbol.sexpr),
    };
    SymbolDiffCanvas::new(
        side,
        pins,
        diff.added_pins.clone(),
        diff.removed_pins.clone(),
        diff.moved_pins.clone(),
    )
}

fn footprint_canvas_for(
    prev: &signex_library::Revision,
    next: &signex_library::Revision,
    diff: &FootprintDiff,
    side: Side,
) -> FootprintDiffCanvas {
    let pads = match side {
        Side::Prev => extract_pads(&prev.pcb.footprint.sexpr),
        Side::Next => extract_pads(&next.pcb.footprint.sexpr),
    };
    FootprintDiffCanvas::new(
        side,
        pads,
        diff.added_pads.clone(),
        diff.removed_pads.clone(),
    )
}

/// Three-column parameter table — added rows green-tinted, removed
/// rows red-tinted, changed rows show `old → new`. Takes the diff by
/// value because the borrow can't outlive the parent view stack frame.
fn parameter_table<'a>(
    diff: ParameterDiff,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    if diff.added.is_empty() && diff.removed.is_empty() && diff.changed.is_empty() {
        return container(text("No parameter changes.").size(10).color(muted))
            .padding([2, 0])
            .into();
    }

    let header_row: Element<'a, LibraryMessage> = row![
        text("Key")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(2)),
        text("Old")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(3)),
        text("New")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(3)),
    ]
    .padding([2, 4])
    .into();
    let mut col = column![header_row].spacing(0);

    for k in diff.added {
        col = col.push(param_row(
            &k,
            "—",
            "(added)",
            ParamRowKind::Added,
            text_c,
            border,
        ));
    }
    for k in diff.removed {
        col = col.push(param_row(
            &k,
            "(removed)",
            "—",
            ParamRowKind::Removed,
            text_c,
            border,
        ));
    }
    for (k, old, new) in diff.changed {
        col = col.push(param_row(
            &k,
            &old,
            &new,
            ParamRowKind::Changed,
            text_c,
            border,
        ));
    }
    col.into()
}

#[derive(Clone, Copy)]
enum ParamRowKind {
    Added,
    Removed,
    Changed,
}

fn param_row<'a>(
    key: &str,
    old: &str,
    new: &str,
    kind: ParamRowKind,
    text_c: iced::Color,
    border: iced::Color,
) -> Element<'a, LibraryMessage> {
    let bg = match kind {
        ParamRowKind::Added => iced::Color::from_rgba(0.20, 0.78, 0.34, 0.18),
        ParamRowKind::Removed => iced::Color::from_rgba(0.93, 0.25, 0.25, 0.18),
        ParamRowKind::Changed => iced::Color::from_rgba(0.30, 0.55, 0.95, 0.14),
    };
    container(
        row![
            text(key.to_string())
                .size(11)
                .color(text_c)
                .width(Length::FillPortion(2)),
            text(old.to_string())
                .size(11)
                .color(text_c)
                .width(Length::FillPortion(3)),
            text(new.to_string())
                .size(11)
                .color(text_c)
                .width(Length::FillPortion(3)),
        ]
        .padding([2, 4]),
    )
    .style(move |_: &Theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(bg)),
        border: Border {
            width: 0.5,
            radius: 2.0.into(),
            color: border,
        },
        ..Default::default()
    })
    .into()
}

fn supplier_rows<'a>(diff: SupplierDiff, tokens: &'a ThemeTokens) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);

    if diff.added.is_empty() && diff.removed.is_empty() {
        return container(text("No supplier changes.").size(10).color(muted))
            .padding([2, 0])
            .into();
    }

    let mut col = column![].spacing(2);
    for s in diff.added {
        col = col.push(supplier_row(&s, true, text_c));
    }
    for s in diff.removed {
        col = col.push(supplier_row(&s, false, text_c));
    }
    col.into()
}

fn supplier_row<'a>(
    label: &str,
    is_added: bool,
    text_c: iced::Color,
) -> Element<'a, LibraryMessage> {
    let bg = if is_added {
        iced::Color::from_rgba(0.20, 0.78, 0.34, 0.18)
    } else {
        iced::Color::from_rgba(0.93, 0.25, 0.25, 0.18)
    };
    let prefix = if is_added { "+ " } else { "- " };
    container(text(format!("{prefix}{label}")).size(11).color(text_c))
        .padding([2, 6])
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(bg)),
            border: Border {
                width: 0.0,
                radius: 2.0.into(),
                color: iced::Color::TRANSPARENT,
            },
            ..Default::default()
        })
        .into()
}

fn lifecycle_banner<'a>(
    diff: LifecycleDiff,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    if diff.from.is_none() && diff.to.is_none() {
        return Space::new().width(0).into();
    }
    let from = diff
        .from
        .map(|s| format!("{s:?}"))
        .unwrap_or_else(|| "—".into());
    let to = diff
        .to
        .map(|s| format!("{s:?}"))
        .unwrap_or_else(|| "—".into());
    container(
        text(format!("Lifecycle: {from} → {to}"))
            .size(10)
            .color(muted),
    )
    .padding([2, 6])
    .style(move |_: &Theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            0.95, 0.65, 0.20, 0.18,
        ))),
        border: Border {
            width: 0.5,
            radius: 2.0.into(),
            color: iced::Color::from_rgba(0.95, 0.65, 0.20, 0.40),
        },
        ..Default::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::{
        Component, FootprintBody, InternalPn, LifecycleState, ParamValue, PcbSide, SchematicSide,
        SharedSide, SupplierLink, SymbolBody, Version,
    };
    use uuid::Uuid;

    fn rev(version: Version, sym: &str, fp: &str) -> signex_library::Revision {
        signex_library::Revision {
            version,
            state: LifecycleState::Released,
            created: chrono::Utc::now(),
            author: "test".into(),
            message: "fix".into(),
            schematic: SchematicSide {
                symbol: SymbolBody {
                    sexpr: sym.to_string(),
                },
                ..Default::default()
            },
            pcb: PcbSide {
                footprint: FootprintBody {
                    sexpr: fp.to_string(),
                },
                ..Default::default()
            },
            shared: SharedSide::default(),
            content_hash: [0u8; 32],
        }
    }

    /// Smoke test — feeding two revisions through the diff pipeline
    /// must produce a non-empty `RevisionDiff` we can render.
    /// Direct view-rendering test would need an Iced runtime, so we
    /// just verify the data side wiring (every primitive used in the
    /// view is on the public surface).
    #[test]
    fn diff_pipeline_surfaces_every_change_kind() {
        let sym_a = r#"(symbol "X"
            (pin passive line (at 0 0 0) (length 1) (name "1") (number "1"))
            (pin passive line (at 5 0 0) (length 1) (name "2") (number "2")))"#;
        let sym_b = r#"(symbol "X"
            (pin passive line (at 1 0 0) (length 1) (name "1") (number "1"))
            (pin passive line (at 5 0 0) (length 1) (name "2") (number "2"))
            (pin passive line (at 0 5 0) (length 1) (name "3") (number "3")))"#;
        let fp_a = r#"(footprint "F" (pad "1" smd rect (at 0 0)))"#;
        let fp_b = r#"(footprint "F" (pad "1" smd rect (at 0 0)) (pad "2" smd rect (at 1 0)))"#;

        let mut a = rev(Version::new(1, 0), sym_a, fp_a);
        let mut b = rev(Version::new(1, 1), sym_b, fp_b);
        a.shared
            .parameters
            .insert("R".into(), ParamValue::Text("10k".into()));
        b.shared
            .parameters
            .insert("R".into(), ParamValue::Text("12k".into()));
        b.shared
            .parameters
            .insert("tol".into(), ParamValue::Text("1%".into()));
        b.shared.suppliers.push(SupplierLink {
            distributor: "Mouser".into(),
            sku: "M-1".into(),
            url: None,
        });
        b.state = LifecycleState::Deprecated;

        let diff = diff_revisions(&a, &b);
        // Symbol changes captured.
        assert!(!diff.symbol.added_pins.is_empty());
        assert!(!diff.symbol.moved_pins.is_empty());
        // Footprint changes captured.
        assert!(!diff.footprint.added_pads.is_empty());
        // Parameter changes captured.
        assert!(!diff.parameters.added.is_empty());
        assert!(!diff.parameters.changed.is_empty());
        // Supplier changes captured.
        assert!(!diff.suppliers.added.is_empty());
        // Lifecycle changes captured.
        assert!(diff.lifecycle.from.is_some());
    }

    /// Construction of the per-side canvases must succeed with both
    /// real and empty bodies — the empty-body branch is what catches
    /// brand-new components that have never been rendered.
    #[test]
    fn build_canvases_for_empty_revisions() {
        let a = rev(Version::new(1, 0), "", "");
        let b = rev(Version::new(1, 1), "", "");
        let diff = diff_revisions(&a, &b);
        let _ = symbol_canvas_for(&a, &b, &diff.symbol, Side::Prev);
        let _ = symbol_canvas_for(&a, &b, &diff.symbol, Side::Next);
        let _ = footprint_canvas_for(&a, &b, &diff.footprint, Side::Prev);
        let _ = footprint_canvas_for(&a, &b, &diff.footprint, Side::Next);
    }

    /// Helper to keep the smoke-test fixture builder honest.
    #[test]
    fn fixture_revs_round_trip_through_component() {
        let r0 = rev(Version::new(1, 0), "", "");
        let r1 = rev(Version::new(1, 1), "", "");
        let _comp = Component {
            uuid: Uuid::now_v7(),
            internal_pn: InternalPn::new("R_TEST"),
            head: Version::new(1, 1),
            revisions: vec![r0, r1],
        };
    }
}
