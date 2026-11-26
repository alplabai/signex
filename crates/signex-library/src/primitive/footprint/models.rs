//! STEP models embedded in the footprint file (`[[models]]`, #3107), so one
//! `.snxfpt` carries its footprints' 3D bodies. One entry per distinct hash,
//! sorted by hash, written last and only for hashes a footprint references.
//! The bytes are inflated only when asked for ([`FootprintFile::step_model`]),
//! never on load. An entry read from a file keeps its `data` text as stored,
//! so a re-save never re-compresses it; only new entries are compressed
//! here. Format and rules: `docs/FORMAT_VERSIONS.md`, `snxfpt/1.4`.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{Footprint, FootprintFile, FootprintFileError, StepAttachment};

/// The one `encoding` value: raw deflate (RFC 1951), then base64.
pub const MODEL_ENCODING: &str = "deflate+base64";

/// Deflate level of a newly embedded model. Fixed, with flate2's default
/// backend, so the same STEP bytes embed as the same text. The compressed
/// bytes are not part of the format contract: conformance compares the
/// inflated bytes (their SHA-256), and an entry read from a file keeps its
/// data as stored, whatever level and line width wrote it.
pub const MODEL_DEFLATE_LEVEL: u32 = 6;

/// Width of the wrapped base64 lines in `data`.
const MODEL_DATA_WIDTH: usize = 76;

/// Largest raw model a file may declare or inflate to.
const MAX_MODEL_BYTES: u64 = 256 * 1024 * 1024;

/// Stands in for each `data` value until the literal block is spliced in.
const MODEL_DATA_PLACEHOLDER_PREFIX: &str = "__SIGNEX_MODEL_DATA_a1b2c3d4_";

/// One embedded model. `data` is the literal block's text exactly as read
/// (or as wrapped when embedded here), written back unchanged; it is
/// decoded only on demand.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddedModel {
    /// Original file name, a hint for the UI.
    pub filename: String,
    /// Raw STEP byte count, checked on inflate.
    pub size: u64,
    data: Arc<str>,
}

/// The `[[models]]` section, keyed by SHA-256 hex of the raw STEP bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EmbeddedModels(BTreeMap<String, EmbeddedModel>);

