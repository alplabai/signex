//! The Customize settings disclosure: PCB rules and schematic defaults,
//! filled in from the chosen profile.

use iced::widget::{Space, column, container, pick_list, row};
use iced::{Alignment, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::designator::DesignatorStandard;
use signex_types::project::{FabTemplate, PcbField, PcbValueError, SHEET_SIZES, SetupProfile};
use signex_types::schematic::{SheetOrientation, SheetSize};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{Label, PickListPlaceholder as _, button, segmented, text, text_input};

use super::{SP, TS, labelled_row};
use crate::app::new_project::{NewProjectMsg, NewProjectState, RULE_FIELDS};
use crate::styles::{Radii, ti, tokens};

/// Width of a value box (a rule field, the layer count).
const VALUE_W: f32 = 64.0;
/// Width of a schematic control (180 px): the Orientation segments' two
/// labels fit on one line.
const SHEET_CONTROL_W: f32 = TS.body * 15.0;

/// The disclosure: header button, and the two columns when open.
pub(super) fn section<'a>(
    state: &NewProjectState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let ink = tokens::resolve(tokens).ink3;
    let header = button(
        row![
            signex_widgets::chevron::chevron(state.customize_open, TS.label, ink),
            text(t!(i18n, ids::DIALOG_NEW_PROJECT_CUSTOMIZE))
                .size(TS.label)
                .color(ti(tokens.text)),
            text(t!(i18n, ids::DIALOG_NEW_PROJECT_CUSTOMIZE_OPTIONAL))
                .size(TS.label)
                .color(ink),
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center),
    )
    .padding([SP.l, SP.l])
    .width(Length::Fill)
    .on_press(NewProjectMsg::CustomizeToggled)
    .style(crate::styles::ghost_button(tokens));
    let mut body = column![header];
    if state.customize_open {
        let divider = || {
            let line = ti(tokens.border);
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fixed(1.0))
                .style(move |_: &Theme| container::Style {
                    background: Some(line.into()),
                    ..container::Style::default()
                })
        };
        body = body.push(divider().boxed()).push(
            row![
                container(pcb_rules(state, true, i18n, tokens)).width(Length::FillPortion(1)),
                container(schematic(state, i18n, tokens)).width(Length::FillPortion(1)),
            ]
            .spacing(SP.xl)
            .boxed(),
        );
    }
    let line = ti(tokens.border);
    let fill = tokens::resolve(tokens).field;
    container(body)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(fill.into()),
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

fn heading<'a>(id: MessageId, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, NewProjectMsg> {
    text(t!(i18n, id))
        .size(TS.section)
        .color(tokens::resolve(tokens).ink4)
        .boxed()
}

fn field_caption(field: PcbField, i18n: &I18n) -> Localized {
    t!(
        i18n,
        match field {
            PcbField::Clearance => ids::DIALOG_NEW_PROJECT_FIELD_CLEARANCE,
            PcbField::TrackWidth => ids::DIALOG_NEW_PROJECT_FIELD_TRACK,
            PcbField::ViaSize => ids::DIALOG_NEW_PROJECT_FIELD_VIA,
            PcbField::ViaDrill => ids::DIALOG_NEW_PROJECT_FIELD_DRILL,
            PcbField::CopperLayers => ids::DIALOG_NEW_PROJECT_LAYERS_LABEL,
            PcbField::Thickness => ids::DIALOG_NEW_PROJECT_FIELD_THICKNESS,
        }
    )
}

