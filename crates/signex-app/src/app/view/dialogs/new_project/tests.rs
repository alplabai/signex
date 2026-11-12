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
        assert_eq!(
            on.background,
            Some(Background::Color(r.accent_soft)),
            "{id:?}"
        );
        assert_eq!(off.border.width, 1.0, "{id:?}");
        assert_eq!(off.border.color, ti(t.border), "{id:?}");
        assert_eq!(off.background, Some(Background::Color(r.raised)), "{id:?}");
        assert_ne!(on.background, off.background, "{id:?}");
    }
}

#[test]
fn a_hovered_unselected_card_strengthens_its_hairline_and_a_selected_one_keeps_the_accent() {
    let t = theme_tokens(ThemeId::GraphiteLight);
    let rest = card_style(&t, false, Status::Active);
    let hover = card_style(&t, false, Status::Hovered);
    assert_ne!(rest.border.color, hover.border.color);
    assert_eq!(
        card_style(&t, true, Status::Hovered).border.color,
        ti(t.accent)
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