/// An embedded model that does not decode to what its entry declares.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("footprint {footprint:?}: embedded 3D model {hash}: {fault}")]
pub struct ModelError {
    pub footprint: String,
    pub hash: String,
    pub fault: ModelFault,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ModelFault {
    #[error("its data is not valid base64 ({0})")]
    Base64(String),
    #[error("its data does not inflate ({0})")]
    Inflate(String),
    #[error("it inflates to {actual} bytes, but its size says {declared}")]
    Size { declared: u64, actual: u64 },
    #[error("its bytes hash to {actual}, not to its own hash")]
    Hash { actual: String },
}

/// Lowercase SHA-256 hex of `bytes`: a STEP attachment's `content_hash`.
pub fn step_model_hash(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push(HEX[usize::from(byte >> 4)] as char);
        out.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    out
}

fn is_model_hash(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

impl EmbeddedModel {
    /// Compress `step` into an entry.
    fn encode(filename: &str, step: &[u8]) -> Self {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::new(MODEL_DEFLATE_LEVEL));
        // Writing into a Vec cannot fail; an error here would be a flate2 bug.
        let deflated = encoder
            .write_all(step)
            .and_then(|()| encoder.finish())
            .unwrap_or_else(|e| unreachable!("in-memory deflate failed: {e}"));
        Self {
            filename: filename.to_string(),
            size: step.len() as u64,
            data: Arc::from(wrap(&BASE64.encode(deflated))),
        }
    }

    /// The raw STEP bytes, checked against `size` and `hash`.
    fn decode(&self, hash: &str) -> Result<Vec<u8>, ModelFault> {
        let text: Vec<u8> = self
            .data
            .bytes()
            .filter(|b| !b.is_ascii_whitespace())
            .collect();
        let deflated = BASE64
            .decode(&text)
            .map_err(|e| ModelFault::Base64(e.to_string()))?;
        let mut raw = Vec::new();
        // One byte past the declared size is enough to tell it is wrong;
        // `size` is already within MAX_MODEL_BYTES.
        DeflateDecoder::new(deflated.as_slice())
            .take(self.size + 1)
            .read_to_end(&mut raw)
            .map_err(|e| ModelFault::Inflate(e.to_string()))?;
        if raw.len() as u64 != self.size {
            return Err(ModelFault::Size {
                declared: self.size,
                actual: raw.len() as u64,
            });
        }
        let actual = step_model_hash(&raw);
        if actual != hash {
            return Err(ModelFault::Hash { actual });
        }
        Ok(raw)
    }
}

/// `base64` wrapped at [`MODEL_DATA_WIDTH`], each line ended.
fn wrap(base64: &str) -> String {
    let data = base64.as_bytes();
    let mut out = String::with_capacity(data.len() + data.len() / MODEL_DATA_WIDTH + 1);
    for line in data.chunks(MODEL_DATA_WIDTH) {
        // base64 text is ASCII, so every chunk is valid UTF-8.
        out.push_str(std::str::from_utf8(line).unwrap_or_default());
        out.push('\n');
    }
    out
}

/// True when `data` holds only base64 characters, spaces, tabs and line
/// ends (`\n`, `\r\n`): what a TOML `'''` literal can hold, so the block is
/// always written back loadable. A form feed or a lone CR is refused.
fn is_data_text(data: &str) -> bool {
    let bytes = data.as_bytes();
    bytes.iter().enumerate().all(|(i, &b)| match b {
        b'\r' => bytes.get(i + 1) == Some(&b'\n'),
        b' ' | b'\t' | b'\n' | b'+' | b'/' | b'=' => true,
        _ => b.is_ascii_alphanumeric(),
    })
}

/// On-disk shape of one `[[models]]` entry, keys in written order.
#[derive(Serialize, Deserialize)]
pub(super) struct ModelWire {
    hash: String,
    filename: String,
    encoding: String,
    size: u64,
    data: String,
}

/// The wrapped `data` blocks of the entries a save writes, in entry order,
/// to splice in after the TOML is emitted.
pub(super) struct ModelBlocks {
    payloads: Vec<String>,
}

fn placeholder(idx: usize) -> String {
    format!("{MODEL_DATA_PLACEHOLDER_PREFIX}{idx}__")
}

impl ModelBlocks {
    /// `out` with each `data = "<placeholder>"` value replaced by its `'''`
    /// literal block. Only a whole `data = ...` line matches, and each
    /// placeholder must match exactly once: anything else (a string that
    /// spells one) refuses the save rather than write a wrong file.
    pub fn fill(&self, out: String) -> Result<String, FootprintFileError> {
        let mut spans = Vec::with_capacity(self.payloads.len());
        for index in 0..self.payloads.len() {
            let value = format!("\"{}\"", placeholder(index));
            let line = format!("\ndata = {value}\n");
            let found: Vec<usize> = out.match_indices(&line).map(|(at, _)| at).collect();
            let [at] = found[..] else {
                return Err(FootprintFileError::ModelDataPlacement { index });
            };
            let start = at + "\ndata = ".len();
            spans.push((start, start + value.len(), index));
        }
        spans.sort_unstable();
        let extra: usize = self.payloads.iter().map(|p| p.len() + 7).sum();
        let mut filled = String::with_capacity(out.len() + extra);
        let mut from = 0;
        for (start, stop, index) in spans {
            filled.push_str(&out[from..start]);
            filled.push_str("'''\n");
            filled.push_str(&self.payloads[index]);
            filled.push_str("'''");
            from = stop;
        }
        filled.push_str(&out[from..]);
        Ok(filled)
    }
}

/// `footprint "A"`, `footprints "A", "B"`, or `no footprint`: who uses `hash`.
fn users_of(footprints: &[Footprint], hash: &str) -> String {
    let names: Vec<String> = footprints
        .iter()
        .filter(|f| f.step_attachment.as_ref().map(|a| a.content_hash.as_str()) == Some(hash))
        .map(|f| format!("{:?}", f.name))
        .collect();
    match names.len() {
        0 => "no footprint".to_string(),
        1 => format!("footprint {}", names[0]),
        _ => format!("footprints {}", names.join(", ")),
    }
}

impl EmbeddedModels {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Every entry, by hash.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &EmbeddedModel)> {
        self.0.iter().map(|(hash, model)| (hash.as_str(), model))
    }

