//! The symbol editor's Properties panel (#122, #123): the UUID row, the
//! Pins tab list and its filter, chips and issue marks, the way back from
//! Pin properties, and what the illustrations leave out.

use iced::widget::Column;
use iced::{Element, Size, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::theme::{ThemeId, ThemeTokens, theme_tokens};
use signex_widgets::property_illustration::{FieldId, IllustrationHover, Primitive};
use signex_widgets::{raw, text};

use super::illustration::{self, field};
use super::pins_tab::{listed_pins, part_chips, pin_issues};
use super::rows::{Form, Look, TS};
use super::*;
use crate::layout_probe::{find, lay_out_texts, messages_on_click, natural};
use crate::library::editor::symbol::{PartChip, SymbolPropsMsg, SymbolPropsTab};
use crate::panels::symbol_context::PartDrawing;
use crate::panels::{
    CollapsedSections, GraphicKindSummary, GraphicSummary, SymbolDisplayOptions, SymbolPinDetails,
    SymbolPinSummary,
};

const UUID: &str = "a1cf3b2e-4cdd-4cdd-9420-8bdf89c0ffee";

fn pin(idx: usize, number: &str, name: &str, electrical: &str, part: u8) -> SymbolPinSummary {
    SymbolPinSummary {
        idx,
        number: number.to_string(),
        name: name.to_string(),
        electrical: electrical.to_string(),
        position: [-7.62, 0.0],
        orientation: "Right".to_string(),
        length: 2.54,
        details: SymbolPinDetails {
            description: String::new(),
            function: Vec::new(),
            pin_package_length: None,
            propagation_delay_ns: None,
            designator_visible: true,
            name_visible: true,
            inside_symbol: signex_library::PinSymbolKind::None,
            inside_edge_symbol: signex_library::PinSymbolKind::None,
            outside_edge_symbol: signex_library::PinSymbolKind::None,
            outside_symbol: signex_library::PinSymbolKind::None,
            hidden: false,
            locked: false,
            part_number: part,
        },
    }
}

/// The op-amp of the mockup: two parts and the shared supply pins.
fn opamp_pins() -> Vec<SymbolPinSummary> {
    vec![
        pin(0, "1", "OUTA", "Output", 1),
        pin(1, "2", "-INA", "Input", 1),
        pin(2, "3", "+INA", "Input", 1),
        pin(3, "4", "V-", "PowerInput", 0),
        pin(4, "5", "+INB", "Input", 2),
        pin(5, "6", "-INB", "Input", 2),
        pin(6, "7", "OUTB", "Output", 2),
        pin(7, "8", "V+", "PowerInput", 0),
    ]
}

fn ctx(pins: Vec<SymbolPinSummary>, selected: SymbolEditorSelection) -> SymbolEditorPanelContext {
    SymbolEditorPanelContext {
        path: std::path::PathBuf::from("opamps.snxsym"),
        symbol_name: "OPA2333".to_string(),
        symbol_designator: "U?".to_string(),
        symbol_comment: "*".to_string(),
        symbol_description: String::new(),
        symbol_component_type: signex_library::ComponentType::Standard,
        symbol_mirrored: false,
        symbol_local_fill_color: None,
        symbol_local_line_color: None,
        symbol_local_pin_color: None,
        symbol_uuid: uuid::Uuid::nil(),
        pins,
        graphics: Vec::new(),
        selected,
        graphic_fill_picker: None,
        local_color_picker: None,
        symbols_in_file: Vec::new(),
        delete_warning: None,
        active_idx: 0,
        active_part: 1,
        active_max_part: 2,
        active_has_part_zero: true,
        active_body: signex_types::schematic::BodyStyle::Normal,
        part_drawing: PartDrawing::default(),
        props: Default::default(),
        display: SymbolDisplayOptions::default(),
    }
}

struct Fixture {
    tokens: ThemeTokens,
    i18n: std::sync::Arc<I18n>,
    hover: IllustrationHover,
    toggled: CollapsedSections,
}

impl Fixture {
    fn new() -> Self {
        Self {
            tokens: theme_tokens(ThemeId::GraphiteLight),
            i18n: I18n::en_us(),
            hover: IllustrationHover::default(),
            toggled: CollapsedSections::new(),
        }
    }

    fn form(&self) -> Form<'_> {
        Form {
            look: Look::of(&self.tokens),
            tokens: &self.tokens,
            i18n: &self.i18n,
            hover: &self.hover,
            toggled: &self.toggled,
            draft: None,
        }
    }
}

