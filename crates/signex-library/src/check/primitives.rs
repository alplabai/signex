//! The primitive files of a library: read, canonical form, naming, STEP
//! attachments, and the index references resolve against.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{Problem, first_difference};
use crate::primitive::{
    FootprintFile, FootprintFileError, SimFile, SimFileError, SymbolFile, SymbolFileError,
};
use crate::writer::{FOOTPRINTS_DIR, SIMS_DIR, STEP_DIR, SYMBOLS_DIR};

/// The primitives a reference into one library can reach.
#[derive(Debug, Default)]
pub struct LibraryIndex {
    pub library_id: Uuid,
    pub symbols: HashSet<Uuid>,
    /// Footprints reachable by file name (`footprints/<uuid>.snxfpt`).
    pub footprints: HashSet<Uuid>,
    /// Sim models reachable by file name (`sims/<uuid>.snxsim`).
    pub sims: HashSet<Uuid>,
}

impl LibraryIndex {
    /// Read every primitive file under `root`, reporting what is wrong with
    /// each into `out`.
    pub fn scan(root: &Path, library_id: Uuid, out: &mut Vec<Problem>) -> Self {
        let mut index = Self {
            library_id,
            ..Self::default()
        };
        let mut symbol_files: HashMap<Uuid, String> = HashMap::new();
        for (rel, bytes) in files(root, SYMBOLS_DIR, "snxsym", "symbol", out) {
            let Some(file) = parse(&rel, &bytes, "symbol", out, SymbolFile::from_bytes, |e| {
                matches!(e, SymbolFileError::UnsupportedFormat { .. })
            }) else {
                continue;
            };
            canonical(&rel, &bytes, "symbol", file.to_toml_string(), out);
            for sym in &file.symbols {
                if let Some(first) = symbol_files.insert(sym.uuid, rel.clone()) {
                    out.push(Problem::error(
                        "symbol.duplicate_uuid",
                        &rel,
                        Some(sym.uuid.to_string()),
                        format!(
                            "symbol {} is also in {first}; which one a part gets is undefined",
                            sym.uuid
                        ),
                    ));
                }
                index.symbols.insert(sym.uuid);
            }
        }
        for (rel, bytes) in files(root, FOOTPRINTS_DIR, "snxfpt", "footprint", out) {
            let Some(file) = parse(
                &rel,
                &bytes,
                "footprint",
                out,
                FootprintFile::from_bytes,
                |e| matches!(e, FootprintFileError::UnsupportedFormat { .. }),
            ) else {
                continue;
            };
            canonical(&rel, &bytes, "footprint", file.to_toml_string(), out);
            let uuids: Vec<Uuid> = file.footprints.iter().map(|f| f.uuid).collect();
            if let Some(uuid) = named_by(&rel, &uuids, "footprint", out) {
                index.footprints.insert(uuid);
            }
            for fp in &file.footprints {
                if let Some(step) = &fp.step_attachment {
                    check_step(root, &rel, fp.uuid, &step.content_hash, out);
                }
            }
        }
        for (rel, bytes) in files(root, SIMS_DIR, "snxsim", "sim", out) {
            let Some(file) = parse(&rel, &bytes, "sim", out, SimFile::from_bytes, |e| {
                matches!(e, SimFileError::UnsupportedFormat { .. })
            }) else {
                continue;
            };
            canonical(&rel, &bytes, "sim", file.to_toml_string(), out);
            let uuids: Vec<Uuid> = file.models.iter().map(|m| m.uuid).collect();
            if let Some(uuid) = named_by(&rel, &uuids, "sim", out) {
                index.sims.insert(uuid);
            }
        }
        index
    }
}

/// `(<sub>/<name>, bytes)` of every `*.<ext>` file directly in `root/sub`,
/// by name.
fn files(
    root: &Path,
    sub: &str,
    ext: &str,
    kind: &str,
    out: &mut Vec<Problem>,
) -> Vec<(String, Vec<u8>)> {
    let entries = match std::fs::read_dir(root.join(sub)) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            out.push(Problem::error(
                "library.unreadable",
                sub,
                None,
                e.to_string(),
            ));
            return Vec::new();
        }
    };
    let mut found: Vec<(String, Vec<u8>)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || path.extension().is_none_or(|e| e != ext) {
            continue;
        }
        let rel = format!("{sub}/{}", entry.file_name().to_string_lossy());
        match std::fs::read(&path) {
            Ok(bytes) => found.push((rel, bytes)),
            Err(e) => out.push(Problem::error(
                error_code(kind, "unreadable"),
                &rel,
                None,
                e.to_string(),
            )),
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

fn error_code(kind: &str, what: &str) -> &'static str {
    match (kind, what) {
        ("symbol", "unreadable") => "symbol.unreadable",
        ("footprint", "unreadable") => "footprint.unreadable",
        ("sim", "unreadable") => "sim.unreadable",
        ("symbol", "not_canonical") => "symbol.not_canonical",
        ("footprint", "not_canonical") => "footprint.not_canonical",
        ("sim", "not_canonical") => "sim.not_canonical",
        ("symbol", "legacy_token") => "symbol.legacy_token",
        ("footprint", "legacy_token") => "footprint.legacy_token",
        ("sim", "legacy_token") => "sim.legacy_token",
        ("footprint", "misnamed") => "footprint.misnamed",
        ("sim", "misnamed") => "sim.misnamed",
        ("footprint", "unreachable") => "footprint.unreachable",
        ("sim", "unreachable") => "sim.unreachable",
        _ => "library.unreadable",
    }
}

