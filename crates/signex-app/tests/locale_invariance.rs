// Standalone binary: the config root override is process-wide, and the test switches the saved UI language in it.

//! #2056: the UI language never reaches a file. The same fixture, driven
//! headless under `en-US`, `tr-TR` and the `en-XA` pseudolocale, writes
//! byte-identical project, sheet, waiver, ERC report, BOM, netlist and PDF
//! files.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use signex_app::app::{
    BomPreviewMsg, FileMsg, Message, PrintPreviewMsg, ProjectMsg, Signex, UiMsg,
};
use signex_app::config_root::set_config_root_override;
use signex_app::dock::DockMessage;
use signex_app::fonts::catalog::{FontSet, install_before_boot};
use signex_app::headless::CommandArgs;
use signex_app::keymap::AppCommandId;
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::{ErcWaiverMsg, PanelMsg};
use signex_app::task_driver::TaskDriver;
use signex_i18n::LanguagePref;
use signex_types::project::{ErcRules, RuleProfile};

const LOCALES: [&str; 3] = ["en-US", "tr-TR", "en-XA"];

/// Output name -> bytes, in a fixed order.
type Outputs = BTreeMap<&'static str, Vec<u8>>;

fn rules() -> RuleProfile {
    RuleProfile::new("Invariance", "1", ErcRules::default())
}

fn settle(app: &mut Signex, task: iced::Task<Message>) {
    TaskDriver::new().settle(app, task).expect("task settled");
}

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    settle(app, task);
}

/// Answer the next dialog with `path`, then send `message`.
fn send_answered(app: &mut Signex, path: &Path, message: Message) {
    app.headless_answers()
        .expect("headless")
        .replace_all([("path".to_string(), path.display().to_string())]);
    send(app, message);
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("{} was not written: {e}", path.display()))
}

/// The waiver's date is today's, from the clock: pinned to a placeholder so
/// a run that crosses midnight does not read as a locale difference.
fn pin_date(bytes: Vec<u8>) -> Vec<u8> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    String::from_utf8_lossy(&bytes)
        .replace(&today, "<today>")
        .into_bytes()
}

/// The sample is ERC-clean; a loose wire with a fixed id gives the ERC
/// report and the waiver something to say.
fn add_dangling_wire(sheet: &Path) {
    use signex_types::format::SnxSchematic;
    use signex_types::schematic::{Point, Wire};

    let text = std::fs::read_to_string(sheet).expect("sample sheet");
    let mut file = SnxSchematic::parse(&text).expect("sample parses");
    file.sheet.wires.push(Wire {
        uuid: uuid::Uuid::from_u128(0x2056),
        start: Point::new(250.0, 20.0),
        end: Point::new(260.0, 20.0),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
        locked: false,
    });
    std::fs::write(sheet, file.write_string().expect("encode")).expect("seed fixture");
}

/// A new sheet is stamped with a fresh random `schematic_id`; masked so the
/// rest of the file is compared.
fn mask_schematic_id(bytes: Vec<u8>) -> Vec<u8> {
    // Only the value changes; every line ending and the final newline stay
    // as written, so a CRLF/LF difference between runs still shows.
    let text = String::from_utf8_lossy(&bytes);
    text.split_inclusive('\n')
        .map(|line| match line.starts_with("schematic_id = ") {
            true => {
                let ending = &line[line.trim_end_matches(['\r', '\n']).len()..];
                format!("schematic_id = \"<random>\"{ending}")
            }
            false => line.to_owned(),
        })
        .collect::<String>()
        .into_bytes()
}

