use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use lib_gerber_edit::layer::{LayerData, LayerType};

use crate::GerberGeometry;

/// A successfully parsed fabrication layer and its render-ready geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedLayer {
    pub source_path: Option<PathBuf>,
    original_source: Option<String>,
    pub job_context: Option<crate::GerberJobContext>,
    pub name: String,
    pub layer_type: LayerType,
    pub data: LayerData,
    pub geometry: GerberGeometry,
}

/// An actionable per-file loading failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GerberLoadFailure {
    pub path: PathBuf,
    pub message: String,
}

impl std::fmt::Display for GerberLoadFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.path.display(), self.message)
    }
}

impl LoadedLayer {
    /// Returns the exact text supplied to the Gerber parser.
    pub fn gerber_source(&self) -> Result<&str, &'static str> {
        if !matches!(self.data, LayerData::Gerber(_)) {
            return Err("Source view is available only for Gerber layers.");
        }
        self.original_source
            .as_deref()
            .ok_or("The original Gerber source is unavailable.")
    }
}

/// Partial-success result for a multi-file load.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GerberLoadBatch {
    pub layers: Vec<LoadedLayer>,
    pub failures: Vec<GerberLoadFailure>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GerberReloadBatch {
    pub layers: Vec<(usize, LoadedLayer)>,
    pub failures: Vec<GerberLoadFailure>,
}

/// Reparses disk-backed layers while preserving the caller's layer indices.
pub fn reload_layers<I>(layers: I) -> GerberReloadBatch
where
    I: IntoIterator<Item = LoadedLayer>,
{
    let mut batch = GerberReloadBatch::default();
    for (index, layer) in layers.into_iter().enumerate() {
        let Some(path) = layer.source_path.clone() else {
            batch.failures.push(GerberLoadFailure {
                path: PathBuf::from(&layer.name),
                message: "layer has no disk source to reload".to_owned(),
            });
            continue;
        };
        let job_context = layer.job_context.clone();
        let result = match layer.data {
            LayerData::Gerber(_) => load_gerber_file(&path),
            LayerData::Excellon(_) => load_excellon_file(&path),
            LayerData::Info(_) => Err(GerberLoadFailure {
                path: path.clone(),
                message: "information layers cannot be reloaded as fabrication data".to_owned(),
            }),
        };
        match result {
            Ok(mut layer) => {
                layer.job_context = job_context;
                batch.layers.push((index, layer));
            }
            Err(failure) => batch.failures.push(failure),
        }
    }
    batch
}

/// Loads one RS-274X file from disk.
pub fn load_gerber_file(path: impl AsRef<Path>) -> Result<LoadedLayer, GerberLoadFailure> {
    let path = path.as_ref();
    let file = File::open(path).map_err(|error| GerberLoadFailure {
        path: path.to_path_buf(),
        message: format!("could not open file: {error}"),
    })?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let mut layer = load_gerber_reader(name, file).map_err(|mut failure| {
        failure.path = path.to_path_buf();
        failure
    })?;
    layer.source_path = Some(path.to_path_buf());
    Ok(layer)
}

/// Loads several RS-274X files, preserving successful layers when another file fails.
pub fn load_gerber_files<I, P>(paths: I) -> GerberLoadBatch
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let mut batch = GerberLoadBatch::default();
    for path in paths {
        match load_gerber_file(path.as_ref()) {
            Ok(layer) => batch.layers.push(layer),
            Err(failure) => batch.failures.push(failure),
        }
    }
    batch
}

/// Loads one fabrication file after detecting Gerber or Excellon from content
/// signatures, falling back to a documented filename extension.
pub fn load_autodetected_file(path: impl AsRef<Path>) -> Result<LoadedLayer, GerberLoadFailure> {
    let path = path.as_ref();
    let file = File::open(path).map_err(|error| GerberLoadFailure {
        path: path.to_path_buf(),
        message: format!("could not open file: {error}"),
    })?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let mut layer = load_autodetected_reader(name, file).map_err(|mut failure| {
        failure.path = path.to_path_buf();
        failure
    })?;
    layer.source_path = Some(path.to_path_buf());
    Ok(layer)
}

pub fn load_autodetected_files<I, P>(paths: I) -> GerberLoadBatch
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let mut batch = GerberLoadBatch::default();
    for path in paths {
        match load_autodetected_file(path.as_ref()) {
            Ok(layer) => batch.layers.push(layer),
            Err(failure) => batch.failures.push(failure),
        }
    }
    batch
}

