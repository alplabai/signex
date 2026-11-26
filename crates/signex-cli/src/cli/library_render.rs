//! `signex-cli library render <library> [--out <dir>] [--symbol <uuid|name>]...
//! [--footprint <uuid|name>]... [--png]` (#3116): render a library's symbols
//! and footprints to SVG without a window, for comparing a converted library
//! with its source. The drawing is the editors' own: a symbol goes through the
//! schematic scene builder the Symbol Editor and Components preview use, a
//! footprint through the preview model of the Components panel. See
//! `docs/CLI.md`, "Library tools".

use std::io::Write;
use std::path::{Path, PathBuf};

use signex_app::library::primitive_scene::symbol_scene;
use signex_library::adapters::local_git::slugify;
use signex_library::primitive::{FootprintFile, SymbolFile};
use signex_renderer::scene_svg::{SceneSvgOptions, scene_to_svg};
use signex_types::theme::{ThemeId, canvas_colors};

mod raster;

/// The folder renders go to when `--out` is not given.
const DEFAULT_OUT: &str = "library-render";

#[derive(Debug, PartialEq, Eq)]
pub struct LibraryRenderOptions {
    pub library: PathBuf,
    pub out: PathBuf,
    /// Symbol uuids or names to render; with no `--symbol` and no
    /// `--footprint`, every primitive.
    pub symbols: Vec<String>,
    pub footprints: Vec<String>,
    pub png: bool,
}

/// Parse the words after `library render`.
pub fn parse(mut args: impl Iterator<Item = String>) -> Result<LibraryRenderOptions, String> {
    let mut library = None;
    let mut out = None;
    let (mut symbols, mut footprints, mut png) = (Vec::new(), Vec::new(), false);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--png" => png = true,
            "--out" => out = Some(PathBuf::from(args.next().ok_or("--out needs a folder")?)),
            "--symbol" => symbols.push(args.next().ok_or("--symbol needs a uuid or name")?),
            "--footprint" => {
                footprints.push(args.next().ok_or("--footprint needs a uuid or name")?)
            }
            flag if flag.starts_with("--") => return Err(format!("unknown option `{flag}`")),
            _ if library.is_none() => library = Some(PathBuf::from(arg)),
            _ => return Err(format!("unexpected argument `{arg}`")),
        }
    }
    Ok(LibraryRenderOptions {
        library: library.ok_or("`library render` needs a library folder or .snxlib")?,
        out: out.unwrap_or_else(|| PathBuf::from(DEFAULT_OUT)),
        symbols,
        footprints,
        png,
    })
}

/// `<kind>-<slug>-<uuid8>`.
fn file_stem(kind: &str, name: &str, uuid: uuid::Uuid) -> String {
    format!(
        "{kind}-{}-{}",
        slugify(name),
        &uuid.simple().to_string()[..8]
    )
}

/// Whether a selector names the primitive; every selector that does is
/// marked in `hit`.
fn wanted(selectors: &[String], hit: &mut [bool], all: bool, name: &str, uuid: uuid::Uuid) -> bool {
    let mut chosen = all;
    for (i, s) in selectors.iter().enumerate() {
        if s.eq_ignore_ascii_case(name) || s.parse::<uuid::Uuid>().is_ok_and(|u| u == uuid) {
            hit[i] = true;
            chosen = true;
        }
    }
    chosen
}

fn files(root: &Path, sub: &str, ext: &str) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(root.join(sub))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == ext))
        .collect();
    found.sort();
    found
}