fn run_flow(tag: &str, work: &Path) -> Outputs {
    let (mut app, boot) = Signex::boot_headless(Vec::new(), &rules()).expect("boot");
    settle(&mut app, boot);
    send(
        &mut app,
        Message::Ui(UiMsg::LanguageChanged(LanguagePref::Locale(tag.into()))),
    );
    // `tr-TR` has no catalog yet and renders en-US; the setting still holds,
    // and the test starts to bite the day a Turkish catalog lands.
    assert_eq!(
        app.ui_state.i18n.pref(),
        &LanguagePref::Locale(tag.into()),
        "the language switched"
    );
    // The locale in force, so a negotiation fallback cannot make the
    // en-XA run compare en-US with en-US.
    let in_force = if tag == "tr-TR" { "en-US" } else { tag };
    assert_eq!(app.ui_state.i18n.locale(), in_force, "the active locale");

    // Same absolute path in every run, so a path inside a file cannot differ.
    if work.exists() {
        std::fs::remove_dir_all(work).expect("clear the work folder");
    }
    let (project, sheet) = signex_app::sample_project::ensure_extracted(work).expect("fixture");
    add_dangling_wire(&sheet);
    send_answered(&mut app, &project, Message::Menu(MenuMessage::OpenProject));
    send(
        &mut app,
        Message::File(FileMsg::Opened(Some(sheet.clone()))),
    );
    assert_eq!(app.document_state.projects.len(), 1, "project opened");

    let run_erc = AppCommandId::new("run_erc").expect("id");
    let (outcome, task) = app.dispatch_command_headless(&run_erc, CommandArgs::none());
    settle(&mut app, task);
    assert_eq!(format!("{outcome:?}"), "Ran");

    // Add ERC Waiver: the first finding that can be waived.
    let (waived_sheet, key, message) = app
        .ui_state
        .erc_violations_by_path
        .iter()
        .find_map(|(path, found)| {
            found.iter().find_map(|v| {
                v.waiver_key
                    .clone()
                    .map(|key| (path.clone(), key, v.message.clone()))
            })
        })
        .expect("the fixture has a finding to waive");
    let panel = |msg| Message::Dock(DockMessage::Panel(PanelMsg::ErcWaiver(msg)));
    send(
        &mut app,
        panel(ErcWaiverMsg::Started {
            key,
            message,
            sheet: waived_sheet,
        }),
    );
    send(
        &mut app,
        panel(ErcWaiverMsg::ReasonChanged("by design".into())),
    );
    send(
        &mut app,
        panel(ErcWaiverMsg::AuthorChanged("Tester".into())),
    );
    send(&mut app, panel(ErcWaiverMsg::Confirmed));

    // New Sheet, then Save All writes the project file and the sheets.
    let project_id = app.document_state.projects[0].id;
    let extra = work.join("Extra.snxsch");
    send(
        &mut app,
        Message::Project(ProjectMsg::AddNewSchematicPicked {
            project_id,
            path: Some(extra.clone()),
        }),
    );
    send(&mut app, Message::Menu(MenuMessage::SaveAll));

    let mut out = Outputs::new();
    out.insert("project.snxprj (save + waiver)", pin_date(read(&project)));
    out.insert("sheet.snxsch", read(&sheet));
    out.insert("Extra.snxsch (new sheet)", mask_schematic_id(read(&extra)));

    for (name, file) in [
        ("erc report html", "erc.html"),
        ("erc report json", "erc.json"),
        ("erc report csv", "erc.csv"),
        ("erc report txt", "erc.txt"),
    ] {
        let path = work.join(file);
        send_answered(&mut app, &path, Message::Menu(MenuMessage::ExportErcReport));
        out.insert(name, read(&path));
    }

    let netlist = work.join("out.NET");
    send_answered(
        &mut app,
        &netlist,
        Message::Menu(MenuMessage::ExportNetlist),
    );
    out.insert("netlist", read(&netlist));

    send(&mut app, Message::Menu(MenuMessage::ExportBom));
    let bom = work.join("bom.csv");
    send_answered(&mut app, &bom, Message::BomPreview(BomPreviewMsg::Export));
    out.insert("bom", read(&bom));

    send(&mut app, Message::Menu(MenuMessage::ExportPdf));
    let pdf = work.join("sheet.pdf");
    send_answered(
        &mut app,
        &pdf,
        Message::PrintPreview(PrintPreviewMsg::Export),
    );
    out.insert("pdf", read(&pdf));
    out
}

#[test]
fn files_are_byte_identical_in_every_ui_language() {
    let isolated = tempfile::tempdir().expect("override root");
    set_config_root_override(isolated.path().to_path_buf()).expect("override");
    install_before_boot(FontSet::BundledOnly).expect("fonts");
    let work: PathBuf = isolated.path().join("work");

    let baseline = run_flow(LOCALES[0], &work);
    // The comparison must be over real content, not empty files.
    for (name, bytes) in &baseline {
        assert!(!bytes.is_empty(), "{name} is empty");
    }
    let has = |name: &str, needle: &str| String::from_utf8_lossy(&baseline[name]).contains(needle);
    assert!(
        has("project.snxprj (save + waiver)", "by design"),
        "waiver saved"
    );
    assert!(
        has("project.snxprj (save + waiver)", "Extra.snxsch"),
        "sheet listed"
    );
    assert!(has("erc report html", "angling"), "findings reported");
    assert!(baseline["pdf"].starts_with(b"%PDF"), "a PDF");
    for tag in &LOCALES[1..] {
        let other = run_flow(tag, &work);
        for (name, bytes) in &baseline {
            assert!(
                other[name] == *bytes,
                "{name} differs between {} and {tag}:\n--- {}\n--- {}",
                LOCALES[0],
                String::from_utf8_lossy(bytes),
                String::from_utf8_lossy(&other[name]),
            );
        }
    }
}
