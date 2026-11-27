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
                        "count" => defs.len() + facts.undefined_types.len()))
                    .size(TS.caption)
                    .color(muted)
                    .boxed(),
                ),
        )
        .padding(SP.m)
        .boxed(),
    );
    if facts.are_definitions_locked {
        col = col.push(
            container(
                text(t!(i18n, ids::DIALOG_SCHEMATIC_DESIGN_UNREADABLE))
                    .size(TS.caption)
                    .color(crate::styles::ti(ctx.tokens.warning)),
            )
            .padding([SP.xs, SP.m])
            .boxed(),
        );
    }
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
    // Types in use with no definition: listed with their counts, not picked
    // (their entries belong to no definition to edit).
    for (j, d) in facts.undefined_types.iter().enumerate() {
        let uses = facts.uses.get(defs.len() + j).copied().unwrap_or(0);
        let caption = entries_caption(d);
        col = col.push(
            row![
                text(UserText::new(d.name.clone()))
                    .size(TS.body)
                    .width(Length::Fill),
                text(t!(i18n, ids::PANELS_HARNESS_DEFS_UNDEFINED_USES,
                    "entries" => caption.as_str(),
                    "uses" => uses))
                .size(TS.caption)
                .color(muted),
            ]
            .padding([SP.xs, SP.m])
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
                .style(crate::styles::primary_button(&ctx.tokens))
                .on_press(msg(HarnessPanelMsg::NewTypeClicked)),
            // Mockup row 152: Back is the secondary action beside New.
            signex_widgets::button(text(t!(i18n, ids::PANELS_HARNESS_DEFS_BACK)).size(TS.body))
                .padding([SP.xs, SP.m])
                .style(crate::styles::outline_button(&ctx.tokens))
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

#[cfg(test)]
mod in_use_tests {
    use super::*;
    use crate::app::{Message, Signex, TabInfo, TabKind};
    use signex_types::schematic::{HarnessConnector, HarnessEntry, Point, SchematicSheet};

    const DOCK: iced::Size = iced::Size::new(300.0, 400.0);

    /// A sheet holding one connector of the fallback type `HARNESS`, with
    /// one entry, open with no project around it: the state the audit
    /// captured for mockup row 152.
    fn app_with_undefined_connector() -> Signex {
        let path = std::path::PathBuf::from("harness.snxsch");
        let mut sheet = SchematicSheet {
            paper_size: "A4".to_string(),
            ..Default::default()
        };
        sheet.harness.connectors.push(HarnessConnector {
            uuid: uuid::Uuid::from_u128(1),
            harness_type: "HARNESS".into(),
            position: Point::new(10.0, 10.0),
            width: 10.16,
            entry_pitch: 2.54,
            rotation: 0.0,
            entries: vec![HarnessEntry {
                uuid: uuid::Uuid::from_u128(10),
                name: "ENTRY_1".into(),
            }],
            locked: false,
        });
        let engine = signex_engine::Engine::new(sheet).expect("engine");
        let (mut app, _boot) = Signex::new();
        app.ui_state.first_run_tour_open = false;
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.tabs.push(TabInfo {
            title: "harness".to_string(),
            path: path.clone(),
            project_id: None,
            kind: TabKind::Schematic,
        });
        app.document_state.active_tab = app.document_state.tabs.len() - 1;
        app.document_state.active_path = Some(path);
        app
    }

    fn open_definitions(app: &mut Signex) {
        let _ = app.update(Message::Dock(crate::dock::DockMessage::Panel(
            PanelMsg::Harness(HarnessPanelMsg::DefinitionsOpened),
        )));
    }

    fn view(ctx: &PanelContext) -> Element<'_, PanelMsg> {
        view_harness_definitions(ctx, Color::BLACK, Color::WHITE)
    }

    /// #3043, mockup row 152: a type a placed connector uses appears in the
    /// definitions list with its entry and use counts, though the project
    /// defines none, and the caption counts it.
    #[test]
    fn a_type_in_use_without_a_definition_is_listed_with_its_counts() {
        let mut app = app_with_undefined_connector();
        open_definitions(&mut app);
        let facts = &app.document_view.panel_ctx.harness;
        assert!(facts.definitions.is_empty());
        let names: Vec<&str> = facts
            .undefined_types
            .iter()
            .map(|d| d.name.as_str())
            .collect();
        assert_eq!(names, ["HARNESS"]);
        assert_eq!(facts.uses, [1]);
        let ctx = &app.document_view.panel_ctx;
        let i18n: &signex_i18n::I18n = &ctx.i18n;
        let clean = |s: &str| s.replace(['\u{2068}', '\u{2069}'], "");
        let counts = clean(
            &t!(i18n, ids::PANELS_HARNESS_DEFS_UNDEFINED_USES,
                "entries" => "1",
                "uses" => 1usize)
            .into_string(),
        );
        assert_eq!(counts, "1 entries · 1 use · not defined");
        let caption = clean(
            &t!(i18n, ids::PANELS_HARNESS_DEFS_CAPTION,
                "project" => "",
                "count" => 1usize)
            .into_string(),
        );
        let texts: Vec<crate::layout_probe::LaidOutText> =
            crate::layout_probe::lay_out_texts(view(ctx), DOCK)
                .into_iter()
                .map(|t| crate::layout_probe::LaidOutText {
                    text: clean(&t.text),
                    bounds: t.bounds,
                })
                .collect();
        let name = crate::layout_probe::find(&texts, "HARNESS").bounds;
        let row = crate::layout_probe::find(&texts, &counts).bounds;
        crate::layout_probe::find(&texts, &caption);
        assert!((name.center_y() - row.center_y()).abs() < 1.0, "{texts:#?}");
        assert!(row.x + row.width <= DOCK.width, "{texts:#?}");
        // The row picks nothing: there is no definition to edit.
        let pressed = crate::layout_probe::messages_on_click(view(ctx), DOCK, "HARNESS");
        assert!(pressed.is_empty(), "{pressed:?}");
    }

    /// Mockup row 152: Back is the secondary action, drawn unlike the filled
    /// New harness type beside it, and still closes the view.
    #[test]
    fn back_is_drawn_as_the_secondary_action() {
        let mut app = app_with_undefined_connector();
        open_definitions(&mut app);
        let ctx = &app.document_view.panel_ctx;
        let new = t!(ctx.i18n, ids::PANELS_HARNESS_DEFS_NEW).into_string();
        let back = t!(ctx.i18n, ids::PANELS_HARNESS_DEFS_BACK).into_string();
        let texts = crate::layout_probe::lay_out_texts(view(ctx), DOCK);
        let at_new = crate::layout_probe::find(&texts, &new).bounds;
        let at_back = crate::layout_probe::find(&texts, &back).bounds;
        let pixels = crate::layout_probe::draw_pixels(view(ctx), DOCK);
        // Inside each button's padding, just left of its label.
        let fill = |r: iced::Rectangle| {
            let (x, y) = ((r.x - 3.0) as usize, r.center_y() as usize);
            let i = (y * DOCK.width as usize + x) * 4;
            pixels[i..i + 3].to_vec()
        };
        assert_ne!(fill(at_new), fill(at_back));
        let pressed = crate::layout_probe::messages_on_click(view(ctx), DOCK, &back);
        assert!(
            matches!(
                pressed.as_slice(),
                [PanelMsg::Harness(HarnessPanelMsg::DefinitionsClosed)]
            ),
            "{pressed:?}"
        );
    }
}