/// Why an imported rule file was not used, in words.
fn rule_error_text(error: &crate::app::new_project::RuleReadError, i18n: &I18n) -> Localized {
    use crate::app::new_project::RuleReadError;
    use signex_types::project::RuleFileError as F;
    match error {
        RuleReadError::Read { path, detail } => t!(
            i18n,
            ids::DIALOG_NEW_PROJECT_ERR_RULES_READ,
            "path" => path.as_str(),
            "detail" => detail.as_str()
        ),
        RuleReadError::File(F::Syntax(detail)) => t!(
            i18n,
            ids::DIALOG_NEW_PROJECT_ERR_RULES_SYNTAX,
            "detail" => detail.as_str()
        ),
        RuleReadError::File(F::NoPcbTable) => t!(i18n, ids::DIALOG_NEW_PROJECT_ERR_RULES_NO_PCB),
        RuleReadError::File(F::Missing(key)) => t!(
            i18n,
            ids::DIALOG_NEW_PROJECT_ERR_RULES_MISSING,
            "key" => *key
        ),
        RuleReadError::File(F::Value { field, why }) => t!(
            i18n,
            ids::DIALOG_NEW_PROJECT_ERR_RULES_VALUE,
            "key" => field.key(),
            "why" => value_error_text(why, i18n).into_string()
        ),
        RuleReadError::File(F::TooLarge(max)) => t!(
            i18n,
            ids::DIALOG_NEW_PROJECT_ERR_RULES_TOO_LARGE,
            "max" => *max
        ),
    }
}

/// The inline reason a rule value is refused.
fn value_error_text(problem: &PcbValueError, i18n: &I18n) -> Localized {
    match problem {
        PcbValueError::NotANumber => t!(i18n, ids::DIALOG_NEW_PROJECT_ERR_NOT_NUMBER),
        PcbValueError::OutOfRange { min, max } => t!(
            i18n,
            ids::DIALOG_NEW_PROJECT_ERR_RANGE,
            "min" => format!("{min}"),
            "max" => format!("{max}")
        ),
        PcbValueError::DrillNotSmallerThanVia => t!(i18n, ids::DIALOG_NEW_PROJECT_ERR_DRILL),
    }
}

fn stepper<'a>(state: &NewProjectState, tokens: &ThemeTokens) -> Element<'a, NewProjectMsg> {
    let step = |glyph: &'static str, delta: i8| {
        button(
            text(UserText::new(glyph))
                .size(TS.label)
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
        )
        .padding([SP.s, SP.m])
        .width(Length::Fixed(TS.label * 2.0))
        .on_press(NewProjectMsg::LayersStepped(delta))
        .style(crate::styles::outline_button(tokens))
    };
    let value = container(
        text(UserText::new(state.setup.pcb.copper_layers.to_string()))
            .size(TS.dense)
            .font(super::super::graphite::mono())
            .color(ti(tokens.text)),
    )
    .padding([SP.s, SP.m])
    .width(Length::Fixed(VALUE_W / 2.0))
    .center_x(Length::Fixed(VALUE_W / 2.0))
    .style(crate::styles::field(tokens, false));
    // The card steppers' step: even layer counts (#2537).
    let by = signex_types::project::LAYER_STEP as i8;
    row![step("\u{2212}", -by), value, step("+", by)]
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .boxed()
}

fn rule_row<'a>(
    state: &NewProjectState,
    field: PcbField,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let input = text_input(
        t!(i18n, ids::DIALOG_NEW_PROJECT_UNIT_MM),
        state.drafts.text(field).to_string(),
    )
    .style(crate::panels::form_look::text_input_style)
    .on_input(move |v| NewProjectMsg::RuleEdited(field, v))
    .on_submit(NewProjectMsg::SubmitRequested)
    .size(TS.dense)
    .font(super::super::graphite::mono())
    .align_x(iced::alignment::Horizontal::Right)
    .padding(SP.s + 1.0)
    .width(Length::Fixed(VALUE_W + SP.xl));
    let control = row![
        input,
        text(t!(i18n, ids::DIALOG_NEW_PROJECT_UNIT_MM))
            .size(TS.body)
            .color(tokens::muted_text(tokens))
            .width(Length::Fixed(TS.title * 1.4)),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    let mut col = column![labelled_row(
        field_caption(field, i18n),
        control.boxed(),
        tokens
    )]
    .spacing(SP.xs);
    if let Some(problem) = state.drafts.problem(field) {
        col = col.push(
            text(value_error_text(&problem, i18n))
                .size(TS.caption)
                .color(tokens::danger(tokens))
                .boxed(),
        );
    }
    col.boxed()
}

