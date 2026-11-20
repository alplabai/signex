use iced::Element;
use signex_i18n::{I18n, UserText};
use signex_types::coord::Unit;
use signex_types::theme::{ThemeId, theme_tokens};

use super::*;

#[derive(Debug, Clone, PartialEq)]
enum Msg {
    GroupToggled(u8),
    MemberSelected(&'static str),
}

fn member(key: &'static str, length: f64, is_selected: bool) -> Member<'static, &'static str> {
    Member {
        key,
        name: UserText::new(key).into(),
        caption: None,
        length,
        target: 28.5,
        tolerance: 0.25,
        is_selected,
    }
}

fn header() -> GroupHeader<'static> {
    GroupHeader {
        name: UserText::new("DDR_DQ0-7").into(),
        captions: vec![UserText::new("28.500 ± 0.250").into()],
        status: Some(StatusPill {
            label: UserText::new("1").into(),
            tone: StatusTone::Err,
        }),
    }
}

fn group(is_open: bool) -> CompactGroup<'static, u8, &'static str, Msg> {
    CompactGroup::new(
        7,
        header(),
        vec![
            member("DDR_DQ0", 28.40, false),
            member("DDR_DQ6", 28.91, true),
        ],
        is_open,
        Msg::GroupToggled,
        Msg::MemberSelected,
    )
}

#[test]
fn the_header_sends_the_toggle_of_its_group() {
    assert_eq!(group(true).toggle_message(), Msg::GroupToggled(7));
    assert_eq!(group(false).toggle_message(), Msg::GroupToggled(7));
}

#[test]
fn a_row_sends_the_select_of_its_member() {
    let g = group(true);
    assert_eq!(g.select_message(0), Some(Msg::MemberSelected("DDR_DQ0")));
    assert_eq!(g.select_message(1), Some(Msg::MemberSelected("DDR_DQ6")));
    assert_eq!(g.select_message(2), None);
}

#[test]
fn members_report_their_delta_and_band() {
    let short = member("A", 28.40, false);
    let long = member("B", 28.91, false);
    assert!((short.delta() + 0.10).abs() < 1e-9);
    assert!(!short.is_out_of_band());
    assert!(long.is_out_of_band());
    assert_eq!(out_of_band_count(&[short, long]), 1);
}

#[test]
fn metrics_follow_the_spec_at_twelve_px_and_scale() {
    let m = GroupMetrics::new(12.0);
    assert_eq!(m.header_height, 34.0);
    assert_eq!(m.header_pad, (4.0, 10.0));
    assert_eq!(m.header_gap, 12.0);
    assert_eq!(m.row_height, 28.0);
    assert_eq!(m.row_pad, 10.0);
    assert_eq!(m.accent_bar, 2.0);
    assert!(GroupMetrics::new(13.0).row_height > m.row_height);
}

#[test]
fn selection_hover_and_pills_are_distinct_on_both_themes() {
    for id in ThemeId::BUILTINS {
        let s = CompactGroupStyle::from_tokens(&theme_tokens(*id), 12.0);
        assert_ne!(s.accent_soft, s.hover, "{id:?}: selected reads as hovered");
        assert_ne!(s.ok_pill.ink, s.err_pill.ink, "{id:?}");
        assert_ne!(s.sunken, s.line_soft, "{id:?}");
    }
}

#[test]
fn builds_open_and_collapsed() {
    let style = CompactGroupStyle::from_tokens(&theme_tokens(ThemeId::DEFAULT), 12.0);
    let mm = LengthFormat::new(I18n::en_us_ref(), Unit::Mm);
    let _: Element<'_, Msg> = group(true).view(&mm, style);
    let _: Element<'_, Msg> = group(false).view(&mm, style);
}
