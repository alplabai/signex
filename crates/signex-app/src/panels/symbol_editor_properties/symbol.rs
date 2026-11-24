//! Nothing selected: the symbol's header, the Symbol / Pins tabs and the
//! Symbol tab (illustration, General, Parts, Graphical, Fields).

use iced::widget::row;
use iced::{Alignment, Color, Element, Widget as _};
use signex_i18n::{ids, t};
use signex_types::schematic::BodyStyle;

use super::super::{ColorFieldProps, PanelMsg, SymbolEditorPanelContext, color_field};
use super::illustration::{self, field};
use super::rows::{Form, SP, props};
use super::{part_name, pins_tab};
use crate::library::editor::symbol::{BodyScope, PartStep, SymbolPropsMsg, SymbolPropsTab};
use crate::styles::PillTone;

/// Section keys (`PanelMsg::ToggleSection`).
const GENERAL: &str = "symprops.general";
const PARTS: &str = "symprops.parts";
const GRAPHICAL: &str = "symprops.graphical";
const FIELDS: &str = "symprops.fields";

pub(super) fn view<'a>(
    sym: &'a SymbolEditorPanelContext,
    form: &Form<'_>,
    border_c: Color,
) -> Vec<Element<'a, PanelMsg>> {
    let i18n = form.i18n;
    let file = sym
        .path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut col = vec![
        form.header(
            t!(i18n, ids::EDITORS_SYMPROPS_TITLE).into(),
            Some((part_name(sym.active_part, i18n).into(), PillTone::Neutral)),
            Some((
                t!(
                    i18n,
                    ids::EDITORS_SYMPROPS_FILE_SYMBOL,
                    "file" => file,
                    "symbol" => sym.symbol_name.as_str()
                )
                .into(),
                true,
            )),
        ),
        tabs(sym, form),
    ];
    match sym.props.tab {
        SymbolPropsTab::Pins => col.extend(pins_tab::view(sym, form)),
        SymbolPropsTab::Symbol => col.extend(symbol_tab(sym, form, border_c)),
    }
    col
}

fn tabs<'a>(sym: &SymbolEditorPanelContext, form: &Form<'_>) -> Element<'a, PanelMsg> {
    let i18n = form.i18n;
    let tab = sym.props.tab;
    form.tabs(vec![
        (
            t!(i18n, ids::EDITORS_SYMPROPS_TAB_SYMBOL),
            None,
            tab == SymbolPropsTab::Symbol,
            props(SymbolPropsMsg::TabPicked(SymbolPropsTab::Symbol)),
        ),
        (
            t!(i18n, ids::EDITORS_SYMPROPS_TAB_PINS),
            Some(sym.pins.len()),
            tab == SymbolPropsTab::Pins,
            props(SymbolPropsMsg::TabPicked(SymbolPropsTab::Pins)),
        ),
    ])
}

