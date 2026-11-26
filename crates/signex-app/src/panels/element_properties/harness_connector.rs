//! Properties of a selected harness connector (#1280), as mockup row 152
//! draws it: a caption with the sheet and entry count; in the mismatch state
//! a warn banner (with Rename and Go to buttons) and the Entries list right
//! under the header, the mismatched row tinted; the read-only illustration
//! with its Width and Entry pitch callouts; the Harness section (Type from
//! the project's definitions, the Definition link, the one project colour);
//! the Placement section (X / Y, Width, Entry pitch); the Entries list with
//! the net each entry meets on this sheet and a + that adds an entry; the
//! compiled-name note.
//! The facts are gathered in `update` (`Signex::harness_panel_facts`); this
//! is pure view. Design: `docs/SIGNAL_HARNESSES.md`.

use super::super::*;
use super::rows::PanelStyle;
use crate::styles::{PillTone, Spacing, TypeScale, pill};
use iced::Widget;
use signex_i18n::{UserText, ids, t};
use signex_types::schematic::{HarnessConnector, HarnessDefinition};
use uuid::Uuid;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// The harness colour swatch: 16 px at the 12 px body, rounded by a quarter.
const SWATCH: f32 = TS.body * 16.0 / 12.0;
const SWATCH_RADIUS: f32 = SWATCH / 4.0;

/// A placement field of a connector typed in Properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessField {
    X,
    Y,
    Width,
    EntryPitch,
}

/// One entry the far side of the harness expects and this side lacks, read
/// from the sheet's ERC findings (`builtin::harness_entry_mismatch`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessMismatchRow {
    /// The entry of this connector standing in the expected one's place.
    pub entry: Uuid,
    pub has: String,
    pub expects: String,
    /// The sheet symbol whose child expects it.
    pub far: String,
}

/// What the harness Properties and the definitions view show.
#[derive(Debug, Clone, Default)]
pub struct HarnessPanel {
    /// The file name of the sheet in view.
    pub sheet_name: String,
    /// The name of the project of the sheet in view.
    pub project_name: String,
    /// The project's harness definitions.
    pub definitions: Vec<HarnessDefinition>,
    /// The project's settings section is unreadable: definitions cannot be
    /// edited.
    pub are_definitions_locked: bool,
    /// Per definition: how many connectors of the open sheets use it.
    pub uses: Vec<usize>,
    /// The selected connector (or the one owning the selected entry).
    pub connector: Option<HarnessConnector>,
    /// Per entry of `connector`: the net it meets on this sheet.
    pub entry_nets: Vec<String>,
    pub mismatches: Vec<HarnessMismatchRow>,
    /// The type of the bundle the selected harness wire belongs to.
    pub wire_type: Option<String>,
    /// The selected harness wire's ends.
    pub wire_ends: Option<(
        signex_types::schematic::Point,
        signex_types::schematic::Point,
    )>,
    /// The harness type of the selected sheet entry, when it has one.
    pub sheet_entry_type: Option<String>,
    /// Whether the definitions view replaces the Properties content.
    pub definitions_open: bool,
    /// The type listed in the definitions view.
    pub selected_definition: usize,
    /// A placement field being typed and its text.
    pub draft: Option<(HarnessField, String)>,
    /// The stamp of the app state these facts were built from; see
    /// `Signex::refresh_harness_panel`.
    pub built_from: Option<u64>,
}

/// An edit or a click in the harness Properties or definitions view.
#[derive(Debug, Clone, PartialEq)]
pub enum HarnessPanelMsg {
    TypePicked {
        connector: Uuid,
        harness_type: String,
    },
    FieldTyped {
        connector: Uuid,
        field: HarnessField,
        text: String,
    },
    /// "Rename CS to CS_ADC": rename the entry, one undo step.
    RenameFixClicked {
        connector: Uuid,
        entry: Uuid,
        to: String,
    },
    /// "Go to sensor": open the child sheet of the sheet symbol `far`.
    GoToSheetClicked {
        far: String,
    },
    DefinitionsOpened,
    DefinitionsClosed,
    DefinitionPicked(usize),
    DefinitionNameTyped {
        index: usize,
        text: String,
    },
    DefinitionDescriptionTyped {
        index: usize,
        text: String,
    },
    DefinitionEntryTyped {
        index: usize,
        entry: usize,
        text: String,
    },
    NewTypeClicked,
    /// The + of the Entries header: one more entry at the end of the
    /// connector, one undo step.
    AddEntryClicked {
        connector: Uuid,
    },
    /// A sheet entry's harness type; `None` makes it a plain entry again.
    SheetEntryTypePicked {
        pin: Uuid,
        harness_type: Option<String>,
    },
}

