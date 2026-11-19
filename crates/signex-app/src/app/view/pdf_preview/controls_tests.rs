//! #2060: the page-setup pick lists show the entry in force and hand back
//! the typed value picked; each margin field sits beside its edge's name.

use iced::{Event, Point, Size, keyboard, mouse};
use signex_output::{MarginEdge, PageSize, PdfScale, PdfScaleMode};
use signex_types::schematic::SheetOrientation;

use super::tests::empty_preview;
use crate::app::state::{PageSetupInputs, PreviewState};
use crate::app::{Message, PrintPreviewMsg, Signex};
use crate::layout_probe::{find, lay_out_texts, messages_for_events};

const WINDOW: Size = Size::new(1600.0, 200.0);

fn preview(size: PageSize, scale: PdfScale, orientation: SheetOrientation) -> PreviewState {
    let mut preview = empty_preview();
    preview.pdf_options.page_size = size;
    preview.pdf_options.scale = scale;
    preview.pdf_options.orientation = orientation;
    preview.page_setup = PageSetupInputs::from_options(&preview.pdf_options);
    preview
}

/// The texts of the Preview-tab control row, as laid out (no marks stripped).
fn control_texts(app: &Signex, preview: &PreviewState) -> Vec<String> {
    lay_out_texts(app.view_pdf_page_controls(preview), WINDOW)
        .into_iter()
        .map(|t| t.text)
        .collect()
}

/// The messages Command + one wheel notch (`lines`, negative = next entry)
/// over the shown text `label` publishes from the control row.
fn wheel_over(app: &Signex, preview: &PreviewState, label: &str, lines: f32) -> Vec<Message> {
    let texts = lay_out_texts(app.view_pdf_page_controls(preview), WINDOW);
    let at = find(&texts, label).bounds.center();
    messages_for_events(
        app.view_pdf_page_controls(preview),
        WINDOW,
        at,
        &[
            Event::Keyboard(keyboard::Event::ModifiersChanged(
                keyboard::Modifiers::COMMAND,
            )),
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y: lines },
            }),
        ],
    )
}

fn picked_sizes(messages: Vec<Message>) -> Vec<PageSize> {
    messages
        .into_iter()
        .filter_map(|m| match m {
            Message::PrintPreview(PrintPreviewMsg::SetPageSize(size)) => Some(size),
            _ => None,
        })
        .collect()
}

#[test]
fn the_pick_lists_show_the_entries_in_force() {
    let (app, _task) = Signex::new();
    let shown = control_texts(
        &app,
        &preview(
            PageSize::UsLetter,
            PdfScale::Percent(50.0),
            SheetOrientation::Portrait,
        ),
    );
    for entry in ["US Letter", "Percent", "Portrait"] {
        assert!(shown.iter().any(|t| t == entry), "{entry} in {shown:?}");
    }
    for other in ["ISO A0", "Fit to Page", "Landscape"] {
        assert!(!shown.iter().any(|t| t == other), "{other} in {shown:?}");
    }
    // Landscape is the orientation list's second entry, not its first.
    let shown = control_texts(
        &app,
        &preview(
            PageSize::IsoA4,
            PdfScale::OneToOne,
            SheetOrientation::Landscape,
        ),
    );
    for entry in ["ISO A4", "1:1", "Landscape"] {
        assert!(shown.iter().any(|t| t == entry), "{entry} in {shown:?}");
    }
    assert!(!shown.iter().any(|t| t == "Portrait"), "{shown:?}");
}

/// A Custom page shows "Custom" and its orientation as text, not a picker.
#[test]
fn a_custom_page_shows_its_orientation_as_text() {
    let (app, _task) = Signex::new();
    let custom = PageSize::Custom {
        width_mm: 100.0,
        height_mm: 200.0,
    };
    let shown = control_texts(
        &app,
        &preview(custom, PdfScale::FitToPage, SheetOrientation::Portrait),
    );
    for entry in ["Custom", "Portrait", "Fit to Page"] {
        assert!(shown.iter().any(|t| t == entry), "{entry} in {shown:?}");
    }
    assert!(!shown.iter().any(|t| t == "Landscape"), "{shown:?}");
}

/// Picking from each list hands back the entry's typed value.
#[test]
fn the_pick_lists_hand_back_the_picked_value() {
    let (app, _task) = Signex::new();
    let state = preview(
        PageSize::UsLetter,
        PdfScale::Percent(50.0),
        SheetOrientation::Portrait,
    );
    assert_eq!(
        picked_sizes(wheel_over(&app, &state, "US Letter", -1.0)),
        [PageSize::UsLegal]
    );
    let modes: Vec<PdfScaleMode> = wheel_over(&app, &state, "Percent", 1.0)
        .into_iter()
        .filter_map(|m| match m {
            Message::PrintPreview(PrintPreviewMsg::SetScaleMode(mode)) => Some(mode),
            _ => None,
        })
        .collect();
    assert_eq!(modes, [PdfScaleMode::OneToOne]);
    let orientations: Vec<SheetOrientation> = wheel_over(&app, &state, "Portrait", -1.0)
        .into_iter()
        .filter_map(|m| match m {
            Message::PrintPreview(PrintPreviewMsg::SetOrientation(o)) => Some(o),
            _ => None,
        })
        .collect();
    assert_eq!(orientations, [SheetOrientation::Landscape]);
}

/// Typing into the field right of the "Top" label edits the top margin.
#[test]
fn the_field_beside_top_edits_the_top_margin() {
    let (app, _task) = Signex::new();
    let state = empty_preview();
    let window = Size::new(1600.0, 400.0);
    let texts = lay_out_texts(app.view_pdf_page_setup_section(&state), window);
    let top = find(&texts, "Top").bounds;
    let in_field = Point::new(top.x + top.width + 30.0, top.center_y());
    let digit = keyboard::Key::Character("5".into());
    let messages = messages_for_events(
        app.view_pdf_page_setup_section(&state),
        window,
        in_field,
        &[
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: digit.clone(),
                modified_key: digit,
                physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Digit5),
                location: keyboard::Location::Standard,
                modifiers: keyboard::Modifiers::default(),
                text: Some("5".into()),
                repeat: false,
            }),
        ],
    );
    let edges: Vec<MarginEdge> = messages
        .into_iter()
        .filter_map(|m| match m {
            Message::PrintPreview(PrintPreviewMsg::SetMargin(edge, _)) => Some(edge),
            _ => None,
        })
        .collect();
    assert_eq!(edges, [MarginEdge::Top]);
}