    /// The entry stored under `hash`.
    pub fn get(&self, hash: &str) -> Option<&EmbeddedModel> {
        self.0.get(hash)
    }

    /// Embed `step`, returning its hash. An entry already under that hash
    /// is kept: the same hash is the same bytes.
    pub fn insert_step(&mut self, filename: &str, step: &[u8]) -> String {
        let hash = step_model_hash(step);
        self.0
            .entry(hash.clone())
            .or_insert_with(|| EmbeddedModel::encode(filename, step));
        hash
    }

    /// Add `other`'s entries this one lacks (a container merge).
    pub fn absorb(&mut self, other: &EmbeddedModels) {
        for (hash, model) in &other.0 {
            self.0.entry(hash.clone()).or_insert_with(|| model.clone());
        }
    }

    /// Read the section. Its shape is checked here; the data is checked
    /// when a model is decoded.
    pub(super) fn from_wire(
        wire: Vec<ModelWire>,
        footprints: &[Footprint],
    ) -> Result<Self, FootprintFileError> {
        let mut models = BTreeMap::new();
        for entry in wire {
            let refuse = |reason: String| FootprintFileError::InvalidModelEntry {
                hash: entry.hash.clone(),
                users: users_of(footprints, &entry.hash),
                reason,
            };
            if !is_model_hash(&entry.hash) {
                return Err(refuse(
                    "its hash is not a lowercase SHA-256 hex digest".into(),
                ));
            }
            if entry.encoding != MODEL_ENCODING {
                return Err(refuse(format!(
                    "unknown encoding {:?} (this build reads {MODEL_ENCODING:?})",
                    entry.encoding
                )));
            }
            if entry.size > MAX_MODEL_BYTES {
                return Err(refuse(format!(
                    "its size {} is over the {MAX_MODEL_BYTES}-byte limit",
                    entry.size
                )));
            }
            if models.contains_key(&entry.hash) {
                return Err(refuse("the file holds it twice".into()));
            }
            if !is_data_text(&entry.data) {
                return Err(refuse(
                    "its data holds characters base64 does not use".into(),
                ));
            }
            let data = entry.data;
            models.insert(
                entry.hash,
                EmbeddedModel {
                    filename: entry.filename,
                    size: entry.size,
                    data: Arc::from(data),
                },
            );
        }
        Ok(Self(models))
    }

    /// The entries `referenced` names, in hash order, for a save.
    pub(super) fn to_wire(&self, referenced: &BTreeSet<&str>) -> (Vec<ModelWire>, ModelBlocks) {
        let mut wire = Vec::new();
        let mut payloads = Vec::new();
        for (hash, model) in self
            .0
            .iter()
            .filter(|(h, _)| referenced.contains(h.as_str()))
        {
            wire.push(ModelWire {
                hash: hash.clone(),
                filename: model.filename.clone(),
                encoding: MODEL_ENCODING.to_string(),
                size: model.size,
                data: placeholder(wire.len()),
            });
            payloads.push(model.data.to_string());
        }
        (wire, ModelBlocks { payloads })
    }
}

impl FootprintFile {
    /// The model hashes this file's footprints attach.
    pub(super) fn referenced_model_hashes(&self) -> BTreeSet<&str> {
        self.footprints
            .iter()
            .filter_map(|f| f.step_attachment.as_ref())
            .map(|a| a.content_hash.as_str())
            .collect()
    }