/// Loads supported fabrication sources through one entry point.
///
/// ZIP archives are detected from their binary signature and Gerber jobs from
/// their JSON structure. Gerber and Excellon text files continue to use their
/// content signatures. Standard extensions are used only to reject a
/// mismatched container before parsing it as another format.
pub fn load_fabrication_files<I, P>(paths: I) -> GerberLoadBatch
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let mut batch = GerberLoadBatch::default();
    for path in paths {
        let path = path.as_ref();
        let mut loaded = match detect_fabrication_container(path) {
            Ok(Some(FabricationContainerFormat::Zip)) => crate::load_zip_archive(path),
            Ok(Some(FabricationContainerFormat::GerberJob)) => crate::load_gerber_job_file(path),
            Ok(None) => match load_autodetected_file(path) {
                Ok(layer) => single_layer(layer),
                Err(failure) => single_load_failure(failure),
            },
            Err(failure) => single_load_failure(failure),
        };
        batch.layers.append(&mut loaded.layers);
        batch.failures.append(&mut loaded.failures);
    }
    batch
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FabricationContainerFormat {
    Zip,
    GerberJob,
}

fn detect_fabrication_container(
    path: &Path,
) -> Result<Option<FabricationContainerFormat>, GerberLoadFailure> {
    let mut file = File::open(path).map_err(|error| GerberLoadFailure {
        path: path.to_path_buf(),
        message: format!("could not open file: {error}"),
    })?;
    let mut prefix = [0_u8; 4096];
    let prefix_length = file.read(&mut prefix).map_err(|error| GerberLoadFailure {
        path: path.to_path_buf(),
        message: format!("could not inspect file type: {error}"),
    })?;
    let prefix = &prefix[..prefix_length];
    let is_zip = prefix.get(..4).is_some_and(|signature| {
        matches!(
            signature,
            [b'P', b'K', 3, 4] | [b'P', b'K', 5, 6] | [b'P', b'K', 7, 8]
        )
    });
    if is_zip {
        return Ok(Some(FabricationContainerFormat::Zip));
    }

    let text_prefix = prefix.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(prefix);
    let looks_like_json = text_prefix
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
        == Some(b'{');
    if looks_like_json {
        let file = File::open(path).map_err(|error| GerberLoadFailure {
            path: path.to_path_buf(),
            message: format!("could not inspect Gerber job: {error}"),
        })?;
        if serde_json::from_reader::<_, serde_json::Value>(file)
            .ok()
            .is_some_and(|value| {
                value
                    .get("FilesAttributes")
                    .is_some_and(serde_json::Value::is_array)
            })
        {
            return Ok(Some(FabricationContainerFormat::GerberJob));
        }
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    match extension.to_ascii_lowercase().as_str()
    {
        "zip" => Err(GerberLoadFailure {
            path: path.to_path_buf(),
            message: "file extension indicates a ZIP archive, but the ZIP signature is missing"
                .to_owned(),
        }),
        "gbrjob" => Err(GerberLoadFailure {
            path: path.to_path_buf(),
            message: "file extension indicates a Gerber job, but the FilesAttributes JSON structure is missing"
                .to_owned(),
        }),
        _ => Ok(None),
    }
}

fn single_layer(layer: LoadedLayer) -> GerberLoadBatch {
    GerberLoadBatch {
        layers: vec![layer],
        failures: Vec::new(),
    }
}

fn single_load_failure(failure: GerberLoadFailure) -> GerberLoadBatch {
    GerberLoadBatch {
        layers: Vec::new(),
        failures: vec![failure],
    }
}

pub fn load_autodetected_reader<R>(
    name: impl Into<String>,
    mut reader: R,
) -> Result<LoadedLayer, GerberLoadFailure>
where
    R: Read,
{
    let name = name.into();
    let path = PathBuf::from(&name);
    let mut source = String::new();
    reader
        .read_to_string(&mut source)
        .map_err(|error| GerberLoadFailure {
            path: path.clone(),
            message: format!("fabrication source is not valid UTF-8 text: {error}"),
        })?;
    let gerber_signature =
        source.contains("%FS") || source.contains("%MO") || source.contains("%AD");
    let excellon_signature = source
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with(';'))
        .is_some_and(|line| line == "M48");

    let detected = match (gerber_signature, excellon_signature)
    {
        (true, true) => {
            return Err(GerberLoadFailure {
                path,
                message: "ambiguous fabrication file: contains both Gerber and Excellon signatures"
                    .to_owned(),
            });
        }
        (true, false) => FabricationFormat::Gerber,
        (false, true) => FabricationFormat::Excellon,
        (false, false) => format_from_extension(&name).ok_or_else(|| GerberLoadFailure {
            path: path.clone(),
            message: "unsupported fabrication file: no Gerber or Excellon signature or recognized extension"
                .to_owned(),
        })?,
    };

    let synthetic_name = match detected {
        FabricationFormat::Gerber => "autodetected.gbr",
        FabricationFormat::Excellon => "autodetected.drl",
    };
    let mut layer = match detected {
        FabricationFormat::Gerber => load_gerber_reader(synthetic_name, source.as_bytes()),
        FabricationFormat::Excellon => load_excellon_reader(synthetic_name, source.as_bytes()),
    }
    .map_err(|failure| GerberLoadFailure {
        path: path.clone(),
        message: failure.message,
    })?;
    layer.name = name;
    Ok(layer)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FabricationFormat {
    Gerber,
    Excellon,
}

fn format_from_extension(name: &str) -> Option<FabricationFormat> {
    let extension = Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if matches!(extension.to_ascii_lowercase().as_str(), "drl" | "drd") {
        return Some(FabricationFormat::Excellon);
    }
    if matches!(extension.to_ascii_lowercase().as_str(), "ger" | "pho")
        || LayerType::try_from(extension).is_ok_and(|layer_type| layer_type != LayerType::Drill)
    {
        return Some(FabricationFormat::Gerber);
    }
    None
}

/// Loads one RS-274X layer from an arbitrary reader.
///
/// This is the common parser seam used by disk loading and archive tests.
pub fn load_gerber_reader<R>(
    name: impl Into<String>,
    mut reader: R,
) -> Result<LoadedLayer, GerberLoadFailure>
where
    R: Read,
{
    let name = name.into();
    let path = PathBuf::from(&name);
    let mut original_source = String::new();
    reader
        .read_to_string(&mut original_source)
        .map_err(|error| GerberLoadFailure {
            path: path.clone(),
            message: format!("Gerber source is not valid UTF-8 text: {error}"),
        })?;
    let extension = Path::new(&name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let requested_type = match extension.to_ascii_lowercase().as_str() {
        "ger" | "pho" => LayerType::UndefinedGerber,
        _ => LayerType::try_from(extension).map_err(|_| GerberLoadFailure {
            path: path.clone(),
            message: format!("unsupported Gerber file extension '.{extension}'"),
        })?,
    };

    if requested_type == LayerType::Drill {
        return Err(GerberLoadFailure {
            path,
            message: "the file is an Excellon drill layer, not an RS-274X Gerber layer".into(),
        });
    }

    let (layer_type, data) =
        LayerData::parse(requested_type, BufReader::new(original_source.as_bytes())).map_err(
            |error| GerberLoadFailure {
                path: path.clone(),
                message: error.to_string(),
            },
        )?;
    let LayerData::Gerber(data) = data else {
        return Err(GerberLoadFailure {
            path,
            message: "the file was detected as Excellon drill data".into(),
        });
    };
    let geometry = GerberGeometry::from_layer(&data);

    Ok(LoadedLayer {
        source_path: None,
        original_source: Some(original_source),
        job_context: None,
        name,
        layer_type,
        data: LayerData::Gerber(data),
        geometry,
    })
}

/// Loads one Excellon drill file from disk.
pub fn load_excellon_file(path: impl AsRef<Path>) -> Result<LoadedLayer, GerberLoadFailure> {
    let path = path.as_ref();
    let file = File::open(path).map_err(|error| GerberLoadFailure {
        path: path.to_path_buf(),
        message: format!("could not open file: {error}"),
    })?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let mut layer = load_excellon_reader(name, file).map_err(|mut failure| {
        failure.path = path.to_path_buf();
        failure
    })?;
    layer.source_path = Some(path.to_path_buf());
    Ok(layer)
}

/// Loads several Excellon files, preserving successful layers when another file fails.
pub fn load_excellon_files<I, P>(paths: I) -> GerberLoadBatch
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let mut batch = GerberLoadBatch::default();
    for path in paths {
        match load_excellon_file(path.as_ref()) {
            Ok(layer) => batch.layers.push(layer),
            Err(failure) => batch.failures.push(failure),
        }
    }
    batch
}

/// Loads one Excellon drill layer from an arbitrary reader.
pub fn load_excellon_reader<R>(
    name: impl Into<String>,
    mut reader: R,
) -> Result<LoadedLayer, GerberLoadFailure>
where
    R: Read,
{
    let name = name.into();
    let path = PathBuf::from(&name);
    let mut original_source = String::new();
    reader
        .read_to_string(&mut original_source)
        .map_err(|error| GerberLoadFailure {
            path: path.clone(),
            message: format!("Excellon source is not valid UTF-8 text: {error}"),
        })?;
    let extension = Path::new(&name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let requested_type = LayerType::try_from(extension).map_err(|_| GerberLoadFailure {
        path: path.clone(),
        message: format!("unsupported Excellon file extension '.{extension}'"),
    })?;
    if requested_type != LayerType::Drill {
        return Err(GerberLoadFailure {
            path,
            message: "the file extension does not identify an Excellon drill layer".into(),
        });
    }

    let (_, data) = LayerData::parse(LayerType::Drill, BufReader::new(original_source.as_bytes()))
        .map_err(|error| GerberLoadFailure {
            path: path.clone(),
            message: error.to_string(),
        })?;
    let LayerData::Excellon(data) = data else {
        return Err(GerberLoadFailure {
            path,
            message: "the file was detected as RS-274X Gerber data".into(),
        });
    };
    let geometry = GerberGeometry::from_excellon(&data);

    Ok(LoadedLayer {
        source_path: None,
        original_source: Some(original_source),
        job_context: None,
        name,
        layer_type: LayerType::Drill,
        data: LayerData::Excellon(data),
        geometry,
    })
}
