//! The Differential Pair Classes dialog (#1489): the class list on the
//! left (each row with Rename, Duplicate and Delete), the selected class on
//! the right (name, impedance, width and gap, the read-only skew and
//! uncoupled limits that live in the Rules tab, a board-style preview, the
//! pairs that use it and the suffix pairing), Cancel / OK below. State and
//! rules: `app/diff_pair_classes/`.

mod preview;

use iced::widget::{Column, Row, Space, column, container, row, scrollable};
use iced::{Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{CheckboxLabel as _, button, checkbox, text, text_input};

use crate::app::diff_pair_classes::{
    DiffPairClassesMsg, DiffPairClassesState, PairClassField, PairClassProblem, field_value,
};
use crate::app::{Message, Signex};
use crate::styles::{
    DialogButtonKind, Spacing, TypeScale, dialog_button, dialog_footer_split, ti, tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The dialog, as the mockup draws it.
const SIZE: (f32, f32) = (760.0, 640.0);
/// The class list column.
const LIST_W: f32 = 200.0;
/// The label column of the Class group.
const LABEL_W: f32 = 110.0;

fn dpc(msg: DiffPairClassesMsg) -> Message {
    Message::from(msg)
}

/// A value as a field shows it.
fn shown(state: &DiffPairClassesState, field: PairClassField) -> String {
    if let Some(draft) = state.draft(field) {
        return draft.to_string();
    }
    state
        .selected_class()
        .and_then(|c| field_value(c, field))
        .map(number)
        .unwrap_or_default()
}

/// `value` without trailing zeros past the point.
fn number(value: f64) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

impl Signex {
    pub(in crate::app::view) fn view_diff_pair_classes_dialog(&self) -> Element<'_, Message> {
        self.wrap_centred_dialog(self.view_diff_pair_classes_card(), SIZE)
    }

    fn view_diff_pair_classes_card(&self) -> Element<'_, Message> {
        let Some(state) = self.ui_state.diff_pair_classes.as_deref() else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let cancel = Some(dpc(DiffPairClassesMsg::CancelRequested));
        let pairs = state.members.len() as u32;
        let header = super::header::graphite_header_closing(
            t!(i18n, ids::DIALOG_DPC_TITLE).into_string(),
            Some(
                t!(
                    i18n,
                    ids::DIALOG_DPC_SUBTITLE,
                    "project" => state.project_name.as_str(),
                    "classes" => state.classes.len() as u32,
                    "pairs" => pairs
                )
                .into_string(),
            ),
            None,
            cancel.clone(),
            super::header::DIALOG_HEADER_H,
            tokens,
        );
        let r = tokens::resolve(tokens);
        let divider = container(Space::new())
            .width(Length::Fixed(1.0))
            .height(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(r.line_soft)),
                ..container::Style::default()
            });
        let body = row![
            class_list(state, i18n, tokens),
            divider,
            scrollable(container(class_form(state, i18n, tokens)).padding([SP.l, SP.xl]))
                .height(Length::Fill)
                .width(Length::Fill),
        ]
        .height(Length::Fill);
        let mut extras: Vec<Element<'_, Message>> = vec![
            text(t!(i18n, ids::DIALOG_DPC_FOOTER))
                .size(TS.caption)
                .color(ti(tokens.text_secondary))
                .boxed(),
        ];
        if let Some(problem) = state.problem {
            let id = match problem {
                PairClassProblem::BlankName => ids::DIALOG_DPC_BLANK_NAME,
                PairClassProblem::DuplicateName => ids::DIALOG_DPC_DUPLICATE_NAME,
            };
            extras.push(
                text(t!(i18n, id))
                    .size(TS.caption)
                    .color(ti(tokens.error))
                    .boxed(),
            );
        }
        let footer = dialog_footer_split(
            extras,
            vec![
                dialog_button(
                    t!(i18n, ids::DIALOG_DPC_CANCEL),
                    DialogButtonKind::Cancel,
                    cancel,
                    tokens,
                ),
                dialog_button(
                    t!(i18n, ids::DIALOG_DPC_OK),
                    super::graphite::call_to_action(tokens),
                    Some(dpc(DiffPairClassesMsg::OkRequested)),
                    tokens,
                ),
            ],
        );
        container(column![header, body, footer].width(Length::Fixed(
            super::widgets::modal_card_width(self.ui_state.window_size, SIZE.0),
        )))
        .height(Length::Fixed(SIZE.1))
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }
}

