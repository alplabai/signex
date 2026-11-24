//! The tab's frame (mockup row 105): the shared document-tab shell with the
//! toolbar, the sub-tabs and a footer, around the Stack page (the table on
//! the left, the cross-section and preview on the right) or the Via Types /
//! Drill Pairs page. Under the table: the drill pair legend with the edit
//! hint, and the layer pairs.

use iced::widget::{Row, Space, column, container, row, scrollable};
use iced::{Alignment, Background, Element, Length, Theme, Widget as _};
use signex_i18n::{MessageId, UserText, ids, t};
use signex_widgets::doc_tab_shell::{DocTabShell, DocTabShellStyle, Footer, SubTab, SubTabs};
use signex_widgets::text;

use super::{
    LayerStackFacts, LayerStackMsg, SP, StackSubTab, TS, drills, msg, section, table, toolbar,
};
use crate::panels::{PanelContext, PanelMsg, empty_state_card};
use crate::styles::{HAIRLINE, ti, tokens::resolve};

/// The Stack page's share of the width for the table, and for the drawings.
const TABLE_SHARE: u16 = 3;
const DRAWING_SHARE: u16 = 2;

/// The tab's content, or its empty state when the board cannot be read.
pub fn view(ctx: &PanelContext) -> Element<'_, PanelMsg> {
    let Some(facts) = ctx.layer_stack.as_ref() else {
        return empty_state_card(
            &ctx.tokens,
            t!(ctx.i18n, ids::PCB_STACK_NO_BOARD_TITLE),
            t!(ctx.i18n, ids::PCB_STACK_NO_BOARD),
            None,
        );
    };
    let body = match facts.sub_tab {
        StackSubTab::Stack => stack_page(ctx, facts),
        StackSubTab::Drills => drills::view(ctx, facts),
    };
    DocTabShell::new(body, DocTabShellStyle::from_tokens(&ctx.tokens, TS.body))
        .toolbar(toolbar::view(ctx, facts))
        .sub_tabs(sub_tabs(ctx, facts))
        .footer(footer(ctx, facts))
        .view()
}

fn sub_tabs<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> SubTabs<'a, PanelMsg> {
    let tabs = StackSubTab::ALL
        .into_iter()
        .map(|tab| SubTab::new(t!(ctx.i18n, tab.message())))
        .collect();
    let selected = StackSubTab::ALL
        .iter()
        .position(|tab| *tab == facts.sub_tab)
        .unwrap_or_default();
    let board = facts.board.clone();
    SubTabs::new(tabs, selected).on_select(move |index| {
        msg(LayerStackMsg::SubTabPicked {
            board: board.clone(),
            tab: StackSubTab::ALL
                .get(index)
                .copied()
                .unwrap_or(StackSubTab::Stack),
        })
    })
}

fn footer<'a>(ctx: &'a PanelContext, facts: &LayerStackFacts) -> Footer<'a> {
    let i18n = &*ctx.i18n;
    let copper = t!(i18n, ids::PCB_STACK_COPPER_COUNT, "count" => facts.stack.copper_count());
    let rows = t!(i18n, ids::PCB_STACK_ROW_COUNT, "count" => facts.stack.layers.len());
    let status = t!(
        i18n,
        ids::PCB_STACK_FOOTER,
        "copper" => copper.into_string(),
        "rows" => rows.into_string()
    );
    Footer::new(status).note(t!(i18n, ids::PCB_STACK_FOOTER_NOTE))
}

/// The table with its notes, beside the two drawings.
fn stack_page<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let left = scrollable(
        column![table::view(ctx, facts), notes(ctx, facts)]
            .spacing(SP.s)
            .padding([SP.s, 0.0])
            .width(Length::Fill),
    )
    .width(Length::FillPortion(TABLE_SHARE))
    .height(Length::Fill);
    let right = column![
        heading(ctx, ids::PCB_STACK_SECTION, ids::PCB_STACK_SECTION_HINT),
        section::cross_section(ctx, facts),
        heading(ctx, ids::PCB_STACK_PREVIEW, ids::PCB_STACK_PREVIEW_HINT),
        section::preview(ctx, facts),
    ]
    .spacing(SP.s)
    .padding(SP.m)
    .width(Length::FillPortion(DRAWING_SHARE))
    .height(Length::Fill);
    row![left, right].height(Length::Fill).boxed()
}

