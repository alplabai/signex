//! Graphite look of the New Project dialog's cards and stack-up (#2232).

use iced::Background;
use signex_types::theme::{ThemeId, theme_tokens};
use signex_widgets::button::Status;

use super::cards::{SELECTED_BORDER, card_style};
use super::preview::{Layer, stack_rows};
use crate::styles::{ti, tokens};

#[test]
fn the_selected_card_has_the_accent_border_and_the_soft_fill() {
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let t = theme_tokens(id);
        let r = tokens::resolve(&t);
        let on = card_style(&t, true, Status::Active);
        let off = card_style(&t, false, Status::Active);
        assert_eq!(on.border.width, SELECTED_BORDER, "{id:?}");
        assert_eq!(on.border.color, ti(t.accent), "{id:?}");
        // Picked: the soft accent at 40 % over the panel (#2537 mockup).
        let soft = iced::Color {
            a: r.accent_soft.a * 0.4,
            ..r.accent_soft
        };
        assert_eq!(
            on.background,
            Some(Background::Color(tokens::composite_over(
                soft,
                ti(t.panel_bg)
            ))),
            "{id:?}"
        );
        assert_eq!(off.border.width, 1.0, "{id:?}");
        assert_eq!(off.border.color, ti(t.border), "{id:?}");
        assert_eq!(
            off.background,
            Some(Background::Color(ti(t.panel_bg))),
            "{id:?}"
        );
        assert_ne!(on.background, off.background, "{id:?}");
    }
}

#[test]
fn a_hovered_unselected_card_takes_the_hover_fill_and_a_selected_one_keeps_the_accent() {
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let t = theme_tokens(id);
        let rest = card_style(&t, false, Status::Active);
        let hover = card_style(&t, false, Status::Hovered);
        assert_eq!(
            hover.background,
            Some(Background::Color(tokens::composite_over(
                ti(t.hover),
                ti(t.panel_bg)
            ))),
            "{id:?}"
        );
        assert_eq!(rest.border.color, hover.border.color, "{id:?}");
        assert_eq!(
            card_style(&t, true, Status::Hovered).border.color,
            ti(t.accent)
        );
    }
}

/// #2537: the layer glyph draws one copper bar per layer up to ten, with
/// thinner bars and a tighter gap above six layers.
#[test]
fn the_layer_glyph_thins_and_caps_its_bars() {
    use super::card_parts::{glyph_bars, glyph_metrics};
    assert_eq!(glyph_metrics(4), (3.0, 2.0, 2.0));
    assert_eq!(glyph_metrics(6), (3.0, 2.0, 1.0));
    assert_eq!(glyph_metrics(8), (2.0, 1.0, 0.0));
    assert_eq!(glyph_bars(2), 2);
    assert_eq!(glyph_bars(8), 8);
    assert_eq!(glyph_bars(32), 10);
}

/// #2538: the summary lines read like the design.
#[test]
fn the_summary_lines_name_the_tier_layers_and_rules() {
    use crate::app::SetupKind;
    use signex_types::project::{ProjectSetup, SetupProfile};
    let (app, _t) = crate::app::Signex::new();
    let i18n = &app.ui_state.i18n;
    let setup = ProjectSetup::from_profile(SetupProfile::Standard);
    assert_eq!(
        plain(super::summary::summary_line(
            &setup,
            SetupKind::Pcb,
            &[],
            i18n
        )),
        "Standard · 4 layers · 0.15 mm clearance · 0.2 mm tracks"
    );
    let mut sheet = setup.clone();
    sheet.sheet.paper_size = "A4".into();
    assert_eq!(
        plain(super::summary::summary_line(
            &sheet,
            SetupKind::Sheet,
            &[],
            i18n
        )),
        "A4 landscape · Signex default title block · IEEE 315"
    );
}

#[test]
fn the_stack_up_has_a_copper_layer_per_count_and_a_dielectric_between() {
    for copper in 1..=32u8 {
        let rows = stack_rows(copper);
        let coppers = rows
            .iter()
            .filter(|l| {
                matches!(
                    l,
                    Layer::TopCopper | Layer::InnerCopper(_) | Layer::BottomCopper
                )
            })
            .count();
        let between = rows
            .iter()
            .filter(|l| matches!(l, Layer::Prepreg | Layer::Core))
            .count();
        assert_eq!(coppers, usize::from(copper), "{copper} layers");
        assert_eq!(between, usize::from(copper) - 1, "{copper} layers");
        assert_eq!(rows.first(), Some(&Layer::Silk));
        assert_eq!(rows.last(), Some(&Layer::MaskBottom));
    }
    assert_eq!(
        stack_rows(4),
        [
            Layer::Silk,
            Layer::Mask,
            Layer::TopCopper,
            Layer::Prepreg,
            Layer::InnerCopper(1),
            Layer::Core,
            Layer::InnerCopper(2),
            Layer::Prepreg,
            Layer::BottomCopper,
            Layer::MaskBottom,
        ]
    );
}