fn stacked(rows: Vec<Element<'_, PanelMsg>>) -> Element<'_, PanelMsg> {
    Column::with_children(rows).boxed()
}

const DOCK: Size = Size::new(300.0, 2000.0);

/// #2262: the UUID wrapped onto a second line and ran past the dock
/// edge. It stays on one line inside the row now, cut with an ellipsis.
#[test]
fn the_uuid_row_stays_on_one_line_inside_a_narrow_dock() {
    let fx = Fixture::new();
    let form = fx.form();
    let dock = Size::new(240.0, 400.0);
    let row = form.field(
        t!(fx.i18n, ids::EDITORS_SYMPROPS_UUID),
        form.read_only(UserText::new(UUID).into(), true),
        None,
    );
    let value = find(&lay_out_texts(row, dock), UUID).bounds;
    let one_line = natural::<PanelMsg>(
        text(raw(UUID)).size(TS.body).font(rows::mono()).boxed(),
        UUID,
    );
    assert!(
        one_line.width > value.width,
        "the dock must be narrower than the UUID: {value:?}, natural {one_line:?}"
    );
    assert!(
        value.height <= one_line.height + 0.5,
        "the UUID wraps: {value:?}, one line is {one_line:?}"
    );
    assert!(
        value.x + value.width <= dock.width + 0.5,
        "the UUID runs past the dock edge: {value:?}"
    );
}

#[test]
fn the_part_chips_are_all_then_each_part_with_its_pin_count() {
    assert_eq!(
        part_chips(&opamp_pins()),
        vec![
            (PartChip::All, 8),
            (PartChip::Part(0), 2),
            (PartChip::Part(1), 3),
            (PartChip::Part(2), 3),
        ]
    );
}

#[test]
fn the_pins_tab_lists_the_pins_of_the_chosen_part_chip() {
    let pins = opamp_pins();
    let en = I18n::en_us();
    let numbers = |chip| -> Vec<String> {
        listed_pins(&pins, chip, "", &en)
            .iter()
            .map(|p| p.number.clone())
            .collect()
    };
    assert_eq!(numbers(PartChip::All).len(), 8);
    assert_eq!(numbers(PartChip::Part(2)), ["5", "6", "7"]);
    assert_eq!(numbers(PartChip::Part(0)), ["4", "8"]);
    // A chip whose part no pin uses any more lists every pin.
    assert_eq!(numbers(PartChip::Part(9)).len(), 8);
}

#[test]
fn the_pin_filter_matches_a_number_or_a_name() {
    let pins = opamp_pins();
    let en = I18n::en_us();
    let names = |filter: &str| -> Vec<String> {
        listed_pins(&pins, PartChip::All, filter, &en)
            .iter()
            .map(|p| p.name.clone())
            .collect()
    };
    assert_eq!(names("7"), ["OUTB"]);
    assert_eq!(names("outa"), ["OUTA"]);
    assert_eq!(names("inb"), ["+INB", "-INB"]);
    // The typographic minus reads as a hyphen.
    assert_eq!(names("\u{2212}INA"), ["-INA"]);
    assert!(names("nothing").is_empty());
}

#[test]
fn a_duplicate_number_marks_both_rows_and_an_empty_name_is_flagged() {
    let mut pins = opamp_pins();
    pins[6].number = "1".to_string();
    pins[2].name = "  ".to_string();
    let issues = pin_issues(&pins);
    assert!(issues[0].bad_number && issues[6].bad_number);
    assert!(!issues[1].bad_number);
    assert!(issues[2].no_name && !issues[2].bad_number);
    assert_eq!(issues.iter().filter(|i| i.any()).count(), 3);
}

#[test]
fn an_empty_number_is_a_bad_number() {
    let pins = vec![pin(0, "", "A", "Passive", 1)];
    assert!(pin_issues(&pins)[0].bad_number);
}

#[test]
fn a_row_press_selects_its_pin() {
    let fx = Fixture::new();
    let mut sym = ctx(opamp_pins(), SymbolEditorSelection::None);
    sym.props.tab = SymbolPropsTab::Pins;
    let rows = pins_tab::view(&sym, &fx.form());
    let messages = messages_on_click(stacked(rows), DOCK, "OUTB");
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, PanelMsg::SymEditorSelectPin(6))),
        "{messages:?}"
    );
}

