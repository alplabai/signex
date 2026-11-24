//! Graphite look of the New Project dialog's cards and stack-up (#2232).

use iced::Background;
use signex_types::theme::{ThemeId, theme_tokens};
use signex_widgets::button::Status;

use super::cards::{SELECTED_BORDER, card_style};
use super::preview::{Layer, stack_rows};
use crate::styles::{ti, tokens};

/// Mockup rows 21 and 22: the column and both larger views build, and the
/// drawing's Expand opens and Esc's message closes them.
#[test]
fn the_dialog_renders_its_previews_and_larger_views() {
    use crate::app::new_project::{DialogPreview, NewProjectMsg, SetupPreviewMsg};
    use crate::app::{Message, Signex};
    let (mut app, _task) = Signex::new();
    let _ = app.open_new_project_dialog();
    let _ = app.view_new_project_dialog();
    for view in [DialogPreview::BoardRules, DialogPreview::Sheet] {
        let open = NewProjectMsg::Preview(SetupPreviewMsg::Expanded(Some(view)));
        let _ = app.update(Message::from(open));
        let state = app.ui_state.new_project.as_ref().unwrap();
        assert_eq!(state.preview.expanded, Some(view));
        let _ = app.view_new_project_dialog();
        let close = NewProjectMsg::Preview(SetupPreviewMsg::Expanded(None));
        let _ = app.update(Message::from(close));
    }
    let state = app.ui_state.new_project.as_ref().unwrap();
    assert_eq!(state.preview.expanded, None);
}

/// Mockup row 22: only a board of a project with a saved setup says it
/// starts from that setup.
#[test]
fn the_new_pcb_intro_follows_whether_the_project_saved_a_setup() {
    use super::document_setup::intro_id;
    use crate::app::SetupKind;
    use signex_i18n::ids;
    assert_eq!(intro_id(SetupKind::Pcb, true), ids::SETUP_PCB_INTRO);
    assert_eq!(
        intro_id(SetupKind::Pcb, false),
        ids::SETUP_PCB_INTRO_NO_SETUP
    );
    assert_eq!(intro_id(SetupKind::Sheet, false), ids::SETUP_SHEET_INTRO);
}

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

/// The size picker names the size alone: Orientation carries the direction,
/// so a portrait sheet never reads "landscape" (mockup rows 21, 22, 31, 37).
#[test]
fn the_size_picker_names_the_size_alone() {
    let options = super::customize::size_options();
    assert_eq!(options.len(), signex_types::project::SHEET_SIZES.len());
    for (value, label) in options {
        assert_eq!(value, label);
        assert!(!label.contains("landscape"), "{label}");
    }
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
        assert_eq!(rows.last(), Some(&Layer::SilkBottom));
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
            Layer::SilkBottom,
        ]
    );
}

/// Mockup row 21: the drawn stack-up ends with Bottom Silkscreen, under
/// Bottom Solder.
#[test]
fn the_drawn_stack_up_ends_with_the_bottom_silkscreen() {
    let (mut app, _t) = crate::app::Signex::new();
    let _ = app.update(crate::app::Message::Menu(
        crate::menu_bar::MenuMessage::NewProject,
    ));
    let state = app.ui_state.new_project.as_ref().unwrap();
    let card = super::preview::stack_card(state, app.ui_state.i18n.as_ref());
    let texts = crate::layout_probe::lay_out_texts(card, iced::Size::new(400.0, 2000.0));
    let names: Vec<String> = texts.into_iter().map(|t| t.text).collect();
    let n = names.len();
    assert!(n >= 2, "{names:?}");
    assert_eq!(names[n - 2], "Bottom Solder", "{names:?}");
    assert_eq!(names[n - 1], "Bottom Silkscreen", "{names:?}");
}

