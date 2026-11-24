use iced::widget::Space;
use iced::{Element, Widget};
use signex_i18n::UserText;
use signex_types::theme::{ThemeId, theme_tokens};

use super::*;
use crate::Label;

#[derive(Debug, Clone, PartialEq)]
enum Msg {
    SubTabPicked(usize),
}

fn label(s: &str) -> Label<'static> {
    UserText::new(s.to_owned()).into()
}

fn style(id: ThemeId) -> DocTabShellStyle {
    DocTabShellStyle::from_tokens(&theme_tokens(id), DEFAULT_BASE_SIZE)
}

fn four_tabs<'a>() -> Vec<SubTab<'a, Msg>> {
    vec![
        SubTab::new(label("Stack")),
        SubTab::new(label("Materials")),
        SubTab::new(label("Impedance")).tag(Space::new().width(4.0).boxed()),
        SubTab::new(label("Back Drill")),
    ]
}

fn body<'a>() -> Element<'a, Msg> {
    Space::new().boxed()
}

#[test]
fn the_selected_sub_tab_is_the_only_active_one() {
    let tabs = SubTabs::new(four_tabs(), 2).on_select(Msg::SubTabPicked);
    let active: Vec<bool> = (0..tabs.len()).map(|i| tabs.is_active(i)).collect();
    assert_eq!(active, [false, false, true, false]);
    assert!(!tabs.is_active(9), "an index past the end is never active");
}

#[test]
fn a_press_sends_the_callers_message_with_the_index() {
    let tabs = SubTabs::new(four_tabs(), 0).on_select(Msg::SubTabPicked);
    assert_eq!(tabs.message_for(1), Some(Msg::SubTabPicked(1)));
    assert_eq!(tabs.message_for(3), Some(Msg::SubTabPicked(3)));
    assert_eq!(
        tabs.message_for(0),
        Some(Msg::SubTabPicked(0)),
        "the active one sends too; the caller decides"
    );
    assert_eq!(tabs.message_for(4), None, "past the end");
    let inert = SubTabs::<Msg>::new(four_tabs(), 0);
    assert_eq!(inert.message_for(1), None, "no handler, no press");
}

#[test]
fn the_active_sub_tab_is_ink_medium_and_underlined_on_both_themes() {
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let s = style(id);
        let tabs = SubTabs::new(four_tabs(), 1).on_select(Msg::SubTabPicked);
        let on = tabs.look(1, &s);
        let off = tabs.look(0, &s);
        assert_eq!(on.ink, s.ink, "{id:?}");
        assert!(on.is_medium && on.has_underline, "{id:?}");
        assert_eq!(off.ink, s.ink2, "{id:?}");
        assert!(!off.is_medium && !off.has_underline, "{id:?}");
        assert_ne!(s.ink, s.ink2, "{id:?}: active and idle read alike");
        assert_ne!(s.chrome, s.border, "{id:?}: the bands lose their edge");
    }
}

#[test]
fn separators_are_drawn_only_between_items() {
    let item = || Space::new().boxed();
    let toolbar = Toolbar::<Msg>::new()
        .separator()
        .push(item())
        .separator()
        .separator()
        .push(item())
        .separator()
        .push_end(item());
    assert_eq!(toolbar.separator_count(), 1);
    assert_eq!(toolbar.start_len(), 3, "item, separator, item");
    assert_eq!(toolbar.end_len(), 1);

    let one = Toolbar::<Msg>::new().push(item()).separator().push(item());
    assert_eq!(one.separator_count(), 1, "the separator is present");
}

#[test]
fn the_footer_omits_an_empty_note() {
    let plain = Footer::new(label("4 copper layers · 13 rows"));
    assert!(plain.has_status());
    assert!(!plain.has_note());
    let blank = Footer::new(label("13 rows")).note(label(""));
    assert!(!blank.has_note(), "an empty note is not drawn");
    let noted = Footer::new(label("13 rows")).note(label("Changes apply on save."));
    assert!(noted.has_note());
    assert!(!Footer::new(label("")).has_status());
}

#[test]
fn metrics_match_the_design_at_the_default_body_and_scale_with_it() {
    let near = |a: f32, b: f32| (a - b).abs() < 1e-3;
    let m = DocTabMetrics::default();
    assert!(near(m.sub_height, 30.0), "{}", m.sub_height);
    assert!(near(m.footer_min_height, 26.0), "{}", m.footer_min_height);
    assert!(near(m.separator_height, 16.0));
    assert!(near(m.underline_inset, 6.0));
    assert_eq!(m.pad_x, Spacing::DEFAULT.m);
    assert_eq!(m.caption_size, TypeScale::DEFAULT.caption);
    let big = DocTabMetrics::from_text_size(DEFAULT_BASE_SIZE * 1.5);
    assert!(near(big.sub_height, 45.0), "{}", big.sub_height);
    assert!(big.toolbar_gap > m.toolbar_gap);
}

#[test]
fn the_shell_builds_with_every_slot_and_with_the_body_alone() {
    let s = style(ThemeId::GraphiteLight);
    let full = DocTabShell::new(body(), s)
        .toolbar(
            Toolbar::new()
                .push(body())
                .separator()
                .push(body())
                .push_end(body()),
        )
        .sub_tabs(SubTabs::new(four_tabs(), 0).on_select(Msg::SubTabPicked))
        .footer(Footer::new(label("13 rows")).note(label("Saved with the board.")));
    assert_eq!(full.slots(), (true, true, true));
    let _: Element<'_, Msg> = full.into();
    let bare = DocTabShell::new(body(), style(ThemeId::GraphiteDark));
    assert_eq!(bare.slots(), (false, false, false));
    let _: Element<'_, Msg> = bare.view();
}
