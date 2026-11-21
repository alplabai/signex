//! The harness definitions view (#1280), opened from a connector's
//! Definition link or by `open_harness_definitions`: the project's harness
//! types with their entry and use counts, the selected type's Name,
//! Description and Entries (each name editable, its kind a caption or a
//! nested-harness pill), a nested type's own list, and New harness type.
//! Edits go to the project as one step each (`ProjectSectionCommand::
//! SetHarnessDefinitions`). Definitions live in the `.snxprj`.

use super::super::*;
use super::harness_connector::HarnessPanelMsg;
use crate::styles::{PillTone, Spacing, TypeScale, pill};
use iced::Widget;
use signex_i18n::{UserText, ids, t};
use signex_types::schematic::{HarnessDefinition, harness_definition};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

fn msg(m: HarnessPanelMsg) -> PanelMsg {
    PanelMsg::Harness(m)
}

/// The whole definitions view.
pub(in crate::panels) fn view_harness_definitions<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    primary: Color,
) -> Element<'a, PanelMsg> {
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let facts = &ctx.harness;
    let defs = &facts.definitions;
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);
    col = col.push(
        container(
            Column::new()
                .spacing(SP.xs)
                .push(
                    text(t!(i18n, ids::PANELS_HARNESS_DEFS_TITLE))
                        .size(TS.label)
                        .color(primary)
                        .boxed(),
                )
                .push(
                    text(t!(i18n, ids::PANELS_HARNESS_DEFS_CAPTION,
                        "project" => facts.project_name.as_str(),
                        "count" => defs.len()))
                    .size(TS.caption)
                    .color(muted)
                    .boxed(),
                ),
        )
        .padding(SP.m)
        .boxed(),
    );
    for (i, d) in defs.iter().enumerate() {
        let uses = facts.uses.get(i).copied().unwrap_or(0);
        let caption = entries_caption(d);
        let line = row![
            text(UserText::new(d.name.clone()))
                .size(TS.body)
                .width(Length::Fill),
            text(t!(i18n, ids::PANELS_HARNESS_DEFS_USES,
                "entries" => caption.as_str(),
                "uses" => uses))
            .size(TS.caption)
            .color(muted),
        ]
        .padding([SP.xs, SP.m]);
        let cell = container(line).width(Length::Fill);
        let cell = if i == facts.selected_definition {
            cell.style(pill(&ctx.tokens, PillTone::Neutral))
        } else {
            cell
        };
        col = col.push(
            signex_widgets::button(cell)
                .padding(0)
                .width(Length::Fill)
                .on_press(msg(HarnessPanelMsg::DefinitionPicked(i)))
                .boxed(),
        );
    }
    if let Some(d) = defs.get(facts.selected_definition) {
        col = push_definition(col, ctx, facts.selected_definition, d, muted);
        // A nested type's own entries follow, read-only.
        for nested in d.entries.iter().filter_map(|e| e.harness.as_deref()) {
            if let Some(n) = harness_definition(defs, nested) {
                col = col.push(
                    container(
                        text(t!(i18n, ids::PANELS_HARNESS_DEFS_NESTED_SECTION, "type" => n.name.as_str()))
                            .size(TS.label)
                            .color(primary),
                    )
                    .padding([SP.s, SP.m])
                    .boxed(),
                );
                for e in &n.entries {
                    col = col.push(
                        row![
                            text(UserText::new(e.name.clone()))
                                .size(TS.body)
                                .width(Length::Fill),
                            kind_cell(ctx, defs, e.harness.as_deref(), muted),
                        ]
                        .padding([SP.xs, SP.m])
                        .boxed(),
                    );
                }
            }
        }
    }
    col = col.push(
        row![
            signex_widgets::button(text(t!(i18n, ids::PANELS_HARNESS_DEFS_NEW)).size(TS.body))
                .padding([SP.xs, SP.m])
                .on_press(msg(HarnessPanelMsg::NewTypeClicked)),
            signex_widgets::button(text(t!(i18n, ids::PANELS_HARNESS_DEFS_BACK)).size(TS.body))
                .padding([SP.xs, SP.m])
                .on_press(msg(HarnessPanelMsg::DefinitionsClosed)),
        ]
        .spacing(SP.s)
        .padding(SP.m)
        .boxed(),
    );
    scrollable(col).width(Length::Fill).boxed()
}