/// Mockup row 21: the preview column holds the board rules drawing and its
/// padding, so the drawing, its expand button and the head's caption are
/// not cut at the right.
#[test]
fn the_preview_column_holds_the_drawing_and_its_caption() {
    use super::{PREVIEW_W, SP, TS};
    use crate::app::new_project::preview::VIEW_W;
    const { assert!(VIEW_W + 2.0 * SP.xl <= PREVIEW_W) };
    let (mut app, _t) = crate::app::Signex::new();
    let _ = app.update(crate::app::Message::Menu(
        crate::menu_bar::MenuMessage::NewProject,
    ));
    let state = app.ui_state.new_project.as_ref().unwrap();
    let pane = super::preview::pane(
        state,
        signex_i18n::ids::DIALOG_NEW_PROJECT_PREVIEW_NOTE,
        Some(&app.document_state.panel_ctx.canvas_colors),
        app.ui_state.i18n.as_ref(),
        &app.document_state.panel_ctx.tokens,
    );
    let texts = crate::layout_probe::lay_out_texts(pane, iced::Size::new(PREVIEW_W, 3000.0));
    let hint = crate::layout_probe::find(&texts, "click a value to edit").bounds;
    assert!(hint.x + hint.width <= PREVIEW_W - SP.xl + 0.5, "{hint:?}");
    assert!(hint.height < 1.8 * TS.caption, "{hint:?}");
}

/// The width a dialog of `dialog_w` leaves its left column, the cards' room.
fn left_column_w(dialog_w: f32) -> f32 {
    use super::{PREVIEW_W, SP};
    dialog_w - PREVIEW_W - 2.0 * (SP.xl + SP.m)
}

/// Mockup rows 21 and 22: at the New Project and New PCB widths the cards
/// sit two to a row with each blurb on one line.
#[test]
fn at_the_dialog_widths_each_card_blurb_stays_on_one_line() {
    use crate::app::SetupKind;
    let (mut app, _t) = crate::app::Signex::new();
    let _ = app.update(crate::app::Message::Menu(
        crate::menu_bar::MenuMessage::NewProject,
    ));
    let state = app.ui_state.new_project.as_ref().expect("dialog open");
    let i18n = app.ui_state.i18n.as_ref();
    let tokens = &app.document_state.panel_ctx.tokens;
    let body = crate::styles::TypeScale::DEFAULT.body;
    let widths = [
        super::super::sizes::NEW_PROJECT.0,
        super::document_setup::design_size(SetupKind::Pcb).0,
    ];
    for dialog_w in widths {
        let room = left_column_w(dialog_w);
        let grid = super::cards::tier_grid(state, &[], i18n, tokens);
        let texts = crate::layout_probe::lay_out_texts(grid, iced::Size::new(room, 1200.0));
        for blurb in [
            "Tighter rules for denser boards.",
            "Fine-pitch BGAs, laser microvias.",
        ] {
            let at = crate::layout_probe::find(&texts, blurb).bounds;
            assert!(at.height < 1.8 * body, "{dialog_w}: {blurb}: {at:?}");
            assert!(at.x + at.width <= room + 0.5, "{dialog_w}: {blurb}: {at:?}");
        }
    }
}

/// #2804, mockup row 22: a card title too long for its card is cut with an
/// ellipsis inside the card instead of running past its edge.
#[test]
fn a_card_title_stays_inside_a_narrow_card() {
    let (mut app, _t) = crate::app::Signex::new();
    let _ = app.update(crate::app::Message::Menu(
        crate::menu_bar::MenuMessage::NewProject,
    ));
    let state = app.ui_state.new_project.as_ref().expect("dialog open");
    let label = crate::styles::TypeScale::DEFAULT.label;
    // Too narrow for "Standard" beside the glyph, the pencil and the disc.
    const ROOM: f32 = 200.0;
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let t = theme_tokens(id);
        let grid = super::cards::tier_grid(state, &[], app.ui_state.i18n.as_ref(), &t);
        let texts = crate::layout_probe::lay_out_texts(grid, iced::Size::new(ROOM, 1200.0));
        let left = crate::layout_probe::find(&texts, "Standard").bounds;
        let right = crate::layout_probe::find(&texts, "Advanced").bounds;
        assert!(left.x + left.width <= ROOM / 2.0, "{id:?}: {left:?}");
        assert!(right.x + right.width <= ROOM + 0.5, "{id:?}: {right:?}");
        assert!(left.height < 1.8 * label, "{id:?}: {left:?}");
    }
}