/// The facts of `field` on `c`, as shown in its box.
pub(crate) fn field_value(c: &HarnessConnector, field: HarnessField, page_y: f64) -> f64 {
    match field {
        HarnessField::X => c.position.x,
        HarnessField::Y => page_y,
        HarnessField::Width => c.width,
        HarnessField::EntryPitch => c.entry_pitch,
    }
}

/// Whether `field`'s box shows refused text: the panel keeps a draft only
/// for text the field did not take; empty and a lone `-` are a start.
fn draft_refused(draft: Option<&(HarnessField, String)>, field: HarnessField) -> bool {
    draft.is_some_and(|(of, text)| *of == field && crate::numeric_field::text_refused(text, false))
}

pub(super) fn push_harness_connector_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let facts = &ctx.harness;
    let Some(c) = facts.connector.as_ref() else {
        return col;
    };
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let muted = st.muted;
    col = col.push(
        container(
            text(t!(i18n, ids::PANELS_HARNESS_CAPTION,
                "sheet" => facts.sheet_name.as_str(),
                "count" => c.entries.len()))
            .size(TS.caption)
            .color(muted),
        )
        .padding([SP.xs, SP.m])
        .boxed(),
    );
    if let Some(m) = facts.mismatches.first() {
        col = col.push(mismatch_banner(ctx, c.uuid, m));
        col = col.push(entries_list(ctx, c, st));
    }
    let swatch = ctx.canvas_colors.harness_color();
    let expects = entry_expectations(c, &facts.mismatches);
    let art = super::illustrations::HarnessArt {
        nets: &facts.entry_nets,
        expects: &expects,
        colour: Color::from_rgb8(swatch.r, swatch.g, swatch.b),
        warn: crate::styles::ti(ctx.tokens.warning),
    };
    col = col.push(super::illustrations::show(
        super::illustrations::harness_connector(c, art, i18n),
        ctx,
    ));
    let uuid = c.uuid;
    let options: Vec<String> = facts.definitions.iter().map(|d| d.name.clone()).collect();
    let options = if options.contains(&c.harness_type) {
        options
    } else {
        std::iter::once(c.harness_type.clone())
            .chain(options)
            .collect()
    };
    let current = c.harness_type.clone();
    let def_entries = signex_types::schematic::harness_definition(&facts.definitions, &current)
        .map_or(c.entries.len(), |d| d.entries.len());
    col = col.push(
        ctx.collapsible(
            "sel_harness",
            ids::PANELS_HARNESS_SECTION,
            &ctx.collapsed_sections,
            move || {
                Column::new()
                    .spacing(0)
                    .width(Length::Fill)
                    .push(form_pick_row(
                        t!(i18n, ids::PANELS_HARNESS_TYPE),
                        options,
                        current.clone(),
                        move |harness_type| {
                            PanelMsg::Harness(HarnessPanelMsg::TypePicked {
                                connector: uuid,
                                harness_type,
                            })
                        },
                        muted,
                    ))
                    .push(super::part_panel::link_row(
                        t!(i18n, ids::PANELS_HARNESS_DEFINITION),
                        &t!(i18n, ids::PANELS_HARNESS_DEFINITION_LINK,
                            "type" => current.as_str(),
                            "count" => def_entries)
                        .into_string(),
                        PanelMsg::Harness(HarnessPanelMsg::DefinitionsOpened),
                        muted,
                        &ctx.tokens,
                    ))
                    .push(colour_row(
                        t!(i18n, ids::PANELS_HARNESS_COLOUR),
                        t!(i18n, ids::PANELS_HARNESS_COLOUR_DEFAULT),
                        swatch,
                        muted,
                    ))
            },
        )
        .boxed(),
    );
    let field = |field: HarnessField| {
        let shown = match &facts.draft {
            Some((f, text)) if *f == field => text.clone(),
            _ => format!("{:.2}", field_value(c, field, ctx.shown_y(c.position.y))),
        };
        let refused =
            draft_refused(facts.draft.as_ref(), field).then(|| crate::styles::ti(ctx.tokens.error));
        form_edit_row_refusable(
            t!(i18n, field_label(field)),
            &shown,
            muted,
            refused,
            move |text| {
                PanelMsg::Harness(HarnessPanelMsg::FieldTyped {
                    connector: uuid,
                    field,
                    text,
                })
            },
        )
    };
    let rows = [
        field(HarnessField::X),
        field(HarnessField::Y),
        field(HarnessField::Width),
        field(HarnessField::EntryPitch),
    ];
    col = col.push(
        ctx.collapsible(
            "sel_harness_placement",
            ids::PANELS_HARNESS_PLACEMENT,
            &ctx.collapsed_sections,
            move || {
                rows.into_iter()
                    .fold(Column::new().spacing(0).width(Length::Fill), Column::push)
            },
        )
        .boxed(),
    );
    if facts.mismatches.is_empty() {
        col = col.push(entries_list(ctx, c, st));
    }
    let example = c
        .entries
        .first()
        .map(|e| format!("{}.{}", c.harness_type, e.name))
        .unwrap_or_else(|| c.harness_type.clone());
    col.push(
        container(
            text(t!(i18n, ids::PANELS_HARNESS_COMPILED, "example" => example.as_str()))
                .size(TS.caption)
                .color(muted),
        )
        .padding([SP.s, SP.m])
        .boxed(),
    )
}

