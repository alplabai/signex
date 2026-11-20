//! The Object List's top: the filter field, the This sheet / Project switch
//! and the kind chips with their counts (More ▾ opens the other kinds).

use iced::widget::{Row, column, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::chips::{ChipMetrics, ChipStyle, chip};
use signex_widgets::{SegmentedStyle, segmented, text_input};

use super::model::{KindChip, ObjectKind, ObjectListMsg, ObjectListView, ObjectScope};
use super::msg;
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::{Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The chip label of `kind`.
pub(super) fn kind_name(kind: ObjectKind) -> MessageId {
    match kind {
        ObjectKind::Part => ids::PANELS_OBJECT_LIST_KIND_PART,
        ObjectKind::Label => ids::PANELS_OBJECT_LIST_KIND_LABEL,
        ObjectKind::Wire => ids::PANELS_OBJECT_LIST_KIND_WIRE,
        ObjectKind::PowerPort => ids::PANELS_OBJECT_LIST_KIND_POWER_PORT,
        ObjectKind::Port => ids::PANELS_OBJECT_LIST_KIND_PORT,
        ObjectKind::SheetSymbol => ids::PANELS_OBJECT_LIST_KIND_SHEET_SYMBOL,
        ObjectKind::SheetEntry => ids::PANELS_OBJECT_LIST_KIND_SHEET_ENTRY,
        ObjectKind::Text => ids::PANELS_OBJECT_LIST_KIND_TEXT,
        ObjectKind::Junction => ids::PANELS_OBJECT_LIST_KIND_JUNCTION,
        ObjectKind::Bus => ids::PANELS_OBJECT_LIST_KIND_BUS,
        ObjectKind::Drawing => ids::PANELS_OBJECT_LIST_KIND_DRAWING,
        ObjectKind::Directive => ids::PANELS_OBJECT_LIST_KIND_DIRECTIVE,
    }
}

fn chip_style(tokens: &ThemeTokens) -> ChipStyle {
    let r = resolve(tokens);
    ChipStyle {
        fill: r.field,
        edge: ti(tokens.border),
        hover: ti(tokens.hover),
        ink: r.ink3,
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    }
}

fn segmented_style(tokens: &ThemeTokens) -> SegmentedStyle {
    let r = resolve(tokens);
    SegmentedStyle::new(
        r.sunken,
        r.raised,
        r.shadow,
        r.ink3,
        ti(tokens.text),
        TS.body,
    )
}

fn count(n: usize) -> Option<u32> {
    Some(u32::try_from(n).unwrap_or(u32::MAX))
}

/// Filter field, scope switch and chips.
pub(super) fn top<'a>(ctx: &'a PanelContext, view: &'a ObjectListView) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let filter = text_input(t!(i18n, ids::PANELS_OBJECT_LIST_FILTER), &view.filter)
        .on_input(|s| msg(ObjectListMsg::FilterChanged(s)))
        .style(crate::panels::form_look::text_input_style)
        .size(TS.body)
        .padding([SP.xs, SP.s])
        .width(Length::Fill);
    let scope = segmented(
        [
            (
                ObjectScope::ThisSheet,
                t!(i18n, ids::PANELS_OBJECT_LIST_SCOPE_SHEET).into(),
            ),
            (
                ObjectScope::Project,
                t!(i18n, ids::PANELS_OBJECT_LIST_SCOPE_PROJECT).into(),
            ),
        ],
        view.scope,
        segmented_style(&ctx.tokens),
        |scope| msg(ObjectListMsg::ScopePicked(scope)),
    );
    column![filter, scope, kind_chips(&ctx.tokens, i18n, view)]
        .spacing(SP.s)
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .boxed()
}

/// All, the main kinds and More ▾; the other kinds in a second row while
/// More is open. A chip is on when its kind is picked; All is on when none
/// is.
fn kind_chips<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    view: &'a ObjectListView,
) -> Element<'a, PanelMsg> {
    let style = chip_style(tokens);
    let m = ChipMetrics::from_text_size(TS.body);
    let one = |label: MessageId, n: Option<u32>, on: bool, press: KindChip| {
        chip(
            t!(i18n, label),
            n,
            on,
            style,
            m,
            i18n,
            Some(msg(ObjectListMsg::KindChipToggled(press))),
        )
    };
    let mut main: Row<Element<'a, PanelMsg>> = Row::new().spacing(m.row_gap);
    main = main.push(one(
        ids::PANELS_OBJECT_LIST_CHIP_ALL,
        count(view.counts.total()),
        view.kinds.is_empty(),
        KindChip::All,
    ));
    for kind in ObjectKind::MAIN {
        main = main.push(one(
            kind_name(kind),
            count(view.counts.of(kind)),
            view.kinds.is_on(kind),
            KindChip::Kind(kind),
        ));
    }
    let more_on = ObjectKind::ALL
        .iter()
        .any(|k| !k.is_main() && view.kinds.is_on(*k));
    let more_label = if view.more_open {
        ids::PANELS_OBJECT_LIST_CHIP_FEWER
    } else {
        ids::PANELS_OBJECT_LIST_CHIP_MORE
    };
    main = main.push(chip(
        t!(i18n, more_label),
        count(view.counts.more()),
        more_on,
        style,
        m,
        i18n,
        Some(msg(ObjectListMsg::MoreToggled)),
    ));
    let main = main.wrap().vertical_spacing(m.row_gap).boxed();
    if !view.more_open {
        return main;
    }
    let mut more: Row<Element<'a, PanelMsg>> = Row::new().spacing(m.row_gap);
    for kind in ObjectKind::ALL.into_iter().filter(|k| !k.is_main()) {
        more = more.push(one(
            kind_name(kind),
            count(view.counts.of(kind)),
            view.kinds.is_on(kind),
            KindChip::Kind(kind),
        ));
    }
    let more = more.wrap().vertical_spacing(m.row_gap).boxed();
    column![main, container(more).width(Length::Fill)]
        .spacing(m.row_gap)
        .boxed()
}