#[test]
fn the_dialog_builds_closed_open_and_with_many_layers() {
    let (mut app, _t) = crate::app::Signex::new();
    let _ = app.update(crate::app::Message::Menu(
        crate::menu_bar::MenuMessage::NewProject,
    ));
    let _ = app.view_new_project_dialog();
    app.ui_state.new_project.as_mut().unwrap().customize_open = true;
    let _ = app.view_new_project_dialog();
    app.ui_state.new_project.as_mut().unwrap().step_layers(30);
    let _ = app.view_new_project_dialog();
}

/// Mockup row 22: a card's blurb wraps inside a narrow card instead of
/// running past its edge.
#[test]
fn a_profile_card_blurb_wraps_inside_a_narrow_card() {
    let (mut app, _t) = crate::app::Signex::new();
    let _ = app.update(crate::app::Message::Menu(
        crate::menu_bar::MenuMessage::NewProject,
    ));
    let state = app.ui_state.new_project.as_ref().expect("dialog open");
    let i18n = signex_i18n::I18n::en_us_ref();
    let body = crate::styles::TypeScale::DEFAULT.body;
    const ROOM: f32 = 460.0;
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let t = theme_tokens(id);
        let grid = super::cards::tier_grid(state, &[], i18n, &t);
        let texts = crate::layout_probe::lay_out_texts(grid, iced::Size::new(ROOM, 800.0));
        let blurb = crate::layout_probe::find(&texts, "Fine-pitch BGAs, laser microvias.").bounds;
        assert!(blurb.height >= 2.0 * body, "{id:?}: {blurb:?}");
        assert!(blurb.x + blurb.width <= ROOM / 2.0, "{id:?}: {blurb:?}");
    }
}

/// A rendered message without Fluent's bidi isolation marks.
fn plain(text: signex_i18n::Localized) -> String {
    text.into_string()
        .chars()
        .filter(|c| !matches!(c, '\u{2068}' | '\u{2069}'))
        .collect()
}

fn edit_name(app: &mut crate::app::Signex, name: &str) {
    let _ = app.update(crate::app::Message::from(
        crate::app::new_project::NewProjectMsg::NameEdited(name.into()),
    ));
}

#[test]
fn the_folder_label_follows_the_typed_name_and_is_neutral_when_empty() {
    let (mut app, _t) = crate::app::Signex::new();
    let _ = app.update(crate::app::Message::Menu(
        crate::menu_bar::MenuMessage::NewProject,
    ));
    let label = |app: &crate::app::Signex| {
        plain(super::make_folder_label(
            app.ui_state.new_project.as_ref().unwrap(),
            &app.ui_state.i18n,
        ))
    };
    edit_name(&mut app, "Motor Driver Rev A");
    assert_eq!(label(&app), "Create \"Motor Driver Rev A\" folder");
    edit_name(&mut app, "Motor Driver Rev B ");
    assert_eq!(label(&app), "Create \"Motor Driver Rev B\" folder");
    edit_name(&mut app, "  ");
    assert_eq!(label(&app), "Create a folder named after the project");
}

#[test]
fn a_taken_target_shows_its_translated_reason() {
    use crate::app::new_project::{TargetProblem, TargetStatus};
    let (mut app, _t) = crate::app::Signex::new();
    let _ = app.update(crate::app::Message::Menu(
        crate::menu_bar::MenuMessage::NewProject,
    ));
    let path = std::path::PathBuf::from("/work/Busy");
    let state = app.ui_state.new_project.as_mut().unwrap();
    state.name = "Busy".into();
    state.target = TargetStatus::Problem(TargetProblem::FolderNotEmpty(path.clone()));
    let state = app.ui_state.new_project.as_ref().unwrap();
    let shown = iced::widget::text::IntoFragment::into_fragment(
        super::blocker_text(state, &app.ui_state.i18n).expect("a reason"),
    )
    .into_owned();
    let expected = signex_i18n::t!(
        app.ui_state.i18n,
        signex_i18n::ids::DIALOG_NEW_PROJECT_ERR_FOLDER_NOT_EMPTY,
        "path" => path.display().to_string()
    )
    .into_string();
    assert_eq!(shown, expected);
}
