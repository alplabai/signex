//! Keepout role sub-form. Split from `subforms.rs`.

use iced::widget::{Column, container, text};
use iced::{Color, Length};

use super::super::super::{FootprintEditorPanelContext, KeepoutKindFlag, PanelMsg};

/// v0.16.4 — Keepout role sub-form. Renders the 6 kind flags as a
/// vertical checklist when the entity's `keepout` attr is set.
pub(in crate::panels::footprint_editor_properties) fn render_keepout_subform<'a>(
    mut col: Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    id: signex_sketch::id::SketchEntityId,
    muted: Color,
    primary: Color,
    _border_c: Color,
) -> Column<'a, PanelMsg> {
    let Some(k) = fp.selected_keepout.as_ref() else {
        return col;
    };
    col = col.push(
        container(text("Keepout kinds").size(10).color(primary))
            .padding([4, 8])
            .width(Length::Fill),
    );

    // v0.18.25 — keepout kinds use the schematic's `form_check_row`
    // (real iced checkbox + On/Off) so the chrome matches the
    // schematic Properties panel byte-for-byte.
    col = col.push(super::super::super::form_check_row(
        "No routing",
        k.no_routing,
        PanelMsg::FpEditorSetKeepoutKind {
            id,
            kind: KeepoutKindFlag::NoRouting,
            value: !k.no_routing,
        },
        muted,
    ));
    col = col.push(super::super::super::form_check_row(
        "No components",
        k.no_components,
        PanelMsg::FpEditorSetKeepoutKind {
            id,
            kind: KeepoutKindFlag::NoComponents,
            value: !k.no_components,
        },
        muted,
    ));
    col = col.push(super::super::super::form_check_row(
        "No copper",
        k.no_copper,
        PanelMsg::FpEditorSetKeepoutKind {
            id,
            kind: KeepoutKindFlag::NoCopper,
            value: !k.no_copper,
        },
        muted,
    ));
    col = col.push(super::super::super::form_check_row(
        "No vias",
        k.no_vias,
        PanelMsg::FpEditorSetKeepoutKind {
            id,
            kind: KeepoutKindFlag::NoVias,
            value: !k.no_vias,
        },
        muted,
    ));
    col = col.push(super::super::super::form_check_row(
        "No drilling",
        k.no_drilling,
        PanelMsg::FpEditorSetKeepoutKind {
            id,
            kind: KeepoutKindFlag::NoDrilling,
            value: !k.no_drilling,
        },
        muted,
    ));
    col = col.push(super::super::super::form_check_row(
        "No pours",
        k.no_pours,
        PanelMsg::FpEditorSetKeepoutKind {
            id,
            kind: KeepoutKindFlag::NoPours,
            value: !k.no_pours,
        },
        muted,
    ));

    col
}

