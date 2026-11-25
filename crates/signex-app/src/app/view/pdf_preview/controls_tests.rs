//! #2060: the Page Setup size and scale pick lists show the entry in force
//! by its catalog name and hand back the typed value picked.

use iced::{Event, Size, keyboard, mouse};
use signex_output::{PageSize, PdfScale, PdfScaleMode};
use signex_types::schematic::SheetOrientation;

use crate::app::state::{PageSetupInputs, PreviewState};
use crate::app::{Message, PrintPreviewMsg, Signex};
use crate::layout_probe::{find, lay_out_texts, messages_for_events};

const WINDOW: Size = Size::new(400.0, 800.0);

fn preview(size: PageSize, scale: PdfScale) -> PreviewState {
    let mut preview = PreviewState::blank_for_tests();
    preview.pdf_options.page_size = size;
    preview.pdf_options.scale = scale;
    preview.pdf_options.orientation = SheetOrientation::Portrait;
    preview.page_setup = PageSetupInputs::from_options(&preview.pdf_options);
    preview
}

/// The texts of the Page Setup section, as laid out.
fn section_texts(app: &Signex, preview: &PreviewState) -> Vec<String> {
    lay_out_texts(app.view_pdf_page_setup(preview), WINDOW)
        .into_iter()
        .map(|t| t.text)
        .collect()
}

/// The messages Command + one wheel notch (`lines`, negative = next entry)
/// over the shown text `label` publishes from the section.
fn wheel_over(app: &Signex, preview: &PreviewState, label: &str, lines: f32) -> Vec<Message> {
    let texts = lay_out_texts(app.view_pdf_page_setup(preview), WINDOW);
    let at = find(&texts, label).bounds.center();
    messages_for_events(
        app.view_pdf_page_setup(preview),
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

#[test]
fn the_pick_lists_show_the_entries_in_force() {
    let (app, _task) = Signex::new();
    let shown = section_texts(&app, &preview(PageSize::UsLetter, PdfScale::Percent(50.0)));
    for entry in ["US Letter", "Percent"] {
        assert!(shown.iter().any(|t| t == entry), "{entry} in {shown:?}");
    }
    // Mockup row 111 names the ISO sizes bare ("A4").
    for other in ["A0", "Fit to Page"] {
        assert!(!shown.iter().any(|t| t == other), "{other} in {shown:?}");
    }
    let shown = section_texts(&app, &preview(PageSize::IsoA4, PdfScale::OneToOne));
    for entry in ["A4", "1:1"] {
        assert!(shown.iter().any(|t| t == entry), "{entry} in {shown:?}");
    }
}

/// Picking from each list hands back the entry's typed value.
#[test]
fn the_pick_lists_hand_back_the_picked_value() {
    let (app, _task) = Signex::new();
    let state = preview(PageSize::UsLetter, PdfScale::Percent(50.0));
    let sizes: Vec<PageSize> = wheel_over(&app, &state, "US Letter", -1.0)
        .into_iter()
        .filter_map(|m| match m {
            Message::PrintPreview(PrintPreviewMsg::SetPageSize(size)) => Some(size),
            _ => None,
        })
        .collect();
    assert_eq!(sizes, [PageSize::UsLegal]);
    let modes: Vec<PdfScaleMode> = wheel_over(&app, &state, "Percent", 1.0)
        .into_iter()
        .filter_map(|m| match m {
            Message::PrintPreview(PrintPreviewMsg::SetScaleMode(mode)) => Some(mode),
            _ => None,
        })
        .collect();
    assert_eq!(modes, [PdfScaleMode::OneToOne]);
}