/// Render and write the selected primitives. `Ok(false)` when a selector
/// matched nothing or a file did not read.
pub fn run(options: &LibraryRenderOptions, out: &mut impl Write) -> Result<bool, String> {
    let root = if options.library.is_file() {
        options
            .library
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf()
    } else {
        options.library.clone()
    };
    std::fs::create_dir_all(&options.out)
        .map_err(|e| format!("creating {}: {e}", options.out.display()))?;
    let everything = options.symbols.is_empty() && options.footprints.is_empty();
    let colors = canvas_colors(ThemeId::GraphiteLight);
    let paper = rgba(&colors.paper);
    let board = rgba(&signex_app::styles::board_canvas_colors().background);
    let mut ok = true;
    let mut symbol_hit = vec![false; options.symbols.len()];
    let mut footprint_hit = vec![false; options.footprints.len()];
    let emit = |stem: String, svg: Option<String>, out: &mut dyn Write| -> Result<(), String> {
        let Some(svg) = svg else {
            let _ = writeln!(out, "skipped {stem}: draws nothing");
            return Ok(());
        };
        let path = options.out.join(format!("{stem}.svg"));
        std::fs::write(&path, &svg).map_err(|e| format!("writing {}: {e}", path.display()))?;
        let _ = writeln!(out, "{}", path.display());
        if options.png {
            let png = options.out.join(format!("{stem}.png"));
            raster::write_png(&svg, &png)?;
            let _ = writeln!(out, "{}", png.display());
        }
        Ok(())
    };

    for path in files(&root, "symbols", "snxsym") {
        let bytes = std::fs::read(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;
        let Ok(file) = SymbolFile::from_bytes(&bytes) else {
            let _ = writeln!(out, "unreadable {}", path.display());
            ok = false;
            continue;
        };
        for symbol in &file.symbols {
            if !wanted(
                &options.symbols,
                &mut symbol_hit,
                everything,
                &symbol.name,
                symbol.uuid,
            ) {
                continue;
            }
            let lib = symbol.to_lib_symbol(symbol.uuid.to_string());
            let svg = symbol_scene(&lib, &colors).and_then(|scene| {
                scene_to_svg(
                    &scene,
                    &SceneSvgOptions {
                        background: paper,
                        ..Default::default()
                    },
                )
            });
            emit(file_stem("symbol", &symbol.name, symbol.uuid), svg, out)?;
        }
    }
    for path in files(&root, "footprints", "snxfpt") {
        let bytes = std::fs::read(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;
        let Ok(file) = FootprintFile::from_bytes(&bytes) else {
            let _ = writeln!(out, "unreadable {}", path.display());
            ok = false;
            continue;
        };
        for fp in &file.footprints {
            if !wanted(
                &options.footprints,
                &mut footprint_hit,
                everything,
                &fp.name,
                fp.uuid,
            ) {
                continue;
            }
            let scene = signex_app::library::editor::footprint::PreviewFootprint::build(fp).scene();
            let svg = scene_to_svg(
                &scene,
                &SceneSvgOptions {
                    background: board,
                    ..Default::default()
                },
            );
            emit(file_stem("footprint", &fp.name, fp.uuid), svg, out)?;
        }
    }
    let missing = options
        .symbols
        .iter()
        .zip(&symbol_hit)
        .chain(options.footprints.iter().zip(&footprint_hit))
        .filter(|(_, hit)| !**hit);
    for (selector, _) in missing {
        let _ = writeln!(out, "no primitive matches `{selector}`");
        ok = false;
    }
    Ok(ok)
}

fn rgba(c: &signex_types::theme::Color) -> [f32; 4] {
    [c.r, c.g, c.b, c.a].map(|v| f32::from(v) / 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(args: &[&str]) -> Result<LibraryRenderOptions, String> {
        parse(args.iter().map(ToString::to_string))
    }

    #[test]
    fn parses_selectors_out_and_png() {
        let o = parsed(&[
            "lib",
            "--out",
            "o",
            "--symbol",
            "R",
            "--symbol",
            "C",
            "--footprint",
            "F",
            "--png",
        ])
        .unwrap();
        assert_eq!(o.library, PathBuf::from("lib"));
        assert_eq!(o.out, PathBuf::from("o"));
        assert_eq!(o.symbols, ["R", "C"]);
        assert_eq!(o.footprints, ["F"]);
        assert!(o.png);
        assert_eq!(parsed(&["lib"]).unwrap().out, PathBuf::from(DEFAULT_OUT));
    }

    #[test]
    fn rejects_what_it_cannot_read() {
        for (args, wanted) in [
            (&[][..], "needs a library"),
            (&["lib", "--symbol"], "--symbol needs"),
            (&["lib", "--x"], "unknown option `--x`"),
            (&["lib", "b"], "unexpected argument `b`"),
        ] {
            assert!(parsed(args).unwrap_err().contains(wanted), "{args:?}");
        }
    }

    #[test]
    fn a_name_is_matched_without_case_and_a_uuid_exactly() {
        let u = uuid::Uuid::from_u128(7);
        let mut hit = [false, false];
        assert!(wanted(
            &["r".into(), u.to_string()],
            &mut hit,
            false,
            "R",
            u
        ));
        assert_eq!(hit, [true, true]);
        assert!(!wanted(&["x".into()], &mut [false], false, "R", u));
        assert_eq!(
            file_stem("symbol", "C Polarized", u),
            "symbol-c-polarized-00000000"
        );
    }
}
