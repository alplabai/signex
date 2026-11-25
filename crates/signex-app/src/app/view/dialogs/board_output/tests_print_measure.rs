//! Temporary layout measurements for the Pages control (to be removed).
//! Prints only; asserts nothing. Registered from `print.rs` so it can
//! reach the private Layout group pieces.

use super::*;
use crate::app::{BoardPrintState, Message};
use crate::layout_probe::{lay_out_texts, natural, root_size};
use iced::Size;
use signex_i18n::{I18n, MessageId, ids, t};
use iced::Widget as _;
use iced::widget::column;
use signex_types::theme::{ThemeId, theme_tokens};
use std::path::PathBuf;

const HUGE: Size = Size::new(100_000.0, 1_000.0);

fn width_of(element: Element<'_, Message>, window: Size) -> f32 {
    root_size(element, window).width
}

fn plain(label: &str, font: iced::Font) -> Element<'static, Message> {
    text(signex_widgets::raw(label.to_owned()))
        .size(TS.body)
        .font(font)
        .wrapping(iced::widget::text::Wrapping::None)
        .into()
}

/// The segment face as `segmented` builds it: Fill height, centred, no
/// wrapping, ellipsis at the end.
fn face(label: &str, font: iced::Font) -> Element<'static, Message> {
    text(signex_widgets::raw(label.to_owned()))
        .size(TS.body)
        .font(font)
        .height(Length::Fill)
        .align_y(iced::alignment::Vertical::Center)
        .align_x(iced::alignment::Horizontal::Center)
        .wrapping(iced::widget::text::Wrapping::None)
        .ellipsis(iced::widget::text::Ellipsis::End)
        .into()
}

fn pages_control(style: SegmentedStyle, i18n: &I18n, only_first: bool) -> Element<'static, Message> {
    let mut options = vec![(
        BoardPrintPages::PerSet,
        Label::from(t!(i18n, ids::BOARD_PRINT_PAGES_PER_SET)),
    )];
    if !only_first {
        options.push((
            BoardPrintPages::Composite,
            Label::from(t!(i18n, ids::BOARD_PRINT_PAGES_COMPOSITE)),
        ));
    }
    segmented(options, BoardPrintPages::PerSet, style, |pages| {
        out(BoardOutputMsg::PrintPagesPicked(pages))
    })
}