/// Per entry of `c`: the entry the far side expects in its place, from the
/// first mismatch naming it.
fn entry_expectations(
    c: &HarnessConnector,
    mismatches: &[HarnessMismatchRow],
) -> Vec<Option<String>> {
    c.entries
        .iter()
        .map(|e| {
            mismatches
                .iter()
                .find(|m| m.entry == e.uuid)
                .map(|m| m.expects.clone())
        })
        .collect()
}

fn field_label(field: HarnessField) -> signex_i18n::MessageId {
    match field {
        HarnessField::X => ids::PANELS_HARNESS_X,
        HarnessField::Y => ids::PANELS_HARNESS_Y,
        HarnessField::Width => ids::PANELS_HARNESS_WIDTH,
        HarnessField::EntryPitch => ids::PANELS_HARNESS_PITCH,
    }
}

/// The warn banner directly under the header: what the far side expects,
/// and the two ways out.
fn mismatch_banner<'a>(
    ctx: &'a PanelContext,
    connector: Uuid,
    m: &HarnessMismatchRow,
) -> Element<'a, PanelMsg> {
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let rename = signex_widgets::button(
        text(t!(i18n, ids::PANELS_HARNESS_RENAME,
            "from" => m.has.as_str(),
            "to" => m.expects.as_str()))
        .size(TS.body),
    )
    .padding([SP.xs, SP.m])
    .on_press(PanelMsg::Harness(HarnessPanelMsg::RenameFixClicked {
        connector,
        entry: m.entry,
        to: m.expects.clone(),
    }));
    let go = signex_widgets::button(
        text(t!(i18n, ids::PANELS_HARNESS_GO_TO, "sheet" => m.far.as_str())).size(TS.body),
    )
    .padding([SP.xs, SP.m])
    .on_press(PanelMsg::Harness(HarnessPanelMsg::GoToSheetClicked {
        far: m.far.clone(),
    }));
    let body: Column<Element<'a, PanelMsg>> = Column::new()
        .spacing(SP.s)
        .push(
            text(t!(i18n, ids::PANELS_HARNESS_MISMATCH,
                "far" => m.far.as_str(),
                "expects" => m.expects.as_str(),
                "has" => m.has.as_str()))
            .size(TS.body)
            .boxed(),
        )
        .push(row![rename, go].spacing(SP.s).boxed());
    container(
        container(body)
            .padding(SP.s)
            .width(Length::Fill)
            .style(pill(&ctx.tokens, PillTone::Warn)),
    )
    .padding([SP.s, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// The Entries list: `Entry | Net on this sheet`, a mismatched entry tinted
/// warn with the far side's expectation in a pill.
fn entries_list<'a>(
    ctx: &'a PanelContext,
    c: &HarnessConnector,
    st: PanelStyle,
) -> Element<'a, PanelMsg> {
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let facts = &ctx.harness;
    let mut list: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);
    list = list.push(
        row![
            text(t!(i18n, ids::PANELS_HARNESS_ENTRY))
                .size(TS.caption)
                .color(st.muted)
                .width(Length::Fill),
            text(t!(i18n, ids::PANELS_HARNESS_NET))
                .size(TS.caption)
                .color(st.muted),
        ]
        .padding([SP.xs, SP.m])
        .boxed(),
    );
    for (i, e) in c.entries.iter().enumerate() {
        let net = facts.entry_nets.get(i).cloned().unwrap_or_default();
        let bad = facts.mismatches.iter().find(|m| m.entry == e.uuid);
        let mut line = row![
            text(UserText::new(e.name.clone()))
                .size(TS.body)
                .width(Length::Fill)
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);
        if let Some(m) = bad {
            line = line.push(
                container(
                    text(t!(i18n, ids::PANELS_HARNESS_EXPECTS_PILL,
                        "far" => m.far.as_str(),
                        "expects" => m.expects.as_str()))
                    .size(TS.caption),
                )
                .padding([1.0, SP.s])
                .style(pill(&ctx.tokens, PillTone::Warn))
                .boxed(),
            );
        }
        line = line.push(text(UserText::new(net)).size(TS.body).boxed());
        let cell = container(line).padding([SP.xs, SP.m]).width(Length::Fill);
        list = list.push(match bad {
            Some(_) => cell.style(pill(&ctx.tokens, PillTone::Warn)).boxed(),
            None => cell.boxed(),
        });
    }
    ctx.collapsible_with_action(
        "sel_harness_entries",
        ids::PANELS_HARNESS_ENTRIES,
        &ctx.collapsed_sections,
        add_entry_button(ctx, c.uuid),
        move || list,
    )
    .boxed()
}