fn symbol_tab<'a>(
    sym: &'a SymbolEditorPanelContext,
    form: &Form<'_>,
    border_c: Color,
) -> Vec<Element<'a, PanelMsg>> {
    let i18n = form.i18n;
    let mut col = Vec::new();
    if let Some(ill) = illustration::symbol(sym) {
        col.push(illustration::view(ill, form));
    }
    col.push(form.section(
        GENERAL,
        t!(i18n, ids::EDITORS_SYMPROPS_SECTION_GENERAL),
        true,
        None,
        || {
            vec![
                form.field(
                    t!(i18n, ids::EDITORS_SYMPROPS_SYMBOL_NAME),
                    form.input(
                        t!(i18n, ids::EDITORS_SYMPROPS_SYMBOL_NAME),
                        sym.symbol_name.clone(),
                        true,
                        PanelMsg::SymEditorSetSymbolName,
                    ),
                    None,
                ),
                form.field(
                    t!(i18n, ids::EDITORS_SYMPROPS_UUID),
                    form.read_only(
                        signex_i18n::UserText::new(sym.symbol_uuid.to_string()).into(),
                        true,
                    ),
                    None,
                ),
                // Not in the mockup; kept so the count stays visible.
                form.field(
                    t!(i18n, ids::EDITORS_SYMPROPS_GRAPHICS),
                    form.read_only(
                        signex_i18n::UserText::new(i18n.count(sym.graphics.len() as i64)).into(),
                        true,
                    ),
                    None,
                ),
            ]
        },
    ));
    col.push(form.section(
        PARTS,
        t!(i18n, ids::EDITORS_SYMPROPS_SECTION_PARTS),
        true,
        None,
        || parts_rows(sym, form),
    ));
    col.push(form.section(
        GRAPHICAL,
        t!(i18n, ids::EDITORS_SYMPROPS_SECTION_GRAPHICAL),
        true,
        None,
        || graphical_rows(sym, form, border_c),
    ));
    col.push(form.section(
        FIELDS,
        t!(i18n, ids::EDITORS_SYMPROPS_SECTION_FIELDS),
        false,
        Some(2),
        || {
            vec![
                form.field(
                    t!(i18n, ids::EDITORS_SYMPROPS_REFERENCE),
                    form.input(
                        t!(i18n, ids::EDITORS_SYMPROPS_REFERENCE),
                        sym.symbol_designator.clone(),
                        true,
                        PanelMsg::SymEditorSetSymbolDesignator,
                    ),
                    None,
                ),
                form.field(
                    t!(i18n, ids::EDITORS_SYMPROPS_VALUE),
                    form.note(t!(i18n, ids::EDITORS_SYMPROPS_VALUE_BOUND)),
                    None,
                ),
            ]
        },
    ));
    col.push(form.caption(t!(i18n, ids::EDITORS_SYMPROPS_SELECT_HINT)));
    col
}

/// The Part stepper (‹ part › + −) and the Body choice. The stepper runs
/// the toolbar's part messages, so the arrows stop where the toolbar's do.
fn parts_rows<'a>(sym: &SymbolEditorPanelContext, form: &Form<'_>) -> Vec<Element<'a, PanelMsg>> {
    let i18n = form.i18n;
    let step = |s: PartStep| Some(props(SymbolPropsMsg::PartStepPressed(s)));
    let part = sym.active_part;
    let max = sym.active_max_part;
    let stepper = row![
        form.ghost(
            t!(i18n, ids::EDITORS_SYMPROPS_PREVIOUS_PART),
            if part > 1 {
                step(PartStep::Previous)
            } else {
                None
            },
            Some(t!(i18n, ids::EDITORS_SYMPROPS_PREVIOUS_PART_TIP)),
        ),
        iced::widget::container(form.read_only(part_name(part, i18n).into(), false))
            .width(iced::Length::Fill),
        form.ghost(
            t!(i18n, ids::EDITORS_SYMPROPS_NEXT_PART),
            if part < max {
                step(PartStep::Next)
            } else {
                None
            },
            Some(t!(i18n, ids::EDITORS_SYMPROPS_NEXT_PART_TIP)),
        ),
        form.ghost(
            t!(i18n, ids::EDITORS_SYMBOL_ADD_PART),
            step(PartStep::Added),
            Some(t!(i18n, ids::EDITORS_SYMPROPS_ADD_PART_TIP)),
        ),
        form.ghost(
            t!(i18n, ids::EDITORS_SYMBOL_REMOVE_PART),
            if max > 1 && part > 1 {
                step(PartStep::Removed)
            } else {
                None
            },
            Some(t!(i18n, ids::EDITORS_SYMPROPS_REMOVE_PART_TIP)),
        ),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center);
    let body = |b: BodyStyle, id: signex_i18n::MessageId| {
        (
            t!(i18n, id),
            sym.active_body == b,
            props(SymbolPropsMsg::BodyPicked(b)),
        )
    };
    vec![
        form.field(
            t!(i18n, ids::EDITORS_SYMPROPS_PART),
            stepper.boxed(),
            Some(field::SYM_PART),
        ),
        form.field(
            t!(i18n, ids::EDITORS_SYMBOL_BODY),
            form.segmented(vec![
                body(BodyStyle::Normal, ids::EDITORS_SYMBOL_BODY_NORMAL),
                body(BodyStyle::Alternate, ids::EDITORS_SYMBOL_BODY_ALTERNATE),
            ]),
            None,
        ),
        applies_to_row(sym, form),
    ]
}