/// The left column: one row per class with its pair count and menu, New
/// class, and where pairs come from.
fn class_list<'a>(
    state: &'a DiffPairClassesState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let ink = ti(tokens.text);
    let muted = ti(tokens.text_secondary);
    let mut list = Column::new().spacing(SP.xs);
    for (index, class) in state.classes.iter().enumerate() {
        let selected = index == state.selected;
        let count = state.pair_count(index) as u32;
        let name = column![
            text(signex_i18n::UserText::new(class.name.clone()))
                .size(TS.body)
                .color(ink)
                .font(iced::Font::MONOSPACE),
            text(t!(i18n, ids::DIALOG_DPC_PAIRS, "count" => count))
                .size(TS.caption)
                .color(muted),
        ];
        let open = if state.menu == Some(index) {
            None
        } else {
            Some(index)
        };
        let row_line = row![
            button(name)
                .on_press(dpc(DiffPairClassesMsg::ClassSelected(index)))
                .style(crate::panels::form_look::ghost_button)
                .width(Length::Fill),
            button(
                text(t!(i18n, ids::DIALOG_DPC_MENU))
                    .size(TS.body)
                    .color(muted)
            )
            .on_press(dpc(DiffPairClassesMsg::ClassMenuToggled(open)))
            .style(crate::panels::form_look::ghost_button),
        ]
        .align_y(iced::Alignment::Center);
        let tint = r.accent_soft;
        list = list.push(
            container(row_line)
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: selected.then_some(Background::Color(tint)),
                    border: Border {
                        radius: SP.xs.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                })
                .boxed(),
        );
        if state.menu == Some(index) {
            list = list.push(class_menu(state, index, i18n, tokens));
        }
    }
    container(
        column![
            scrollable(list).height(Length::Fill),
            button(text(t!(i18n, ids::DIALOG_DPC_NEW_CLASS)).size(TS.body))
                .on_press(dpc(DiffPairClassesMsg::ClassAdded))
                .style(crate::panels::form_look::ghost_button),
            text(t!(i18n, ids::DIALOG_DPC_PAIRS_FROM))
                .size(TS.caption)
                .color(muted)
                .wrapping(iced::widget::text::Wrapping::Word),
        ]
        .spacing(SP.s),
    )
    .width(Length::Fixed(LIST_W))
    .height(Length::Fill)
    .padding(SP.s)
    .boxed()
}

/// A class row's menu: Rename, Duplicate, and Delete, greyed while the
/// class has pairs (it says how many).
fn class_menu<'a>(
    state: &DiffPairClassesState,
    index: usize,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let muted = ti(tokens.text_secondary);
    let item = |id, msg: Option<DiffPairClassesMsg>| {
        let mut b = button(text(t!(i18n, id)).size(TS.body))
            .style(crate::panels::form_look::ghost_button)
            .width(Length::Fill);
        if let Some(msg) = msg {
            b = b.on_press(dpc(msg));
        }
        b.boxed()
    };
    let deletable = state.can_delete(index);
    let pairs = state.pair_count(index) as u32;
    let mut delete = Row::new()
        .push(item(
            ids::DIALOG_DPC_DELETE,
            deletable.then_some(DiffPairClassesMsg::ClassDeleted(index)),
        ))
        .align_y(iced::Alignment::Center);
    if !deletable {
        delete = delete.push(
            text(t!(i18n, ids::DIALOG_DPC_PAIRS, "count" => pairs))
                .size(TS.caption)
                .color(muted)
                .boxed(),
        );
    }
    let r = tokens::resolve(tokens);
    container(
        column![
            item(
                ids::DIALOG_DPC_RENAME,
                Some(DiffPairClassesMsg::ClassRenameStarted(index))
            ),
            item(
                ids::DIALOG_DPC_DUPLICATE,
                Some(DiffPairClassesMsg::ClassDuplicated(index))
            ),
            delete,
        ]
        .spacing(0),
    )
    .padding(SP.xs)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(r.raised)),
        border: Border {
            color: r.line_soft,
            width: 1.0,
            radius: SP.xs.into(),
        },
        ..container::Style::default()
    })
    .boxed()
}