/// A drawing's title and its caption.
pub(super) fn heading<'a>(
    ctx: &'a PanelContext,
    title: MessageId,
    hint: MessageId,
) -> Element<'a, PanelMsg> {
    let semibold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    row![
        text(t!(ctx.i18n, title))
            .size(TS.body)
            .font(semibold)
            .color(ti(ctx.tokens.text)),
        text(t!(ctx.i18n, hint))
            .size(TS.caption)
            .color(resolve(&ctx.tokens).ink3),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// The drill pair legend with the edit hint, the layer pairs, then why
/// nothing can be edited, that the stack is not stored yet, and the Enter
/// hint while a cell holds a draft.
fn notes<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let muted = resolve(&ctx.tokens).ink3;
    let note = |id| text(t!(ctx.i18n, id)).size(TS.dense).color(muted).boxed();
    let mut col = column![legend(ctx, facts), layer_pairs(ctx, facts)].spacing(SP.s);
    if facts.read_only {
        col = col.push(note(ids::PCB_STACK_READ_ONLY));
    } else if !facts.stored {
        col = col.push(note(ids::PCB_STACK_DERIVED));
    }
    if !facts.drafts.is_empty() {
        col = col.push(note(ids::PCB_STACK_ENTER_HINT));
    }
    container(col)
        .padding([SP.s, SP.l])
        .width(Length::Fill)
        .boxed()
}

/// The length of a legend key, in body-text ems.
const KEY_EMS: f32 = 1.0;

/// One key per drill pair, "Through 1–4": a full bar for a pair through the
/// whole stack, an accent bar for one that stops inside it. Then the edit
/// hint, unless the board is read-only.
fn legend<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let r = resolve(&ctx.tokens);
    let copper = facts.stack.copper_count();
    let mut keys: Row<Element<'a, PanelMsg>> = Row::new().spacing(SP.m).align_y(Alignment::Center);
    for pair in &facts.drill_pairs {
        let colour = if pair.span == (1, copper) {
            ti(ctx.tokens.text)
        } else {
            ti(ctx.tokens.accent)
        };
        let key = container(Space::new())
            .width(Length::Fixed(TS.body * KEY_EMS))
            .height(Length::Fixed(2.0 * HAIRLINE))
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(colour)),
                ..container::Style::default()
            });
        let name = text(UserText::new(pair.name.clone()))
            .size(TS.dense)
            .color(ti(ctx.tokens.text));
        keys = keys.push(
            row![key, name]
                .spacing(SP.xs)
                .align_y(Alignment::Center)
                .boxed(),
        );
    }
    let caption =
        |id: MessageId, size: f32| text(t!(ctx.i18n, id)).size(size).color(r.ink3).boxed();
    if facts.drill_pairs.is_empty() {
        keys = keys.push(caption(ids::PCB_STACK_NO_DRILL_PAIRS, TS.dense));
    }
    if !facts.read_only {
        keys = keys.push(caption(ids::PCB_STACK_EDIT_HINT, TS.caption));
    }
    keys.wrap().vertical_spacing(SP.xs).boxed()
}