/// Parse with `read`, reporting a refused format token as
/// `format.unsupported` and any other failure as `<kind>.unreadable`.
fn parse<T, E: std::fmt::Display>(
    rel: &str,
    bytes: &[u8],
    kind: &str,
    out: &mut Vec<Problem>,
    read: impl Fn(&[u8]) -> Result<T, E>,
    is_token_refusal: impl Fn(&E) -> bool,
) -> Option<T> {
    match read(bytes) {
        Ok(file) => Some(file),
        Err(e) => {
            let code = if is_token_refusal(&e) {
                "format.unsupported"
            } else {
                error_code(kind, "unreadable")
            };
            out.push(Problem::error(code, rel, None, e.to_string()));
            None
        }
    }
}

/// A file must re-save as its own bytes. When only the `format` line
/// differs (a token this build reads but no longer writes, such as the stock
/// library's `snxfpt/1`), it is a warning: the content is canonical and the
/// next save only rewrites the token.
fn canonical<E: std::fmt::Display>(
    rel: &str,
    bytes: &[u8],
    kind: &str,
    resaved: Result<String, E>,
    out: &mut Vec<Problem>,
) {
    let text = String::from_utf8_lossy(bytes);
    let resaved = match resaved {
        Ok(resaved) if resaved == text => return,
        Ok(resaved) => resaved,
        Err(e) => {
            out.push(Problem::error(
                error_code(kind, "not_canonical"),
                rel,
                None,
                format!("the file does not re-save: {e}"),
            ));
            return;
        }
    };
    let without_token = |t: &str| -> String {
        t.lines()
            .filter(|l| !l.starts_with("format = "))
            .collect::<Vec<_>>()
            .join("\n")
    };
    if without_token(&text) == without_token(&resaved) {
        out.push(Problem::warning(
            error_code(kind, "legacy_token"),
            rel,
            None,
            format!(
                "format token is read but no longer written; a save rewrites it ({})",
                first_difference(&text, &resaved)
            ),
        ));
    } else {
        out.push(Problem::error(
            error_code(kind, "not_canonical"),
            rel,
            None,
            format!(
                "a save would rewrite the file ({})",
                first_difference(&text, &resaved)
            ),
        ));
    }
}

/// The uuid a one-item file is reached by: its stem, which must be the uuid
/// of its first item. Items after the first are unreachable (format open
/// point 1).
fn named_by(rel: &str, uuids: &[Uuid], kind: &str, out: &mut Vec<Problem>) -> Option<Uuid> {
    let stem = Path::new(rel).file_stem()?.to_string_lossy().into_owned();
    let Some(first) = uuids.first() else {
        out.push(Problem::error(
            error_code(kind, "unreadable"),
            rel,
            None,
            format!("the file holds no {kind}"),
        ));
        return None;
    };
    for extra in &uuids[1..] {
        out.push(Problem::error(
            error_code(kind, "unreachable"),
            rel,
            Some(extra.to_string()),
            format!("only the first {kind} of a file is reachable; write one {kind} per file"),
        ));
    }
    match Uuid::parse_str(&stem) {
        Ok(named) if named == *first && stem == first.to_string() => Some(named),
        _ => {
            out.push(Problem::error(
                error_code(kind, "misnamed"),
                rel,
                Some(first.to_string()),
                format!(
                    "the file must be named {first}.{} after the {kind} it holds",
                    ext_of(rel)
                ),
            ));
            None
        }
    }
}

fn ext_of(rel: &str) -> String {
    Path::new(rel)
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A footprint's STEP attachment must be a SHA-256 whose model is present.
fn check_step(root: &Path, rel: &str, footprint: Uuid, hash: &str, out: &mut Vec<Problem>) {
    let item = Some(footprint.to_string());
    if hash.len() != 64 || !hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        out.push(Problem::error(
            "model.bad_hash",
            rel,
            item,
            format!("step_attachment.content_hash {hash:?} is not a lower-case hex SHA-256"),
        ));
        return;
    }
    if embedded_model(rel, hash).is_some() {
        return;
    }
    let step_rel = format!("{STEP_DIR}/{hash}.step");
    match std::fs::read(root.join(&step_rel)) {
        Ok(bytes) => {
            let actual = format!("{:x}", Sha256::digest(&bytes));
            if actual != hash {
                out.push(Problem::error(
                    "model.hash_mismatch",
                    &step_rel,
                    item,
                    format!("the file's SHA-256 is {actual}, not the {hash} it is named and referenced by"),
                ));
            }
        }
        // The app shows the footprint without its model: a warning.
        Err(_) => out.push(Problem::warning(
            "model.missing",
            rel,
            item,
            format!("STEP model {step_rel} is not in the library"),
        )),
    }
}

/// Hook for embedded models (`[[models]]`, `snxfpt/1.4`, #3107): the model
/// with `hash` stored inside the footprint file `rel`, once the format has
/// them. Until then nothing is embedded and `step/` is the only store.
fn embedded_model(_rel: &str, _hash: &str) -> Option<()> {
    None
}