/// One `label | control` row of the Class group.
fn form_row<'a>(
    label: signex_i18n::Localized,
    control: Element<'a, Message>,
    muted: Color,
) -> Element<'a, Message> {
    row![
        container(text(label).size(TS.body).color(muted)).width(Length::Fixed(LABEL_W)),
        control,
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// A value box `width` wide for `field`, with its unit after it.
fn value_box<'a>(
    state: &DiffPairClassesState,
    field: PairClassField,
    width: f32,
    unit: signex_i18n::Localized,
    muted: Color,
) -> Element<'a, Message> {
    row![
        text_input(
            signex_i18n::UserText::new(String::new()),
            shown(state, field)
        )
        .on_input(move |t| dpc(DiffPairClassesMsg::FieldEdited(field, t)))
        .size(TS.body)
        .padding([SP.xs, SP.s])
        .style(crate::panels::form_look::text_input_style)
        .width(Length::Fixed(width)),
        text(unit).size(TS.caption).color(muted),
    ]
    .spacing(SP.xs)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// A group title.
fn group<'a>(label: signex_i18n::Localized, tokens: &ThemeTokens) -> Element<'a, Message> {
    container(
        text(label)
            .size(TS.label)
            .color(ti(tokens.text_secondary))
            .font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..iced::Font::DEFAULT
            }),
    )
    .padding([SP.m, 0.0])
    .boxed()
}

