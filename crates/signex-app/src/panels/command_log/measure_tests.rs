//! Temporary layout measurements for the filter row (to be removed).
//! Prints only; asserts nothing. Registered from `toolbar.rs` so it can
//! reach the private toolbar pieces.

use super::*;
use crate::layout_probe::{LaidOutText, lay_out_texts, root_size};
use crate::panels::command_log::{LogOutcome, LogRow, LogSource};
use iced::Size;
use signex_i18n::{I18n, ids, t};
use std::time::Duration;

const HUGE: Size = Size::new(100_000.0, 1_000.0);
const DOCK: f32 = 890.0;

fn context_with_one_row() -> PanelContext {
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_state.panel_ctx.clone();
    let log = &mut std::sync::Arc::make_mut(&mut ctx.command_log).log;
    let _seq = log.record(LogRow {
        seq: 0,
        at: chrono::Local::now(),
        source: LogSource::Gui { window: None },
        command: "undo".to_string(),
        label: "Undo".to_string(),
        operands: serde_json::Map::new(),
        outcome: LogOutcome::Failed("Nothing to undo".to_string()),
        took: Duration::ZERO,
        document: None,
        project: None,
        undoable: false,
        view_only: false,
        undone_at: None,
    });
    ctx
}

fn texts(element: Element<'_, PanelMsg>, window: Size) -> Vec<LaidOutText> {
    lay_out_texts(element, window)
        .into_iter()
        .map(|t| LaidOutText {
            text: t.text.replace(['\u{2068}', '\u{2069}'], ""),
            ..t
        })
        .collect()
}

fn w(element: Element<'_, PanelMsg>, window: Size) -> f32 {
    root_size(element, window).width
}

#[test]
fn measure_command_log_bar() {
    let ctx = context_with_one_row();
    let state: &CommandLogState = &ctx.command_log;
    let counts = state.log.counts();
    let tokens = &ctx.tokens;
    let i18n: &I18n = &ctx.i18n;

    let search_min = TS.body * 10.0;
    let bar_without_search = TS.body * 58.0;
    println!("== constants");
    println!(
        "TS.body={} TS.caption={} SP.xs={} SP.s={} SP.m={} HAIRLINE={HAIRLINE}",
        TS.body, TS.caption, SP.xs, SP.s, SP.m
    );
    println!(
        "SEARCH_MIN={search_min} BAR_WITHOUT_SEARCH=58*body={bar_without_search} ONE_ROW_FROM={} search_width(890)={}",
        search_min + bar_without_search,
        layout::search_width(DOCK)
    );
    println!("filters_on_one_row(890)={}", layout::filters_on_one_row(DOCK));

    println!("== C1 natural widths of the parts (laid out in a 100000 window)");
    let search = w(search_field(tokens, i18n, "", search_min), HUGE);
    let sep = w(vsep(tokens), HUGE);
    let source: Element<'_, PanelMsg> = text(t!(i18n, ids::COMMAND_LOG_SOURCE))
        .size(TS.caption)
        .into();
    let source = w(source, HUGE);
    let (style, metrics) = chip_look(tokens);
    let mut chips = Vec::new();
    for (kind, label, glyph) in [
        (
            SourceKind::Gui,
            ids::COMMAND_LOG_SOURCE_GUI,
            crate::icons::icon_log_mouse(GLYPH_THEME),
        ),
        (
            SourceKind::Cli,
            ids::COMMAND_LOG_SOURCE_CLI,
            crate::icons::icon_log_terminal(GLYPH_THEME),
        ),
        (
            SourceKind::Agent,
            ids::COMMAND_LOG_SOURCE_AGENT,
            crate::icons::icon_log_plug(GLYPH_THEME),
        ),
    ] {
        let count = u32::try_from(counts.of_source(kind)).unwrap_or(u32::MAX);
        let chip = icon_chip(
            glyph,
            t!(i18n, label),
            Some(count),
            state.log.filters.sources.contains(&kind),
            style,
            metrics,
            i18n,
            Some(message(CommandLogMsg::SourceToggled(kind))),
        );
        let cw = w(chip, HUGE);
        println!("chip {:?}: {cw:.2}", t!(i18n, label).into_string());
        chips.push(cw);
    }
    let result_huge = w(
        result_switch(tokens, i18n, state.log.filters.result, &counts),
        HUGE,
    );
    let result_400 = w(
        result_switch(tokens, i18n, state.log.filters.result, &counts),
        Size::new(400.0, 1_000.0),
    );
    let follow = w(follow_switch(&ctx, state), HUGE);
    println!("search field at SEARCH_MIN: {search:.2}");
    println!("vsep: {sep:.2}  Source label: {source:.2}");
    println!("result switch: {result_huge:.2} in 100000 window, {result_400:.2} in 400 window");
    println!("follow newest: {follow:.2}");
    for t in texts(
        result_switch(tokens, i18n, state.log.filters.result, &counts),
        HUGE,
    ) {
        println!("  result text {:?} {:?}", t.text, t.bounds);
    }
    for t in texts(follow_switch(&ctx, state), HUGE) {
        println!("  follow text {:?} {:?}", t.text, t.bounds);
    }

    println!("== C2 the sum against the 890 dock");
    let item_gap = SP.s + SP.xs;
    let items = [search, sep, source, chips[0], chips[1], chips[2], sep, result_huge];
    let filters = items.iter().sum::<f32>() + item_gap * (items.len() - 1) as f32;
    let follow_gap = SP.m + SP.xs;
    let pad = SP.m + SP.xs;
    let total = 2.0 * pad + filters + follow_gap + follow;
    println!("filters natural (search at SEARCH_MIN, {} items, gap {item_gap}) = {filters:.2}", items.len());
    println!("bar natural = 2*pad({pad}) + filters + follow gap({follow_gap}) + follow = {total:.2}");
    println!("bar without search = {:.2} (BAR_WITHOUT_SEARCH says {bar_without_search})", total - search);
    println!("dock = {DOCK}; slack at SEARCH_MIN = {:.2}", DOCK - total);
    let search_890 = layout::search_width(DOCK);
    println!(
        "bar natural with search_width(890)={search_890:.2}: {:.2} (slack {:.2})",
        total - search + search_890,
        DOCK - (total - search + search_890)
    );

    println!("== C3 the filter group and bar as laid out");
    let group_890 = w(filter_group(&ctx, state, &counts, false, DOCK), Size::new(DOCK, 1_000.0));
    println!("filter_group root width at 890: {group_890:.2}");
    for t in texts(bar(&ctx, state, &counts, false, DOCK), Size::new(DOCK, 180.0)) {
        println!("  bar text {:?} {:?}", t.text, t.bounds);
    }
}