/// The + of the Entries header (mockup row 152). A locked connector keeps
/// it clickable; the engine refuses the edit.
fn add_entry_button<'a>(ctx: &'a PanelContext, connector: Uuid) -> Element<'a, PanelMsg> {
    let ink = crate::styles::ti(ctx.tokens.text);
    let glyph = svg(crate::icons::icon_plus(ctx.theme_id))
        .width(TS.body)
        .height(TS.body)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..svg::Style::default()
        });
    let add = signex_widgets::button(glyph)
        .padding(SP.xs)
        .style(crate::panels::form_look::ghost_button)
        .on_press(PanelMsg::Harness(HarnessPanelMsg::AddEntryClicked {
            connector,
        }));
    crate::panels::graphite::hint_tooltip(
        add.boxed(),
        t!(ctx.i18n, ids::PANELS_HARNESS_ADD_ENTRY),
        &ctx.tokens,
        iced::widget::tooltip::Position::Left,
    )
}

/// `label | swatch caption`: the one project harness colour.
fn colour_row<'a>(
    label: signex_i18n::Localized,
    caption: signex_i18n::Localized,
    swatch: signex_types::theme::Color,
    muted: Color,
) -> Element<'a, PanelMsg> {
    let fill = Color::from_rgb8(swatch.r, swatch.g, swatch.b);
    row![
        property_label(label, muted),
        container(Space::new().width(SWATCH).height(SWATCH)).style(move |_| container::Style {
            background: Some(iced::Background::Color(fill)),
            border: iced::Border {
                radius: SWATCH_RADIUS.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        }),
        text(caption).size(TS.caption).color(muted),
    ]
    .padding([4.0, SP.m])
    .spacing(SP.s)
    .align_y(iced::Alignment::Center)
    .boxed()
}