/// DECISIONS 2026-11-23: a new project keeps A3 as its default sheet; the
/// A4 in mockup row 21 is a sample value. The preview says so too.
#[test]
fn a_new_project_previews_an_a3_landscape_sheet() {
    let (mut app, _t) = crate::app::Signex::new();
    let _ = app.update(crate::app::Message::Menu(
        crate::menu_bar::MenuMessage::NewProject,
    ));
    let state = app.ui_state.new_project.as_ref().unwrap();
    assert_eq!(state.setup.sheet.paper_size, "A3");
    let pane = super::preview::pane(
        state,
        signex_i18n::ids::DIALOG_NEW_PROJECT_PREVIEW_NOTE,
        Some(&app.document_state.panel_ctx.canvas_colors),
        app.ui_state.i18n.as_ref(),
        &app.document_state.panel_ctx.tokens,
    );
    let texts: Vec<String> =
        crate::layout_probe::lay_out_texts(pane, iced::Size::new(super::PREVIEW_W, 3000.0))
            .into_iter()
            .map(|t| {
                t.text
                    .chars()
                    .filter(|c| !matches!(c, '\u{2068}' | '\u{2069}'))
                    .collect()
            })
            .collect();
    assert!(texts.iter().any(|t| t == "A3 landscape sheet"), "{texts:?}");
    assert!(!texts.iter().any(|t| t.contains("A4")), "{texts:?}");
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

/// The texts of the name, location and folder block, as laid out.
fn field_texts(app: &crate::app::Signex) -> Vec<String> {
    let state = app.ui_state.new_project.as_ref().unwrap();
    let tokens = &app.document_state.panel_ctx.tokens;
    crate::layout_probe::lay_out_texts(
        super::fields(state, &app.ui_state.i18n, tokens),
        iced::Size::new(900.0, 400.0),
    )
    .into_iter()
    .map(|t| t.text.to_string())
    .collect()
}

/// Mockup row 21: the line follows the name, the location and the checkbox.
#[test]
fn the_path_line_follows_the_name_location_and_folder_choice() {
    use crate::app::Message;
    use crate::app::new_project::NewProjectMsg;
    let (mut app, _t) = crate::app::Signex::new();
    let _ = app.update(Message::Menu(crate::menu_bar::MenuMessage::NewProject));
    let work = std::path::Path::new("/work");
    let _ = app.update(Message::from(NewProjectMsg::LocationEdited(
        work.display().to_string(),
    )));
    edit_name(&mut app, " Motor ");
    let nested = work.join("Motor").join("Motor.snxprj");
    let flat = work.join("Motor.snxprj");
    let texts = field_texts(&app);
    assert!(texts.contains(&nested.display().to_string()), "{texts:?}");
    assert!(!texts.contains(&flat.display().to_string()), "{texts:?}");

    let _ = app.update(Message::from(NewProjectMsg::MakeFolderToggled(false)));
    let texts = field_texts(&app);
    assert!(texts.contains(&flat.display().to_string()), "{texts:?}");
    assert!(!texts.contains(&nested.display().to_string()), "{texts:?}");

    let other = std::path::Path::new("/other");
    let _ = app.update(Message::from(NewProjectMsg::LocationEdited(
        other.display().to_string(),
    )));
    edit_name(&mut app, "Pump");
    let moved = other.join("Pump.snxprj");
    let texts = field_texts(&app);
    assert!(texts.contains(&moved.display().to_string()), "{texts:?}");
}

/// An empty name is asked for once, no path is shown and Create stays off.
#[test]
fn an_empty_name_asks_for_a_name_once_and_create_stays_off() {
    use crate::app::Message;
    use crate::app::new_project::{NewProjectMsg, TargetStatus};
    let (mut app, _t) = crate::app::Signex::new();
    let _ = app.update(Message::Menu(crate::menu_bar::MenuMessage::NewProject));
    let _ = app.update(Message::from(NewProjectMsg::LocationEdited("/work".into())));
    for empty in ["", "   "] {
        edit_name(&mut app, empty);
        // Even a target reported clear does not switch Create on.
        app.ui_state.new_project.as_mut().unwrap().target = TargetStatus::Clear;
        let ask = signex_i18n::t!(
            app.ui_state.i18n,
            signex_i18n::ids::DIALOG_NEW_PROJECT_ERR_NAME_EMPTY
        )
        .into_string();
        let texts = field_texts(&app);
        assert_eq!(texts.iter().filter(|t| **t == ask).count(), 1, "{texts:?}");
        assert!(!texts.iter().any(|t| t.contains(".snxprj")), "{texts:?}");
        assert!(!app.ui_state.new_project.as_ref().unwrap().can_create());
    }
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