    /// True when a save writes a `[[models]]` entry.
    pub(super) fn writes_models(&self) -> bool {
        self.referenced_model_hashes()
            .iter()
            .any(|hash| self.models.get(hash).is_some())
    }

    /// The STEP bytes `footprint` attaches, from this file. `Ok(None)` when
    /// it attaches none or this file does not embed it (a library written
    /// before #3107 keeps it in `step/<hash>.step`).
    pub fn step_model(&self, footprint: &Footprint) -> Result<Option<Vec<u8>>, ModelError> {
        let Some(attachment) = footprint.step_attachment.as_ref() else {
            return Ok(None);
        };
        let hash = attachment.content_hash.as_str();
        let Some(model) = self.models.get(hash) else {
            return Ok(None);
        };
        model.decode(hash).map(Some).map_err(|fault| ModelError {
            footprint: footprint.name.clone(),
            hash: hash.to_string(),
            fault,
        })
    }

    /// Decode every model a footprint uses; the first that fails is named.
    pub fn verify_models(&self) -> Result<(), ModelError> {
        for footprint in &self.footprints {
            self.step_model(footprint)?;
        }
        Ok(())
    }

    /// Embed `step` and attach it to the footprint `uuid`, at no offset or
    /// turn. `None`, with nothing embedded, when the file has no such
    /// footprint.
    pub fn attach_step_model(
        &mut self,
        uuid: Uuid,
        filename: &str,
        step: &[u8],
    ) -> Option<StepAttachment> {
        let index = self.footprints.iter().position(|f| f.uuid == uuid)?;
        let attachment = StepAttachment {
            content_hash: self.models.insert_step(filename, step),
            filename: filename.to_string(),
            offset_xyz: [0.0; 3],
            rotation_xyz: [0.0; 3],
        };
        self.footprints[index].step_attachment = Some(attachment.clone());
        Some(attachment)
    }

    /// Embed each attached model this file lacks from `step_dir/<hash>.step`,
    /// the store of libraries written before #3107. The files are only
    /// read, never removed. A missing file is skipped; one whose bytes do
    /// not match its hash is skipped and logged. Returns how many embedded.
    pub fn embed_models_from_step_dir(&mut self, step_dir: &Path) -> usize {
        let wanted: Vec<(String, String)> = self
            .footprints
            .iter()
            .filter_map(|f| f.step_attachment.as_ref())
            .filter(|a| {
                is_model_hash(&a.content_hash) && self.models.get(&a.content_hash).is_none()
            })
            .map(|a| (a.content_hash.clone(), a.filename.clone()))
            .collect();
        let mut embedded = 0;
        for (hash, filename) in wanted {
            if self.models.get(&hash).is_some() {
                continue;
            }
            let path = step_dir.join(format!("{hash}.step"));
            let bytes = match read_step_file(&path) {
                Ok(Some(bytes)) => bytes,
                Ok(None) => continue,
                Err(error) => {
                    tracing::warn!(
                        target: "signex::library",
                        path = %path.display(),
                        error = %error,
                        "STEP model not embedded; the footprint keeps reading it from step/"
                    );
                    continue;
                }
            };
            if step_model_hash(&bytes) != hash {
                tracing::warn!(
                    target: "signex::library",
                    path = %path.display(),
                    "STEP file does not match its hash; not embedded"
                );
                continue;
            }
            self.models.insert_step(&filename, &bytes);
            embedded += 1;
        }
        embedded
    }
}

/// The bytes at `path`, `None` when there is no file, refused over the cap.
fn read_step_file(path: &Path) -> std::io::Result<Option<Vec<u8>>> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let meta = file.metadata()?;
    if !meta.is_file() || meta.len() > MAX_MODEL_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "not a regular file within the model size limit",
        ));
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    file.take(MAX_MODEL_BYTES + 1).read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}