#[cfg(test)]
mod definition_link_tests {
    use super::*;

    /// Mockup row 152: the Definition row is a plain link that opens the
    /// definitions view.
    #[test]
    fn the_definition_link_opens_the_definitions_view() {
        const LINK: &str = "SPI_BUS - 4 entries - edit...";
        let window = iced::Size::new(320.0, 60.0);
        let en = signex_i18n::I18n::en_us();
        let (app, _boot) = crate::app::Signex::new();
        let tokens = app.document_state.panel_ctx.tokens;
        let row = || {
            super::super::part_panel::link_row(
                t!(en, ids::PANELS_HARNESS_DEFINITION),
                LINK,
                PanelMsg::Harness(HarnessPanelMsg::DefinitionsOpened),
                Color::BLACK,
                &tokens,
            )
        };
        let shown = crate::layout_probe::lay_out_texts(row(), window);
        let link = shown.iter().find(|t| t.text == LINK).expect("the link");
        let over = iced::Point::new(link.bounds.x + 2.0, link.bounds.center_y());
        let button = iced::mouse::Button::Left;
        let events = [
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: over }),
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(button)),
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(button)),
        ];
        let pressed = crate::layout_probe::messages_for_events(row(), window, over, &events);
        assert!(
            matches!(
                pressed.as_slice(),
                [PanelMsg::Harness(HarnessPanelMsg::DefinitionsOpened)]
            ),
            "{pressed:?}"
        );
    }
}

#[cfg(test)]
mod add_entry_tests {
    use super::*;
    use crate::app::{EditMsg, Message, Signex, TabInfo, TabKind};
    use signex_types::schematic::{HarnessEntry, Point, SchematicSheet};

    const CONNECTOR: u128 = 1;
    const DOCK: iced::Size = iced::Size::new(300.0, 400.0);

    fn connector(names: &[&str]) -> HarnessConnector {
        HarnessConnector {
            uuid: Uuid::from_u128(CONNECTOR),
            harness_type: "SPI_BUS".into(),
            position: Point::new(10.0, 10.0),
            width: 10.16,
            entry_pitch: 2.54,
            rotation: 0.0,
            entries: names
                .iter()
                .enumerate()
                .map(|(i, n)| HarnessEntry {
                    uuid: Uuid::from_u128(10 + i as u128),
                    name: (*n).into(),
                })
                .collect(),
            locked: false,
        }
    }

