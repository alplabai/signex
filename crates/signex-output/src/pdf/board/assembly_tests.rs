use super::*;
use crate::assembly::tests::{check_golden, real_board};

const BOTH: [BoardLayerSet; 2] = [BoardLayerSet::AssemblyTop, BoardLayerSet::AssemblyBottom];

fn options(sets: &[BoardLayerSet], pages: BoardPrintPages) -> BoardPrintOptions {
    BoardPrintOptions {
        sets: sets.to_vec(),
        pages,
        ..BoardPrintOptions::default()
    }
}

#[test]
fn an_assembly_drawing_is_a_page_of_its_own_after_the_composite() {
    let sets = [
        BoardLayerSet::AssemblyBottom,
        BoardLayerSet::Top,
        BoardLayerSet::AssemblyTop,
        BoardLayerSet::Bottom,
    ];
    let plan = options(&sets, BoardPrintPages::Composite).page_plan();
    assert_eq!(
        plan,
        [
            (vec![BoardLayerSet::Top, BoardLayerSet::Bottom], false),
            (vec![BoardLayerSet::AssemblyTop], false),
            (vec![BoardLayerSet::AssemblyBottom], true),
        ]
    );
    let only = options(&BOTH, BoardPrintPages::Composite).page_plan();
    assert_eq!(only.len(), 2, "no empty composite page");
    let flat = BoardPrintOptions {
        mirror_bottom: false,
        ..options(&BOTH, BoardPrintPages::PerSet)
    };
    assert!(flat.page_plan().iter().all(|(_, mirrored)| !mirrored));
}

#[test]
fn the_assembly_sets_take_the_sides_documentation_layers() {
    use signex_types::pcb_layer::TechnicalLayer as T;
    let t = PcbLayer::Technical;
    assert!(BoardLayerSet::AssemblyTop.contains(t(T::FabTop)));
    assert!(BoardLayerSet::AssemblyTop.contains(t(T::Dimensions)));
    assert!(!BoardLayerSet::AssemblyTop.contains(t(T::SilkscreenTop)));
    assert!(BoardLayerSet::AssemblyBottom.contains(t(T::DesignatorBottom)));
    assert!(!BoardLayerSet::AssemblyBottom.contains(t(T::FiducialTop)));
    assert_eq!(
        BoardLayerSet::from_id("assembly_bottom"),
        Some(BoardLayerSet::AssemblyBottom)
    );
}

#[test]
fn an_assembly_page_is_framed_on_the_boards_edge_cuts() {
    let print = BoardPrintOptions {
        scale: BoardPrintScale::OneToOne,
        ..options(&BOTH, BoardPrintPages::PerSet)
    };
    let out = board_pdf(&real_board(), &print, &BoardPrintText::default()).expect("pdf");
    assert_eq!(out.pages.len(), 2);
    for page in &out.pages {
        let (w, h) = page.printed_size_mm;
        assert!((w - 50.0).abs() < 1e-9 && (h - 40.0).abs() < 1e-9, "{w} x {h}");
    }
    assert_eq!(page_mirrors(&out.pages), [false, true]);
}

fn page_mirrors(pages: &[BoardPdfPage]) -> Vec<bool> {
    pages.iter().map(|p| p.mirrored).collect()
}

/// Every number in a content stream to two decimals, so the golden does not
/// hang on the last bits of a sine.
fn normalised(stream: &[u8]) -> String {
    String::from_utf8_lossy(stream)
        .lines()
        .map(|line| {
            line.split(' ')
                .map(|token| match token.parse::<f64>() {
                    Ok(v) if token.contains('.') => {
                        let v = format!("{v:.2}");
                        if v == "-0.00" { "0.00".to_string() } else { v }
                    }
                    _ => token.to_string(),
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// The top and bottom assembly pages of a real board, fitted on A4 with the
/// bottom mirrored and a title block: their content streams.
#[test]
fn the_real_board_assembly_pages_match_their_goldens() {
    let text = BoardPrintText {
        title: "mixed-tht-smd".to_string(),
        sets: vec![
            (BoardLayerSet::AssemblyTop, "Assembly Top".to_string()),
            (BoardLayerSet::AssemblyBottom, "Assembly Bottom".to_string()),
        ],
        mirrored: "mirrored".to_string(),
    };
    let mut fonts = FontUsage::new();
    let print = options(&BOTH, BoardPrintPages::PerSet);
    let (contents, pages) =
        render_pages(&real_board(), &print, &text, &mut fonts).expect("pages");
    assert_eq!(pages.len(), 2);
    check_golden("mixed-tht-smd-top.pdf.txt", &normalised(&contents[0]));
    check_golden("mixed-tht-smd-bottom.pdf.txt", &normalised(&contents[1]));
}