#[test]
fn the_pins_tab_shows_only_the_rows_the_filter_keeps() {
    let fx = Fixture::new();
    let mut sym = ctx(opamp_pins(), SymbolEditorSelection::None);
    sym.props.pin_filter = "OUTB".to_string();
    let texts = lay_out_texts(stacked(pins_tab::view(&sym, &fx.form())), DOCK);
    assert!(texts.iter().any(|t| t.text == "OUTB"));
    assert!(!texts.iter().any(|t| t.text == "OUTA"));
}

#[test]
fn a_filter_that_keeps_nothing_says_so() {
    let fx = Fixture::new();
    let mut sym = ctx(opamp_pins(), SymbolEditorSelection::None);
    sym.props.pin_filter = "zzz".to_string();
    let texts = lay_out_texts(stacked(pins_tab::view(&sym, &fx.form())), DOCK);
    let no_match = t!(fx.i18n, ids::EDITORS_SYMPROPS_NO_MATCH).into_string();
    find(&texts, &no_match);
}

#[test]
fn back_to_pins_on_the_pin_properties_asks_for_the_pins_tab() {
    let fx = Fixture::new();
    let pins = opamp_pins();
    let selected = SymbolEditorSelection::Pin(pins[2].clone());
    let sym = ctx(pins, selected);
    let SymbolEditorSelection::Pin(p) = &sym.selected else {
        unreachable!()
    };
    let rows = pin::view(&sym, p, &fx.form());
    let back = t!(fx.i18n, ids::EDITORS_SYMPROPS_BACK_TO_PINS).into_string();
    let messages = messages_on_click(stacked(rows), DOCK, &back);
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, PanelMsg::SymbolProps(SymbolPropsMsg::PinsBackPressed))),
        "{messages:?}"
    );
}