/// Applies to (mockup row 123): the canvas bar's Both / This only on the
/// selection. With nothing selected it shows the shown items' scope, disabled.
fn applies_to_row<'a>(sym: &SymbolEditorPanelContext, form: &Form<'_>) -> Element<'a, PanelMsg> {
    let i18n = form.i18n;
    let row = sym.body_scope;
    let cell = |scope: BodyScope, id: signex_i18n::MessageId| {
        (
            t!(i18n, id),
            row.scope == Some(scope),
            row.can_assign
                .then(|| props(SymbolPropsMsg::BodyScopePicked(scope))),
        )
    };
    form.field(
        t!(i18n, ids::EDITORS_SYMPROPS_APPLIES_TO),
        form.segmented_maybe(vec![
            cell(BodyScope::Both, ids::EDITORS_SYMBOL_BODY_BOTH),
            cell(BodyScope::ThisOnly, ids::EDITORS_SYMBOL_BODY_THIS_ONLY),
        ]),
        None,
    )
}

fn graphical_rows<'a>(
    sym: &SymbolEditorPanelContext,
    form: &Form<'_>,
    border_c: Color,
) -> Vec<Element<'a, PanelMsg>> {
    use crate::app::LocalColorSlot as Slot;
    let i18n = form.i18n;
    let colour = |label: signex_i18n::MessageId, slot: Slot, current: Option<[u8; 4]>| {
        local_color_field(form, label, slot, current, sym, border_c)
    };
    vec![
        form.field(
            t!(i18n, ids::EDITORS_SYMPROPS_MIRRORED),
            signex_widgets::checkbox(sym.symbol_mirrored)
                .size(super::rows::TS.heading)
                .style(super::super::form_look::checkbox_style)
                .on_toggle(|_| PanelMsg::SymEditorToggleSymbolMirrored)
                .boxed(),
            Some(field::SYM_MIRROR),
        ),
        form.mini_heading(t!(i18n, ids::EDITORS_SYMPROPS_LOCAL_COLORS)),
        colour(
            ids::EDITORS_SYMPROPS_FILLS,
            Slot::Fill,
            sym.symbol_local_fill_color,
        ),
        colour(
            ids::EDITORS_SYMPROPS_LINES,
            Slot::Line,
            sym.symbol_local_line_color,
        ),
        colour(
            ids::EDITORS_SYMPROPS_PINS,
            Slot::Pin,
            sym.symbol_local_pin_color,
        ),
    ]
}

/// One Local Colors row (Fills / Lines / Pins) on the shared colour field:
/// a swatch, or "Inherit" while the slot is unset. The palette opens below
/// the row while the editor's picker targets this slot.
fn local_color_field<'a>(
    form: &Form<'_>,
    label: signex_i18n::MessageId,
    slot: crate::app::LocalColorSlot,
    current: Option<[u8; 4]>,
    sym: &SymbolEditorPanelContext,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    use std::rc::Rc;
    let i18n = form.i18n;
    let this = sym.local_color_picker.filter(|p| p.slot == slot);
    let on_pick: Rc<dyn Fn([u8; 4]) -> PanelMsg + 'static> =
        Rc::new(move |rgba| PanelMsg::SymEditorSetLocalColor { slot, color: rgba });
    color_field(
        ColorFieldProps {
            label: t!(i18n, label).into(),
            current,
            none_label: t!(i18n, ids::EDITORS_SYMPROPS_INHERIT).into(),
            show_palette: this.is_some(),
            show_advanced: this.is_some_and(|p| p.advanced),
            muted: form.look.ink3,
            border_c,
            on_toggle: PanelMsg::SymEditorToggleLocalColorPicker(slot),
            on_advanced: PanelMsg::SymEditorOpenLocalColorAdvanced(slot),
            on_cancel: PanelMsg::SymEditorCancelLocalColorPicker,
            on_pick,
            on_clear: current
                .is_some()
                .then_some(PanelMsg::SymEditorClearLocalColor(slot)),
        },
        i18n,
    )
}