/// The right side: the selected class's fields, its preview, its pairs and
/// the suffix pairing.
fn class_form<'a>(
    state: &'a DiffPairClassesState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let muted = ti(tokens.text_secondary);
    let Some(class) = state.selected_class() else {
        return text(t!(i18n, ids::DIALOG_DPC_NO_CLASS))
            .size(TS.body)
            .color(muted)
            .boxed();
    };
    let mm = || t!(i18n, ids::DIALOG_DPC_UNIT_MM);
    let name = text_input(
        signex_i18n::UserText::new(String::new()),
        class.name.clone(),
    )
    .id(iced::widget::Id::new(
        crate::app::diff_pair_classes::NAME_INPUT_ID,
    ))
    .on_input(|t| dpc(DiffPairClassesMsg::NameEdited(t)))
    .size(TS.body)
    .padding([SP.xs, SP.s])
    .style(crate::panels::form_look::text_input_style)
    .width(Length::Fixed(220.0))
    .boxed();
    let impedance = row![
        value_box(
            state,
            PairClassField::Impedance,
            80.0,
            t!(i18n, ids::DIALOG_DPC_UNIT_OHM),
            muted
        ),
        text(t!(i18n, ids::DIALOG_DPC_PLUS_MINUS))
            .size(TS.caption)
            .color(muted),
        value_box(
            state,
            PairClassField::Tolerance,
            64.0,
            t!(i18n, ids::DIALOG_DPC_UNIT_PCT),
            muted
        ),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center)
    .boxed();
    let width_gap = row![
        value_box(state, PairClassField::Width, 96.0, mm(), muted),
        value_box(state, PairClassField::Gap, 96.0, mm(), muted),
        text(t!(i18n, ids::DIALOG_DPC_ON_TOP))
            .size(TS.caption)
            .color(muted),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center)
    .boxed();
    // SCH-06: skew and uncoupled length are High Speed rules; no such rule
    // exists yet, so both read "—" with a way to the Rules tab.
    let read_only = || {
        text(t!(i18n, ids::DIALOG_DPC_NOT_SET))
            .size(TS.body)
            .color(muted)
            .boxed()
    };
    let link = button(
        text(t!(i18n, ids::DIALOG_DPC_EDIT_IN_RULES))
            .size(TS.caption)
            .color(ti(tokens.accent)),
    )
    .on_press(dpc(DiffPairClassesMsg::RulesLinkClicked))
    .style(crate::panels::form_look::ghost_button)
    .boxed();
    let pair_by_suffix = class.pair_by_suffix;
    let mut members = Column::new().spacing(0).push(member_row(
        [
            t!(i18n, ids::DIALOG_DPC_COL_PAIR).into_string(),
            t!(i18n, ids::DIALOG_DPC_COL_P).into_string(),
            t!(i18n, ids::DIALOG_DPC_COL_N).into_string(),
            t!(i18n, ids::DIALOG_DPC_COL_SOURCE).into_string(),
        ],
        muted,
    ));
    let mut any = false;
    for m in state.members.iter().filter(|m| m.class == class.name) {
        any = true;
        let source =
            t!(i18n, ids::DIALOG_DPC_SOURCE_DIRECTIVE, "sheet" => m.sheet.as_str()).into_string();
        members = members.push(member_row(
            [
                m.pair.clone(),
                m.positive.clone(),
                m.negative.clone(),
                source,
            ],
            ti(tokens.text),
        ));
    }
    if !any {
        members = members.push(
            text(t!(i18n, ids::DIALOG_DPC_NO_MEMBERS))
                .size(TS.caption)
                .color(muted)
                .boxed(),
        );
    }
    Column::new()
        .spacing(SP.s)
        .push(group(t!(i18n, ids::DIALOG_DPC_CLASS), tokens))
        .push(form_row(t!(i18n, ids::DIALOG_DPC_NAME), name, muted))
        .push(form_row(
            t!(i18n, ids::DIALOG_DPC_IMPEDANCE),
            impedance,
            muted,
        ))
        .push(form_row(
            t!(i18n, ids::DIALOG_DPC_WIDTH_GAP),
            width_gap,
            muted,
        ))
        .push(form_row(
            t!(i18n, ids::DIALOG_DPC_MAX_SKEW),
            read_only(),
            muted,
        ))
        .push(form_row(
            t!(i18n, ids::DIALOG_DPC_MAX_UNCOUPLED),
            read_only(),
            muted,
        ))
        .push(row![Space::new().width(Length::Fixed(LABEL_W + SP.m)), link].boxed())
        .push(preview::preview(class, i18n))
        .push(group(t!(i18n, ids::DIALOG_DPC_MEMBERS), tokens))
        .push(members.boxed())
        .push(group(t!(i18n, ids::DIALOG_DPC_PAIRING), tokens))
        .push(
            checkbox(pair_by_suffix)
                .style(crate::panels::form_look::checkbox_style)
                .size(TS.title)
                .labelled(t!(i18n, ids::DIALOG_DPC_PAIR_BY_SUFFIX))
                .text_size(TS.label)
                .on_toggle(|on| dpc(DiffPairClassesMsg::PairBySuffixToggled(on)))
                .boxed(),
        )
        .boxed()
}

/// One Members table line: Pair | P net | N net | Source.
fn member_row<'a>(cells: [String; 4], ink: Color) -> Element<'a, Message> {
    let mut line = Row::new().spacing(SP.s).padding([SP.xs, 0.0]);
    for (i, cell) in cells.into_iter().enumerate() {
        let width = if i < 3 {
            Length::FillPortion(22)
        } else {
            Length::FillPortion(34)
        };
        line = line.push(
            container(
                text(signex_i18n::UserText::new(cell))
                    .size(TS.body)
                    .color(ink)
                    .wrapping(iced::widget::text::Wrapping::None)
                    .ellipsis(iced::widget::text::Ellipsis::End),
            )
            .width(width)
            .boxed(),
        );
    }
    line.boxed()
}
