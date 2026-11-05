//! Sheet colour and page origin commands (#2034): one undo step each on the
//! PAPER patch, a repeat changes nothing, undo restores the old value.

use crate::test_support::test_sheet;
use crate::*;
use signex_types::schematic::{PageOrigin, SheetPageSettings};
use signex_types::theme::Color;

const GRAY: Color = Color::new(0xd0, 0xd0, 0xd0, 0xff);

fn run(engine: &mut Engine, cmd: Command) -> CommandResult {
    engine.execute(cmd).expect("command")
}

#[test]
fn sheet_color_sets_no_ops_and_undoes() {
    let mut engine = Engine::new(test_sheet()).expect("engine");
    let result = run(&mut engine, Command::SetSheetColor { color: Some(GRAY) });
    assert!(result.changed);
    assert_eq!(
        result.patch_pair.expect("patch").document,
        DocumentPatch::PAPER
    );
    assert_eq!(engine.document().page.color, Some(GRAY));
    assert!(!run(&mut engine, Command::SetSheetColor { color: Some(GRAY) }).changed);

    engine.undo().expect("undo");
    assert_eq!(engine.document().page, SheetPageSettings::default());
    engine.redo().expect("redo");
    assert_eq!(engine.document().page.color, Some(GRAY));
}

#[test]
fn page_origin_sets_keeps_the_colour_and_undoes() {
    let mut engine = Engine::new(test_sheet()).expect("engine");
    run(&mut engine, Command::SetSheetColor { color: Some(GRAY) });
    let result = run(
        &mut engine,
        Command::SetPageOrigin {
            origin: PageOrigin::UpperLeft,
        },
    );
    assert!(result.changed);
    assert_eq!(
        engine.document().page,
        SheetPageSettings {
            color: Some(GRAY),
            origin: PageOrigin::UpperLeft,
        }
    );
    engine.undo().expect("undo");
    assert_eq!(engine.document().page.origin, PageOrigin::LowerLeft);
    assert_eq!(engine.document().page.color, Some(GRAY));
}
