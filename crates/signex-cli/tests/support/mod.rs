//! Shared by the end-to-end tests: a fixture project on disk and a way to run
//! the real `signex-cli` binary on it.

#![allow(
    clippy::disallowed_methods,
    reason = "the tests run the signex-cli binary as a child process"
)]
#![allow(
    dead_code,
    reason = "each test binary uses a different part of this shared module"
)]

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;
use signex_types::net_class::NetClass;
use signex_types::project::{
    LibraryEntry, LibraryEntryKind, ProjectData, SheetEntry, write_project,
};
use signex_types::schematic::{
    Label, LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet,
    Symbol, Wire,
};

pub struct Fixture {
    pub dir: PathBuf,
    pub project: PathBuf,
    pub sheet: PathBuf,
}

/// A project with one sheet: a wire from (0,0) to (10,0), a resistor pin and
/// a global label GND at the origin (so the net GND exists), and a net class
/// `Power`. `root` must be a canonical folder that exists.
pub fn fixture(root: &Path) -> Fixture {
    let dir = root.join("proj");
    std::fs::create_dir_all(&dir).expect("project dir");
    let (project, sheet) = (dir.join("demo.snxprj"), dir.join("main.snxsch"));
    let origin = Point::new(0.0, 0.0);
    let wire = Wire {
        uuid: uuid::Uuid::now_v7(),
        start: origin,
        end: Point::new(10.0, 0.0),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
        locked: false,
    };
    let lib = LibSymbol {
        id: "Device:R".into(),
        reference: "R".into(),
        value: String::new(),
        footprint: String::new(),
        datasheet: String::new(),
        description: String::new(),
        keywords: String::new(),
        fp_filters: String::new(),
        in_bom: true,
        on_board: true,
        in_pos_files: true,
        duplicate_pin_numbers_are_jumpers: false,
        graphics: Vec::new(),
        pins: vec![LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: origin,
                rotation: 0.0,
                length: 0.0,
                name: String::new(),
                number: "1".into(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        }],
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    };
    let content = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        wires: vec![wire],
        labels: vec![Label::new("GND", origin, LabelType::Global)],
        symbols: vec![Symbol {
            uuid: uuid::Uuid::now_v7(),
            lib_id: "Device:R".into(),
            reference: "R1".into(),
            position: origin,
            ..Default::default()
        }],
        lib_symbols: [("Device:R".to_string(), lib)].into_iter().collect(),
        ..Default::default()
    };
    signex_engine::Engine::new_with_path(content, Some(sheet.clone()))
        .expect("engine")
        .save()
        .expect("seed sheet");
    write_project(&project, &project_data(&dir, Vec::new())).expect("seed project");
    Fixture {
        dir,
        project,
        sheet,
    }
}

fn project_data(dir: &Path, libraries: Vec<LibraryEntry>) -> ProjectData {
    ProjectData {
        document_id: None,
        description: None,
        name: "demo".into(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some("main.snxsch".into()),
        pcb_file: None,
        sheets: vec![SheetEntry {
            name: "Main".into(),
            filename: "main.snxsch".into(),
            symbols_count: 0,
            wires_count: 1,
            labels_count: 1,
            unknown: Default::default(),
        }],
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries,
        enable_git: false,
        net_classes: vec![NetClass::named("Power")],
        rules: None,
        annotation: None,
        schematic_defaults: Default::default(),
        parameters: Default::default(),
        unknown: Default::default(),
    }
}

/// Rewrite the fixture's project file with git versioning enabled.
pub fn enable_git(fx: &Fixture) {
    let data = ProjectData {
        document_id: None,
        enable_git: true,
        ..project_data(&fx.dir, Vec::new())
    };
    write_project(&fx.project, &data).expect("rewrite project");
}

/// Rewrite the fixture's project file listing `paths` as shared libraries.
pub fn list_libraries(fx: &Fixture, paths: &[&Path]) {
    let libraries = paths
        .iter()
        .map(|path| LibraryEntry {
            path: path.to_path_buf(),
            kind: LibraryEntryKind::Shared,
            library_id: None,
            unknown: Default::default(),
        })
        .collect();
    write_project(&fx.project, &project_data(&fx.dir, libraries)).expect("rewrite project");
}

/// A running `signex-cli run`: send a line, read its result line, repeat.
pub struct Live {
    child: std::process::Child,
    lines: std::io::BufReader<std::process::ChildStdout>,
    /// The agent registry the run holds its project in, when `start` made
    /// one: not the user's.
    _runtime: Option<tempfile::TempDir>,
}

impl Live {
    pub fn start(args: &[&str]) -> Self {
        let runtime = tempfile::tempdir().expect("runtime dir");
        let mut live = Self::start_in(runtime.path(), args);
        live._runtime = Some(runtime);
        live
    }

    /// [`Live::start`] with the agent registry under `runtime`.
    pub fn start_in(runtime: &Path, args: &[&str]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_signex-cli"))
            .args(args)
            .env("XDG_RUNTIME_DIR", runtime)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn signex-cli");
        let lines = std::io::BufReader::new(child.stdout.take().expect("stdout"));
        Self {
            child,
            lines,
            _runtime: None,
        }
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Send one command line and return its result line.
    pub fn send(&mut self, command: &Value) -> Value {
        use std::io::BufRead;
        let stdin = self.child.stdin.as_mut().expect("stdin");
        writeln!(stdin, "{command}").expect("write command");
        stdin.flush().expect("flush");
        let mut line = String::new();
        self.lines.read_line(&mut line).expect("read result");
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("result line `{line}`: {e}"))
    }

    pub fn finish(mut self) {
        drop(self.child.stdin.take());
        self.child.wait().expect("signex-cli ends");
    }
}

/// Every file under `dir` with its bytes.
pub fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(folder) = stack.pop() {
        for entry in std::fs::read_dir(&folder).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.insert(path.clone(), std::fs::read(&path).expect("read"));
            }
        }
    }
    out
}

pub struct Output {
    pub code: Option<i32>,
    pub lines: Vec<Value>,
    pub stdout: String,
    pub stderr: String,
}

/// Run `signex-cli <args>` with `stdin` and parse every output line as JSON.
pub fn run_cli(args: &[&str], stdin: impl AsRef<[u8]>) -> Output {
    run_cli_env(args, stdin, &[])
}

/// [`run_cli`] with environment variables set for the child. Unless `env`
/// names `XDG_RUNTIME_DIR`, the child's agent registry is a folder of its own.
pub fn run_cli_env(args: &[&str], stdin: impl AsRef<[u8]>, env: &[(&str, &Path)]) -> Output {
    let runtime = tempfile::tempdir().expect("runtime dir");
    let mut child = Command::new(env!("CARGO_BIN_EXE_signex-cli"))
        .args(args)
        .env("XDG_RUNTIME_DIR", runtime.path())
        .envs(env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn signex-cli");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(stdin.as_ref())
        .expect("write stdin");
    let output = child.wait_with_output().expect("signex-cli runs");
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    let lines = stdout
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    Output {
        code: output.status.code(),
        lines,
        stdout,
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// `{"id": .., "args": ..}` lines, one per command.
pub fn jsonl(commands: &[Value]) -> String {
    commands.iter().map(|c| format!("{c}\n")).collect()
}

/// A canonical temp folder (macOS temp dirs are symlinks).
pub fn temp_root() -> (tempfile::TempDir, PathBuf) {
    let guard = tempfile::tempdir().expect("temp dir");
    let root = signex_cli::confine::canonical(guard.path()).expect("canonical temp dir");
    (guard, root)
}