/// Layer pairs: each signal layer and the plane nearest it, with the nets
/// poured on that plane.
fn layer_pairs<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let r = resolve(&ctx.tokens);
    let semibold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    let name = |row: usize| facts.names.get(row).cloned().unwrap_or_default();
    let mut line: Row<Element<'a, PanelMsg>> = Row::new().spacing(SP.m).align_y(Alignment::Center);
    line = line.push(
        text(t!(ctx.i18n, ids::PCB_STACK_LAYER_PAIRS))
            .size(TS.body)
            .font(semibold)
            .color(ti(ctx.tokens.text))
            .boxed(),
    );
    line = line.push(
        text(t!(ctx.i18n, ids::PCB_STACK_LAYER_PAIRS_HINT))
            .size(TS.caption)
            .color(r.ink3)
            .boxed(),
    );
    let pairs = super::layer_pairs(&facts.stack);
    if pairs.iter().all(|(_, plane)| plane.is_none()) {
        line = line.push(
            text(t!(ctx.i18n, ids::PCB_STACK_NO_PLANE))
                .size(TS.dense)
                .color(r.ink3)
                .boxed(),
        );
    }
    for (signal, plane) in pairs {
        let Some(plane) = plane else { continue };
        let pair = t!(
            ctx.i18n,
            ids::PCB_STACK_LAYER_PAIR,
            "signal" => name(signal),
            "plane" => name(plane)
        );
        let nets = facts
            .copper_use
            .get(plane)
            .and_then(Option::as_ref)
            .map(|used| used.nets.join(", "))
            .unwrap_or_default();
        let mut entry: Row<Element<'a, PanelMsg>> =
            Row::new().spacing(SP.xs).align_y(Alignment::Center);
        entry = entry.push(text(pair).size(TS.dense).color(ti(ctx.tokens.text)).boxed());
        if !nets.is_empty() {
            entry = entry.push(
                text(UserText::new(nets))
                    .size(TS.caption)
                    .color(r.ink3)
                    .boxed(),
            );
        }
        line = line.push(entry.boxed());
    }
    line.wrap().vertical_spacing(SP.xs).boxed()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use signex_types::pcb_stack::{LayerStack, StackLayer, StackLayerKind};

    use super::view;
    use crate::layout_probe::{lay_out_texts, messages_on_click};
    use crate::panels::layer_stack::{
        CopperUse, DrillPairRow, LayerStackFacts, StackSubTab, StackUnit,
    };
    use crate::panels::{PanelContext, PanelMsg};

    const WINDOW: iced::Size = iced::Size::new(3200.0, 1600.0);

    /// Mask, Top (signal), GND plane, +3V3 plane, Bottom (signal), mask,
    /// with dielectrics between the copper.
    fn stack(planes: bool) -> LayerStack {
        use StackLayerKind::{Dielectric, Plane, Signal, SolderMask};
        let inner = if planes { Plane } else { Signal };
        let rows = [
            (SolderMask, ""),
            (Signal, "TopCopper"),
            (Dielectric, ""),
            (inner, "Inner1Copper"),
            (Dielectric, ""),
            (inner, "Inner2Copper"),
            (Dielectric, ""),
            (Signal, "BottomCopper"),
            (SolderMask, ""),
        ];
        LayerStack {
            name: None,
            layers: rows
                .into_iter()
                .map(|(kind, layer)| StackLayer {
                    layer: layer.into(),
                    ..StackLayer::new(kind)
                })
                .collect(),
        }
    }

    fn pair(name: &str, span: (usize, usize)) -> DrillPairRow {
        DrillPairRow {
            name: name.into(),
            from: String::new(),
            to: String::new(),
            kind: String::new(),
            span,
            depth_mm: None,
            min_drill_mm: None,
            aspect_ratio: None,
            vias: 1,
        }
    }

    fn facts(planes: bool) -> LayerStackFacts {
        let stack = stack(planes);
        let names = [
            "Top Mask",
            "Top",
            "Prepreg",
            "Mid 1",
            "Core",
            "Mid 2",
            "Prepreg",
            "Bottom",
            "Bottom Mask",
        ]
        .map(String::from)
        .to_vec();
        let used = |items, nets: &[&str]| {
            Some(CopperUse {
                items,
                nets: nets.iter().map(|n| n.to_string()).collect(),
            })
        };
        LayerStackFacts {
            board: PathBuf::from("board.snxpcb"),
            copper_use: vec![
                None,
                used(412, &[]),
                None,
                used(2, &["GND"]),
                None,
                used(3, &["+3V3"]),
                None,
                used(96, &[]),
                None,
            ],
            stack,
            stored: true,
            read_only: false,
            names,
            selected: None,
            drafts: BTreeMap::new(),
            name_draft: None,
            unit: StackUnit::Mm,
            sub_tab: StackSubTab::Stack,
            drill_pairs: vec![pair("Through 1–4", (1, 4)), pair("Blind 1–2", (1, 2))],
            via_types: Vec::new(),
            has_project: true,
            fab_profile: Some("Standard".into()),
        }
    }

    fn context(facts: LayerStackFacts) -> PanelContext {
        let (app, _boot) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.layer_stack = Some(facts);
        ctx
    }

    fn texts(ctx: &PanelContext) -> Vec<String> {
        lay_out_texts(view(ctx), WINDOW)
            .into_iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect()
    }

    fn has(shown: &[String], wanted: &str) -> bool {
        shown.iter().any(|t| t == wanted)
    }

    #[test]
    fn the_table_has_every_column_of_the_mockup_row() {
        let shown = texts(&context(facts(true)));
        for heading in [
            "#",
            "Name",
            "Type",
            "Material",
            "Thickness (mm)",
            "Weight (oz)",
            "Dk",
            "Df",
            "Net",
            "Cu %",
            "Items",
            "Drill pairs",
        ] {
            assert!(has(&shown, heading), "{heading}: {shown:?}");
        }
        assert!(!has(&shown, "Er"), "Dk replaces Er");
    }

    #[test]
    fn copper_rows_show_their_place_items_nets_and_drill_pairs() {
        let shown = texts(&context(facts(true)));
        for place in ["1", "2", "3", "4"] {
            assert!(has(&shown, place), "copper place {place}: {shown:?}");
        }
        for items in ["412", "2", "3", "96"] {
            assert!(has(&shown, items), "items {items}");
        }
        assert!(has(&shown, "GND") && has(&shown, "+3V3"), "plane nets");
        assert_eq!(
            shown.iter().filter(|t| *t == "signal").count(),
            2,
            "both signal layers"
        );
        assert!(has(&shown, "1–4, 1–2"), "Top starts both pairs: {shown:?}");
        assert!(has(&shown, "1–2"), "Mid 1 ends the blind pair");
        assert!(has(&shown, "1–4"), "Bottom ends the through pair");
    }

    #[test]
    fn under_the_table_the_legend_hint_and_layer_pairs_show() {
        let shown = texts(&context(facts(true)));
        assert!(has(&shown, "Through 1–4") && has(&shown, "Blind 1–2"));
        assert!(
            shown.iter().any(|t| t.starts_with("Click a row's number")),
            "the edit hint: {shown:?}"
        );
        assert!(has(&shown, "Layer pairs"));
        assert!(has(&shown, "Top → Mid 1"), "{shown:?}");
        assert!(has(&shown, "Bottom → Mid 2"), "{shown:?}");
    }

    #[test]
    fn a_read_only_board_drops_the_edit_hint() {
        let mut read_only = facts(true);
        read_only.read_only = true;
        let shown = texts(&context(read_only));
        assert!(!shown.iter().any(|t| t.starts_with("Click a row's number")));
    }

    #[test]
    fn a_stack_without_planes_or_vias_says_so() {
        let mut plain = facts(false);
        plain.drill_pairs.clear();
        let shown = texts(&context(plain));
        assert!(has(&shown, "No plane layer to reference."), "{shown:?}");
        assert!(has(&shown, "No drill pairs: the board has no vias yet."));
        assert!(
            !shown
                .iter()
                .any(|t| t.contains('→') && t != "signal → reference plane")
        );
    }

    #[test]
    fn the_fab_profile_link_names_the_profile_and_opens_the_rules() {
        let ctx = context(facts(true));
        let shown = texts(&ctx);
        assert!(has(&shown, "Fab profile") && has(&shown, "Standard"));
        assert!(!has(&shown, "Open Rules"), "the link names the profile");
        let sent = messages_on_click(view(&ctx), WINDOW, "Standard");
        assert!(
            sent.iter().any(|m| matches!(m, PanelMsg::OpenRules)),
            "{sent:?}"
        );
    }

    #[test]
    fn a_project_without_a_profile_and_a_board_without_a_project_say_so() {
        let mut unnamed = facts(true);
        unnamed.fab_profile = None;
        assert!(has(&texts(&context(unnamed)), "None set"));

        let mut alone = facts(true);
        alone.has_project = false;
        alone.fab_profile = None;
        let ctx = context(alone);
        assert!(has(&texts(&ctx), "No project"));
        let sent = messages_on_click(view(&ctx), WINDOW, "No project");
        assert!(
            !sent.iter().any(|m| matches!(m, PanelMsg::OpenRules)),
            "inert without a project"
        );
    }
}