fn label_texts(ill: &signex_widgets::property_illustration::Illustration) -> Vec<String> {
    ill.visible_parts()
        .filter_map(|p| match &p.primitive {
            Primitive::Label(l) => Some(l.text.as_str().to_string()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_pin_illustration_drops_a_name_or_number_that_is_not_shown() {
    let en = I18n::en_us();
    let mut p = pin(0, "3", "+INA", "Input", 1);
    let shown = label_texts(&illustration::pin(&p, &en));
    assert!(shown.contains(&"+INA".to_string()) && shown.contains(&"3".to_string()));
    p.details.name_visible = false;
    let shown = label_texts(&illustration::pin(&p, &en));
    assert!(!shown.contains(&"+INA".to_string()), "{shown:?}");
    assert!(shown.contains(&"3".to_string()));
    p.details.designator_visible = false;
    assert!(!label_texts(&illustration::pin(&p, &en)).contains(&"3".to_string()));
}

#[test]
fn a_rectangle_without_fill_drops_its_fill_from_the_illustration() {
    let en = I18n::en_us();
    let mut g = GraphicSummary {
        idx: 0,
        kind: GraphicKindSummary::Rectangle {
            from: [-5.0, -4.0],
            to: [5.0, 4.0],
        },
        stroke_width: 0.2,
        fill: Some([255, 255, 200, 255]),
    };
    let fill = FieldId::new(field::G_FILL);
    let has_fill = |g: &GraphicSummary| {
        illustration::graphic(g, &[], &en)
            .unwrap()
            .visible_parts()
            .any(|p| p.is_linked_to(&fill))
    };
    assert!(has_fill(&g));
    g.fill = None;
    assert!(!has_fill(&g));
}

#[test]
fn a_hovered_field_row_lights_the_callout_linked_to_it() {
    let en = I18n::en_us();
    let ill = illustration::pin(&pin(0, "3", "+INA", "Input", 1), &en);
    let mut hover = IllustrationHover::default();
    hover.update(
        signex_widgets::property_illustration::IllustrationMsg::FieldHovered(FieldId::new(
            field::PIN_LEN,
        )),
    );
    assert!(ill.visible_parts().any(|p| hover.is_part_highlighted(p)));
}

#[test]
fn the_part_pill_names_the_shared_part_and_lettered_parts() {
    let en = I18n::en_us();
    let shown = |part| {
        part_name(part, &en)
            .into_string()
            .replace(['\u{2068}', '\u{2069}'], "")
    };
    assert_eq!(shown(0), "Part 0 (shared)");
    assert_eq!(shown(2), "Part 2 (B)");
    assert_eq!(shown(30), "Part 30");
}

/// A section header toggled through `Signex::update` shows on the next
/// frame: Properties reads the live `collapsed_sections`, not a copy taken
/// when the symbol context was built.
#[test]
fn a_toggled_section_opens_without_a_context_rebuild() {
    use crate::app::{Message, Signex, SymbolDocument, TabInfo, TabKind};
    use crate::dock::DockMessage;
    use crate::library::editor::symbol::SymbolPropsMsg;
    use signex_library::{Symbol, SymbolFile};

    let (mut app, _task) = Signex::new();
    let path = std::path::PathBuf::from("toggle.snxsym");
    let file = SymbolFile::from_symbol(Symbol::empty("TOGGLE"));
    app.document_state
        .symbol_editors
        .insert(path.clone(), SymbolDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "TOGGLE".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(path),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    let panel = |app: &mut Signex, msg: PanelMsg| {
        let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
    };
    panel(
        &mut app,
        PanelMsg::SymbolProps(SymbolPropsMsg::TabPicked(SymbolPropsTab::Symbol)),
    );
    let bound = t!(app.ui_state.i18n, ids::EDITORS_SYMPROPS_VALUE_BOUND).to_string();
    let fields_open = |app: &Signex| {
        let view = crate::panels::view_panel(
            crate::panels::PanelKind::Properties,
            &app.document_state.panel_ctx,
        );
        lay_out_texts(view, DOCK).iter().any(|t| t.text == bound)
    };
    assert!(app.document_state.panel_ctx.symbol_editor.is_some());
    assert!(!fields_open(&app), "Fields starts folded");
    panel(&mut app, PanelMsg::ToggleSection("symprops.fields".into()));
    assert!(fields_open(&app), "the toggle opened Fields");
}

/// The Pins tab keeps the bulk edit of the old Pins panel reachable: Edit
/// pins in table… asks for the table, and in the table a row's Hidden cell
/// toggles that pin through the Pin properties' message.
#[test]
fn the_pin_table_opens_from_the_pins_tab_and_toggles_hidden_in_place() {
    let fx = Fixture::new();
    let mut sym = ctx(opamp_pins(), SymbolEditorSelection::None);
    sym.props.tab = SymbolPropsTab::Pins;
    let open = t!(fx.i18n, ids::EDITORS_SYMPROPS_PIN_TABLE_OPEN).into_string();
    let messages = messages_on_click(stacked(pins_tab::view(&sym, &fx.form())), DOCK, &open);
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, PanelMsg::SymbolProps(SymbolPropsMsg::PinTableToggled))),
        "{messages:?}"
    );

    sym.props.pin_table = true;
    sym.props.pin_filter = "OUTB".to_string();
    let texts = lay_out_texts(stacked(pins_tab::view(&sym, &fx.form())), DOCK);
    let close = t!(fx.i18n, ids::EDITORS_SYMPROPS_PIN_TABLE_CLOSE).into_string();
    find(&texts, &close);
    find(
        &texts,
        &t!(fx.i18n, ids::EDITORS_PINS_COLUMN_ORIENTATION).into_string(),
    );
    let shown = t!(fx.i18n, ids::EDITORS_PINS_SHOWN).into_string();
    let messages = messages_on_click(stacked(pins_tab::view(&sym, &fx.form())), DOCK, &shown);
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, PanelMsg::SymEditorTogglePinHidden(6))),
        "{messages:?}"
    );
}