    fn app_with(c: HarnessConnector) -> Signex {
        let path = std::path::PathBuf::from("harness.snxsch");
        let mut sheet = SchematicSheet {
            paper_size: "A4".to_string(),
            ..Default::default()
        };
        sheet.harness.connectors.push(c);
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

    fn names(app: &Signex) -> Vec<String> {
        let engine = app.document_state.active_engine().expect("a sheet");
        let c = engine
            .document()
            .harness
            .connector(Uuid::from_u128(CONNECTOR));
        c.expect("the connector")
            .entries
            .iter()
            .map(|e| e.name.clone())
            .collect()
    }

    fn add(app: &mut Signex) {
        let msg = HarnessPanelMsg::AddEntryClicked {
            connector: Uuid::from_u128(CONNECTOR),
        };
        let _ = app.update(Message::Dock(crate::dock::DockMessage::Panel(
            PanelMsg::Harness(msg),
        )));
    }

    /// Mockup row 152: each + adds one entry after the last, named as the
    /// Harness Entry tool names one, and each is one undo step.
    #[test]
    fn each_plus_adds_one_entry_as_one_undo_step() {
        let mut app = app_with(connector(&["SCK"]));
        add(&mut app);
        assert_eq!(names(&app), ["SCK", "ENTRY_1"]);
        add(&mut app);
        assert_eq!(names(&app), ["SCK", "ENTRY_1", "ENTRY_2"]);
        let _ = app.update(Message::Edit(EditMsg::Undo));
        assert_eq!(names(&app), ["SCK", "ENTRY_1"]);
        let _ = app.update(Message::Edit(EditMsg::Undo));
        assert_eq!(names(&app), ["SCK"]);
    }

    /// A locked connector keeps its entries: the engine refuses the edit.
    #[test]
    fn a_locked_connector_takes_no_entry() {
        let mut app = app_with(HarnessConnector {
            locked: true,
            ..connector(&["SCK"])
        });
        add(&mut app);
        assert_eq!(names(&app), ["SCK"]);
    }

    fn entries_section<'a>(ctx: &'a PanelContext, c: &HarnessConnector) -> Element<'a, PanelMsg> {
        let st = PanelStyle {
            muted: Color::BLACK,
            primary: Color::BLACK,
            border_c: Color::BLACK,
            palette: crate::panels::palette::PanelPalette::from_tokens(&ctx.tokens),
        };
        entries_list(ctx, c, st)
    }

    /// Mockup row 152: the + sits at the right end of the Entries header and
    /// adds an entry; the title still folds the section.
    #[test]
    fn the_entries_header_carries_a_plus_that_adds_an_entry() {
        let (app, _boot) = Signex::new();
        let ctx = &app.document_state.panel_ctx;
        let c = connector(&["SCK", "MOSI"]);
        let title = ctx
            .i18n
            .upper_for_display(&t!(ctx.i18n, ids::PANELS_HARNESS_ENTRIES).into_string());
        let texts = crate::layout_probe::lay_out_texts(entries_section(ctx, &c), DOCK);
        let at = crate::layout_probe::find(&texts, &title).bounds;
        let side = crate::panels::form_look::SECTION_SIDE;
        let plus = iced::Point::new(DOCK.width - side - SP.xs - TS.body / 2.0, at.center_y());
        let pressed =
            crate::layout_probe::messages_on_click_at(entries_section(ctx, &c), DOCK, None, plus);
        assert!(
            matches!(
                pressed.as_slice(),
                [PanelMsg::Harness(HarnessPanelMsg::AddEntryClicked { connector })]
                    if *connector == c.uuid
            ),
            "{pressed:?}"
        );
        let folded = crate::layout_probe::messages_on_click(entries_section(ctx, &c), DOCK, &title);
        assert!(
            matches!(folded.as_slice(), [PanelMsg::ToggleSection(key)] if key == "sel_harness_entries"),
            "{folded:?}"
        );
    }
}

#[cfg(test)]
mod expectation_tests {
    use super::*;
    use signex_types::schematic::{HarnessEntry, Point};

    /// Mockup row 152, Entry mismatch: the drawing learns, per entry, the
    /// name the far side expects in its place; other entries expect nothing.
    #[test]
    fn each_mismatched_entry_carries_what_the_far_side_expects() {
        let entry = |n: u128, name: &str| HarnessEntry {
            uuid: Uuid::from_u128(n),
            name: name.into(),
        };
        let c = HarnessConnector {
            uuid: Uuid::from_u128(1),
            harness_type: "SPI_BUS".into(),
            position: Point::new(0.0, 0.0),
            width: 10.16,
            entry_pitch: 2.54,
            rotation: 0.0,
            entries: vec![entry(10, "SCK"), entry(11, "CS")],
            locked: false,
        };
        let mismatch = HarnessMismatchRow {
            entry: Uuid::from_u128(11),
            has: "CS".into(),
            expects: "CS_ADC".into(),
            far: "sensor".into(),
        };
        assert_eq!(
            entry_expectations(&c, &[mismatch]),
            [None, Some("CS_ADC".to_string())]
        );
        assert_eq!(entry_expectations(&c, &[]), [None, None]);
    }
}

#[cfg(test)]
mod refused_look_tests {
    use super::{HarnessField, draft_refused};

    /// #2792: Width and Entry pitch outline the text they refused, in their
    /// own box only.
    #[test]
    fn a_refused_draft_marks_only_its_own_box() {
        for field in [HarnessField::Width, HarnessField::EntryPitch] {
            let draft = (field, "0".to_string());
            assert!(draft_refused(Some(&draft), field), "{field:?}");
            assert!(!draft_refused(Some(&draft), HarnessField::X));
            let start = (field, "-".to_string());
            assert!(!draft_refused(Some(&start), field), "a number's start");
        }
        assert!(!draft_refused(None, HarnessField::Width));
    }
}
