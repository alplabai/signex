//! m116 on screen: the page plan's cards follow `page_plan`, an empty plan
//! disables Export PDF, and Mirror the Bottom Side takes clicks only while
//! a bottom set is ticked.

use std::path::PathBuf;

use iced::widget::column;
use iced::{Size, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_output::{BoardLayerSet, BoardPrintOptions, BoardPrintPages};
use signex_types::pcb::PcbBoard;
use signex_types::theme::{ThemeId, theme_tokens};

use super::print::parts;
use super::print_plan::{page_cards, unticked_sets};
use crate::app::{BoardOutputMsg, BoardPrintFlag, BoardPrintState, Message, PcbMsg};
use crate::layout_probe::{find, lay_out_texts, messages_on_click, messages_on_click_at};

const WINDOW: Size = Size::new(780.0, 600.0);

fn state(sets: &[BoardLayerSet]) -> BoardPrintState {
    BoardPrintState {
        board: PathBuf::from("dcdc.snxpcb"),
        board_name: "dcdc.snxpcb".into(),
        options: BoardPrintOptions {
            sets: sets.to_vec(),
            ..BoardPrintOptions::default()
        },
    }
}

fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": uuid::Uuid::nil(),
        "layers": [
            {"id": 0, "name": "TopCopper", "layer_type": "signal"},
            {"id": 1, "name": "BottomCopper", "layer_type": "signal"},
            {"id": 2, "name": "TopSilkscreen", "layer_type": "user"},
            {"id": 3, "name": "EdgeCuts", "layer_type": "user"}
        ]
    }))
    .expect("a board")
}

/// What clicking `label` (or the point `at`) in the dialog sends.
fn clicks(
    state: &BoardPrintState,
    i18n: &I18n,
    label: Option<&str>,
    at: iced::Point,
) -> Vec<Message> {
    let tokens = theme_tokens(ThemeId::BUILTINS[0]);
    let (header, body, footer) = parts(state, None, i18n, &tokens);
    let element = column![header, body, footer].boxed();
    match label {
        Some(label) => messages_on_click(element, WINDOW, label),
        None => messages_on_click_at(element, WINDOW, None, at),
    }
}

fn texts(state: &BoardPrintState, i18n: &I18n) -> Vec<crate::layout_probe::LaidOutText> {
    let tokens = theme_tokens(ThemeId::BUILTINS[0]);
    let (header, body, footer) = parts(state, None, i18n, &tokens);
    lay_out_texts(column![header, body, footer].boxed(), WINDOW)
}

fn sent_export(sent: &[Message]) -> bool {
    sent.iter().any(|m| {
        matches!(
            m,
            Message::Pcb(PcbMsg::Output(BoardOutputMsg::PdfExportRequested))
        )
    })
}

#[test]
fn the_cards_follow_the_page_plan() {
    let i18n = I18n::en_us();
    let board = board();
    let mut options = state(&[
        BoardLayerSet::Top,
        BoardLayerSet::Bottom,
        BoardLayerSet::AssemblyTop,
    ])
    .options;
    let cards = page_cards(&options, Some(&board), &i18n);
    let plan = options.page_plan();
    assert_eq!(cards.len(), plan.len());
    for (card, (sets, mirrored)) in cards.iter().zip(&plan) {
        assert_eq!(card.mirrored, *mirrored);
        let first = t!(i18n, crate::app::board_output::set_label(sets[0])).into_string();
        assert!(card.title.starts_with(&first), "{card:?}");
    }
    assert!(cards[1].mirrored, "the bottom page is mirrored");
    assert!(
        !cards[0].mirrored && !cards[2].mirrored,
        "only bottom pages"
    );
    let top_copper = crate::pcb_canvas::layer_style::layer_name(
        &i18n,
        signex_types::pcb_layer::PcbLayer::Copper(signex_types::pcb_layer::CopperLayer::Top),
    )
    .into_string();
    assert!(cards[0].layers.contains(&top_copper), "{:?}", cards[0]);
    assert!(
        unticked_sets(&options, &i18n).is_some(),
        "three sets are left for the last card"
    );

    options.pages = BoardPrintPages::Composite;
    let cards = page_cards(&options, Some(&board), &i18n);
    assert_eq!(cards.len(), options.page_plan().len());
    assert_eq!(
        cards.len(),
        2,
        "the layer sets on one page, the drawing alone"
    );

    options.mirror_bottom = false;
    options.pages = BoardPrintPages::PerSet;
    assert!(
        page_cards(&options, None, &i18n)
            .iter()
            .all(|c| !c.mirrored),
        "no mirrored tag when mirroring is off"
    );
}

#[test]
fn an_empty_plan_disables_export_pdf() {
    let i18n = I18n::en_us();
    let export = t!(i18n, ids::BOARD_PRINT_EXPORT).into_string();
    let ticked = state(&[BoardLayerSet::Top]);
    assert!(sent_export(&clicks(
        &ticked,
        &i18n,
        Some(&export),
        iced::Point::ORIGIN
    )));
    let empty = state(&[]);
    assert!(page_cards(&empty.options, None, &i18n).is_empty());
    assert!(
        !sent_export(&clicks(&empty, &i18n, Some(&export), iced::Point::ORIGIN)),
        "nothing to print, Export PDF… is disabled"
    );
    let laid = texts(&empty, &i18n);
    find(&laid, &t!(i18n, ids::BOARD_PRINT_NO_SETS).into_string());
}

#[test]
fn mirror_takes_clicks_only_with_a_bottom_set() {
    let i18n = I18n::en_us();
    let mirror = t!(i18n, ids::BOARD_PRINT_MIRROR_BOTTOM).into_string();
    let live = state(&[BoardLayerSet::Top, BoardLayerSet::Bottom]);
    let at = find(&texts(&live, &i18n), &mirror).bounds.center();
    let toggled = |sent: &[Message]| {
        sent.iter().any(|m| {
            matches!(
                m,
                Message::Pcb(PcbMsg::Output(BoardOutputMsg::PrintFlagToggled(
                    BoardPrintFlag::MirrorBottom
                )))
            )
        })
    };
    assert!(toggled(&clicks(&live, &i18n, None, at)));
    let deaf = state(&[BoardLayerSet::Top]);
    assert!(
        !toggled(&clicks(&deaf, &i18n, None, at)),
        "greyed while no bottom set is ticked"
    );
}