/// `4` or `2 + UART`: the entry count, nested types named.
fn entries_caption(d: &HarnessDefinition) -> String {
    let signals = d.entries.iter().filter(|e| e.harness.is_none()).count();
    let nested: Vec<&str> = d
        .entries
        .iter()
        .filter_map(|e| e.harness.as_deref())
        .collect();
    if nested.is_empty() {
        signals.to_string()
    } else {
        format!("{signals} + {}", nested.join(" + "))
    }
}

/// The Kind cell of an entry: `signal`, or a pill naming the nested type's
/// entries.
fn kind_cell<'a>(
    ctx: &'a PanelContext,
    defs: &[HarnessDefinition],
    nested: Option<&str>,
    muted: Color,
) -> Element<'a, PanelMsg> {
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    match nested {
        None => text(t!(i18n, ids::PANELS_HARNESS_DEFS_SIGNAL))
            .size(TS.caption)
            .color(muted)
            .boxed(),
        Some(ty) => {
            let names = harness_definition(defs, ty)
                .map(|n| n.entry_names().join(", "))
                .unwrap_or_default();
            container(
                text(t!(i18n, ids::PANELS_HARNESS_DEFS_NESTED, "entries" => names.as_str()))
                    .size(TS.caption)
                    .color(ctx.canvas_colors.harness_color().into_iced()),
            )
            .padding([1.0, SP.s])
            .style(pill(&ctx.tokens, PillTone::Neutral))
            .boxed()
        }
    }
}

/// The selected type: Name, Description and its Entries.
fn push_definition<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    index: usize,
    d: &HarnessDefinition,
    muted: Color,
) -> Column<Element<'a, PanelMsg>> {
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    col = col
        .push(form_edit_row(
            t!(i18n, ids::PANELS_HARNESS_DEFS_NAME),
            &d.name,
            muted,
            move |text| msg(HarnessPanelMsg::DefinitionNameTyped { index, text }),
        ))
        .push(form_edit_row(
            t!(i18n, ids::PANELS_HARNESS_DEFS_DESCRIPTION),
            &d.description,
            muted,
            move |text| msg(HarnessPanelMsg::DefinitionDescriptionTyped { index, text }),
        ))
        .push(
            row![
                text(t!(i18n, ids::PANELS_HARNESS_ENTRY))
                    .size(TS.caption)
                    .color(muted)
                    .width(Length::Fill),
                text(t!(i18n, ids::PANELS_HARNESS_DEFS_KIND))
                    .size(TS.caption)
                    .color(muted),
            ]
            .padding([SP.xs, SP.m])
            .boxed(),
        );
    for (entry, e) in d.entries.iter().enumerate() {
        col = col.push(
            row![
                form_edit_row(
                    t!(i18n, ids::PANELS_HARNESS_ENTRY),
                    &e.name,
                    muted,
                    move |text| msg(HarnessPanelMsg::DefinitionEntryTyped { index, entry, text }),
                ),
                kind_cell(ctx, &ctx.harness.definitions, e.harness.as_deref(), muted),
            ]
            .align_y(iced::Alignment::Center)
            .boxed(),
        );
    }
    col
}

/// iced colour of a theme colour.
trait IntoIced {
    fn into_iced(self) -> Color;
}

impl IntoIced for signex_types::theme::Color {
    fn into_iced(self) -> Color {
        Color::from_rgba8(self.r, self.g, self.b, f32::from(self.a) / 255.0)
    }
}