/// The Fab tier field (#2537): it mirrors the picked card (Custom and a
/// saved profile show the Custom name), and a pick takes that tier with its
/// `fab_*` rules.
fn fab_tier<'a>(
    state: &NewProjectState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let options: Vec<Opt<FabTemplate>> = FabTemplate::ALL
        .into_iter()
        .map(|fab| Opt {
            value: fab,
            label: t!(
                i18n,
                crate::app::new_project::profile_name_id(SetupProfile::from_fab(fab))
            )
            .into_string(),
        })
        .collect();
    let shown = state
        .setup
        .user_profile
        .is_none()
        .then(|| state.setup.profile.fab_template())
        .flatten();
    let current = options.iter().find(|o| Some(o.value) == shown).cloned();
    let picker = pick_list(current, options, |o: &Opt<FabTemplate>| o.label.clone())
        .graphite()
        .placeholder_label(t!(i18n, ids::DIALOG_NEW_PROJECT_PROFILE_CUSTOM_NAME))
        .on_select(|o: Opt<FabTemplate>| NewProjectMsg::FabPicked(o.value))
        .on_open(NewProjectMsg::DropdownToggled(true))
        .on_close(NewProjectMsg::DropdownToggled(false))
        .width(Length::Fixed(VALUE_W * 2.2));
    labelled_row(
        t!(i18n, ids::DIALOG_NEW_PROJECT_FAB_LABEL),
        picker.boxed(),
        tokens,
    )
}

/// The board rules column under its heading. `with_fab_tier`: the Fab tier
/// field shows; New PCB leaves it out while the project-setup summary row
/// stands for the tier (#2538, F06).
pub(crate) fn pcb_rules<'a>(
    state: &NewProjectState,
    with_fab_tier: bool,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let col = column![
        heading(ids::DIALOG_NEW_PROJECT_PCB_HEADING, i18n, tokens),
        pcb_rule_fields(state, with_fab_tier, i18n, tokens),
    ]
    .spacing(SP.m);
    container(col.padding(SP.l)).boxed()
}

/// The board rule fields alone, for a surface that titles them itself.
pub(crate) fn pcb_rule_fields<'a>(
    state: &NewProjectState,
    with_fab_tier: bool,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let setup = &state.setup;
    let import = button(text(t!(i18n, ids::DIALOG_NEW_PROJECT_FAB_IMPORT)).size(TS.body))
        .padding([SP.s, SP.l])
        .on_press(NewProjectMsg::ImportRequested)
        .style(crate::styles::outline_button(tokens));
    let note_id = if setup.imported {
        ids::DIALOG_NEW_PROJECT_FAB_NOTE_IMPORTED
    } else {
        ids::DIALOG_NEW_PROJECT_FAB_NOTE
    };
    let mut col: iced::widget::Column<Element<'a, NewProjectMsg>> = column![].spacing(SP.m);
    if with_fab_tier {
        col = col.push(fab_tier(state, i18n, tokens));
    }
    col = col.push(
        row![
            text(t!(i18n, note_id))
                .size(TS.caption)
                .color(tokens::muted_text(tokens))
                .width(Length::Fill),
            import,
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .boxed(),
    );
    if let Some(why) = &state.import_error {
        col = col.push(
            text(t!(
                i18n,
                ids::DIALOG_NEW_PROJECT_IMPORT_FAILED,
                "why" => rule_error_text(why, i18n).into_string()
            ))
            .size(TS.caption)
            .color(tokens::danger(tokens))
            .boxed(),
        );
    }
    col = col.push(labelled_row(
        field_caption(PcbField::CopperLayers, i18n),
        stepper(state, tokens),
        tokens,
    ));
    for field in RULE_FIELDS {
        col = col.push(rule_row(state, field, i18n, tokens));
    }
    col.boxed()
}

/// The copper layer stepper and the board thickness field, for the larger
/// board rules view (mockup row 21).
pub(super) fn board_size_fields<'a>(
    state: &NewProjectState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    column![
        labelled_row(
            field_caption(PcbField::CopperLayers, i18n),
            stepper(state, tokens),
            tokens,
        ),
        rule_row(state, PcbField::Thickness, i18n, tokens),
    ]
    .spacing(SP.m)
    .boxed()
}

/// One dropdown entry: the value and the words shown for it.
#[derive(Clone, Debug, PartialEq)]
struct Opt<T> {
    value: T,
    label: String,
}