#[test]
fn measure_pages_row() {
    let i18n = I18n::en_us();
    let tokens = theme_tokens(ThemeId::BUILTINS[0]);
    let word = |id: MessageId| t!(i18n, id).into_string();
    let regular = signex_widgets::ui_font::at(iced::font::Weight::Normal);
    let medium = signex_widgets::ui_font::at(iced::font::Weight::Medium);

    let compact = kit::segmented_style(&tokens).fill_width(true).compact(true);
    let hug = kit::segmented_style(&tokens).fill_width(false).compact(true);
    let m = SegmentedMetrics::from_text_size(compact.text_size);
    println!("== constants");
    println!("TS.body={} SP.xs={} SP.s={} SP.l={} SP.xl={}", TS.body, SP.xs, SP.s, SP.l, SP.xl);
    println!(
        "segment_pad(compact)={} track_pad={} segment_gap={} track_height={}",
        compact.segment_pad(),
        m.track_pad,
        m.gap,
        m.track_height
    );

    println!("== B1 natural label text (plain text, no ellipsis)");
    for id in [ids::BOARD_PRINT_PAGES_PER_SET, ids::BOARD_PRINT_PAGES_COMPOSITE] {
        let w = word(id);
        let r = natural(plain(&w, regular), &w).width;
        let md = natural(plain(&w, medium), &w).width;
        println!("{w:?}: regular={r:.2} medium={md:.2}");
    }

    println!("== B2 segment face (Fill height + ellipsis) as laid out alone");
    for id in [ids::BOARD_PRINT_PAGES_PER_SET, ids::BOARD_PRINT_PAGES_COMPOSITE] {
        let w = word(id);
        for window in [HUGE, Size::new(300.0, 100.0), Size::new(120.0, 100.0)] {
            let at = lay_out_texts(face(&w, medium), window);
            let root = width_of(face(&w, medium), window);
            println!(
                "{w:?} window_w={}: text_bounds_w={:.2} root_w={root:.2}",
                window.width,
                at.first().map_or(-1.0, |t| t.bounds.width)
            );
        }
    }

    println!("== B3 one-segment control, hugging (what FitRow measures as a cell natural)");
    for window_w in [100_000.0, 300.0, 241.98] {
        let w = width_of(pages_control(hug, &i18n, true), Size::new(window_w, 1_000.0));
        println!("Per Set alone in window {window_w}: control_w={w:.2} (cell = control - 2*track_pad = {:.2})", w - 2.0 * m.track_pad);
    }

    println!("== B4 whole Pages control unconstrained");
    let hug_w = width_of(pages_control(hug, &i18n, false), HUGE);
    let hug_h = root_size(pages_control(hug, &i18n, false), HUGE).height;
    println!("hugging style: w={hug_w:.2} h={hug_h:.2} (one line when h ~ track_height {})", m.track_height);
    let fill = root_size(pages_control(compact, &i18n, false), HUGE);
    println!("fill style in 100000 window: w={:.2} h={:.2}", fill.width, fill.height);
    for t in lay_out_texts(pages_control(hug, &i18n, false), HUGE) {
        println!("  hugging text {:?} {:?}", t.text, t.bounds);
    }

    println!("== B5 label column and gap");
    let mut widest: f32 = 0.0;
    for id in [ids::BOARD_PRINT_PAGES, ids::BOARD_PRINT_SCALE, ids::BOARD_PRINT_INK] {
        let w = word(id);
        let el: Element<'_, Message> = text(t!(i18n, id)).size(TS.body).into();
        let n = natural(el, &w).width;
        widest = widest.max(n);
        println!("label {w:?}: {n:.2}");
    }
    let grid_gap = SP.l - SP.xs;
    println!("label column (widest) = {widest:.2}; grid gap (SP.l - SP.xs) = {grid_gap}; line gap = {}", SP.s + SP.xs);

    println!("== B6 width available to the control");
    let side = SP.xl + SP.xs;
    let column_w = OPTIONS_W - 2.0 * side;
    let available = column_w - widest - grid_gap;
    println!("OPTIONS_W={OPTIONS_W:.2} side padding={side} options content={column_w:.2}");
    println!("available to the control = content - label col - gap = {available:.2}");
    println!(
        "need (hugging control) = {hug_w:.2}; inner available to FitRow = {:.2}",
        available - 2.0 * m.track_pad
    );
    for w in [available, column_w, 400.0] {
        let s = root_size(pages_control(compact, &i18n, false), Size::new(w, 1_000.0));
        println!("fill control in window {w:.2}: {s:?}");
    }

    println!("== B7 the real dialog");
    let state = BoardPrintState {
        board: PathBuf::from("dcdc.snxpcb"),
        board_name: "dcdc.snxpcb".into(),
        options: BoardPrintOptions {
            sets: vec![BoardLayerSet::Top],
            ..BoardPrintOptions::default()
        },
    };
    let (header, body, footer) = parts(&state, None, &i18n, &tokens);
    let laid = lay_out_texts(column![header, body, footer].boxed(), Size::new(780.0, 600.0));
    let wanted = [
        ids::BOARD_PRINT_PAGES,
        ids::BOARD_PRINT_PAGES_PER_SET,
        ids::BOARD_PRINT_PAGES_COMPOSITE,
        ids::BOARD_PRINT_SCALE,
        ids::BOARD_PRINT_SCALE_FIT,
        ids::BOARD_PRINT_SCALE_ONE,
        ids::BOARD_PRINT_INK,
        ids::BOARD_PRINT_INK_COLOUR,
        ids::BOARD_PRINT_INK_MONO,
    ];
    for id in wanted {
        let w = word(id);
        for t in laid.iter().filter(|t| t.text == w) {
            println!("  dialog {:?} {:?}", t.text, t.bounds);
        }
    }
}