/// A labelled dropdown for the schematic column; `options` pair each value
/// with its words.
fn choice<'a, T>(
    caption: MessageId,
    selected: &T,
    options: Vec<(T, String)>,
    on_select: impl Fn(T) -> NewProjectMsg + 'a,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg>
where
    T: Clone + PartialEq + std::fmt::Debug + 'a,
{
    let options: Vec<Opt<T>> = options
        .into_iter()
        .map(|(value, label)| Opt { value, label })
        .collect();
    let current = options.iter().find(|o| o.value == *selected).cloned();
    let picker = pick_list(current, options, |o: &Opt<T>| o.label.clone())
        .graphite()
        .on_select(move |o: Opt<T>| on_select(o.value))
        .on_open(NewProjectMsg::DropdownToggled(true))
        .on_close(NewProjectMsg::DropdownToggled(false))
        .width(Length::Fixed(SHEET_CONTROL_W));
    labelled_row(t!(i18n, caption), picker.boxed(), tokens)
}

/// The sheet sizes by name: the Orientation control carries the direction.
pub(super) fn size_options() -> Vec<(String, String)> {
    SHEET_SIZES
        .iter()
        .map(|size| ((*size).to_string(), (*size).to_string()))
        .collect()
}

/// Orientation as two segments (mockup rows 21, 22, 31 and 37).
fn orientation_row<'a>(
    orientation: SheetOrientation,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let options = [
        (
            SheetOrientation::Landscape,
            Label::from(t!(i18n, ids::SETUP_SHEET_LANDSCAPE)),
        ),
        (
            SheetOrientation::Portrait,
            Label::from(t!(i18n, ids::SETUP_SHEET_PORTRAIT)),
        ),
    ];
    let control = container(segmented(
        options,
        orientation,
        tokens::resolve(tokens).segmented(TS.body).fill_width(true),
        NewProjectMsg::SheetOrientationPicked,
    ))
    .width(Length::Fixed(SHEET_CONTROL_W));
    labelled_row(
        t!(i18n, ids::SETUP_SHEET_ORIENTATION),
        control.boxed(),
        tokens,
    )
}

fn title_block_choice<'a>(
    is_shown: bool,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    choice(
        ids::DIALOG_NEW_PROJECT_SCH_TITLE_BLOCK,
        &is_shown,
        vec![
            (
                true,
                t!(i18n, ids::DIALOG_NEW_PROJECT_SCH_TITLE_BLOCK_DEFAULT).into_string(),
            ),
            (
                false,
                t!(i18n, ids::DIALOG_NEW_PROJECT_SCH_TITLE_BLOCK_NONE).into_string(),
            ),
        ],
        NewProjectMsg::TitleBlockPicked,
        i18n,
        tokens,
    )
}

/// The schematic column under its heading.
pub(crate) fn schematic<'a>(
    state: &NewProjectState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let col = column![
        heading(ids::DIALOG_NEW_PROJECT_SHEET_HEADING, i18n, tokens),
        schematic_fields(state, i18n, tokens),
    ]
    .spacing(SP.m);
    container(col.padding(SP.l)).boxed()
}

/// The sheet fields alone, for a surface that titles them itself.
pub(crate) fn schematic_fields<'a>(
    state: &NewProjectState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let sheet = &state.setup.sheet;
    let (size, orientation) = SheetSize::parse(&sheet.paper_size);
    let size_choice = choice(
        ids::DIALOG_NEW_PROJECT_SCH_SIZE,
        &size.token(SheetOrientation::Landscape),
        size_options(),
        NewProjectMsg::SheetSizePicked,
        i18n,
        tokens,
    );
    let designators = choice(
        ids::DIALOG_NEW_PROJECT_SCH_DESIGNATORS,
        &state.setup.designators,
        DesignatorStandard::ALL
            .iter()
            .map(|d| (*d, d.label().to_string()))
            .collect(),
        NewProjectMsg::DesignatorsPicked,
        i18n,
        tokens,
    );
    column![
        size_choice,
        orientation_row(orientation, i18n, tokens),
        title_block_choice(sheet.show_title_block, i18n, tokens),
        designators,
    ]
    .spacing(SP.m)
    .boxed()
}
