use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;

use crate::{GerberLoadBatch, GerberLoadFailure, load_autodetected_file};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JobFileAttributes {
    pub path: String,
    pub file_function: Option<String>,
    pub file_polarity: Option<String>,
    pub file_format: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GerberJobContext {
    pub job_path: PathBuf,
    pub document: Arc<serde_json::Value>,
    pub file_attributes: JobFileAttributes,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct GerberJobDocument {
    files_attributes: Vec<JobFileAttributes>,
}

pub fn load_gerber_job_file(path: impl AsRef<Path>) -> GerberLoadBatch {
    let path = path.as_ref();
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            return single_failure(path, format!("could not read Gerber job file: {error}"));
        }
    };
    let value = match serde_json::from_str::<serde_json::Value>(&source) {
        Ok(value) => value,
        Err(error) => {
            return single_failure(path, format!("invalid Gerber job JSON: {error}"));
        }
    };
    let document = match serde_json::from_value::<GerberJobDocument>(value.clone()) {
        Ok(document) => document,
        Err(error) => {
            return single_failure(path, format!("invalid Gerber job structure: {error}"));
        }
    };
    let Some(base) = path.parent() else {
        return single_failure(path, "Gerber job has no parent directory".to_owned());
    };
    let canonical_base = match base.canonicalize() {
        Ok(base) => base,
        Err(error) => {
            return single_failure(
                path,
                format!("could not resolve Gerber job directory: {error}"),
            );
        }
    };
    let document_value = Arc::new(value);
    let mut batch = GerberLoadBatch::default();

    for attributes in document.files_attributes {
        if !is_safe_relative_path(&attributes.path) {
            batch.failures.push(GerberLoadFailure {
                path: PathBuf::from(&attributes.path),
                message: "unsafe Gerber job member path was rejected".to_owned(),
            });
            continue;
        }
        let relative = Path::new(&attributes.path);
        let target = base.join(relative);
        let canonical_target = match target.canonicalize() {
            Ok(target) => target,
            Err(error) => {
                batch.failures.push(GerberLoadFailure {
                    path: target,
                    message: format!("could not resolve referenced file: {error}"),
                });
                continue;
            }
        };
        if !canonical_target.starts_with(&canonical_base) {
            batch.failures.push(GerberLoadFailure {
                path: target,
                message: "Gerber job member resolves outside the job directory".to_owned(),
            });
            continue;
        }

        match load_autodetected_file(&canonical_target) {
            Ok(mut layer) => {
                layer.job_context = Some(GerberJobContext {
                    job_path: path.to_path_buf(),
                    document: Arc::clone(&document_value),
                    file_attributes: attributes,
                });
                batch.layers.push(layer);
            }
            Err(failure) => batch.failures.push(failure),
        }
    }
    batch
}

fn is_safe_relative_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    let has_windows_drive_prefix =
        bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';

    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !has_windows_drive_prefix
        && path.split(['/', '\\']).all(|component| component != "..")
}

fn single_failure(path: &Path, message: String) -> GerberLoadBatch {
    GerberLoadBatch {
        layers: Vec::new(),
        failures: vec![GerberLoadFailure {
            path: path.to_path_buf(),
            message,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_loads_relative_files_and_preserves_complete_metadata() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let gerber_path = directory.path().join("board.gtl");
        std::fs::write(
            &gerber_path,
            b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.5*%\nD10*\nX0Y0D03*\nM02*\n",
        )
        .expect("write Gerber");
        let job_path = directory.path().join("board.gbrjob");
        std::fs::write(
            &job_path,
            r#"{
  "Header": {"Comment": "Preserve this metadata"},
  "GeneralSpecs": {"Owner": "Signex"},
  "FilesAttributes": [
    {
      "Path": "board.gtl",
      "FileFunction": "Copper,L1,Top",
      "FilePolarity": "Positive",
      "FileFormat": "Gerber"
    },
    {"Path": "missing.gbl", "FileFunction": "Copper,L2,Bot"}
  ]
}"#,
        )
        .expect("write job");

        let batch = load_gerber_job_file(&job_path);

        assert_eq!(batch.layers.len(), 1);
        assert_eq!(batch.failures.len(), 1);
        let context = batch.layers[0].job_context.as_ref().expect("job context");
        assert_eq!(context.job_path, job_path);
        assert_eq!(
            context.file_attributes.file_function.as_deref(),
            Some("Copper,L1,Top"),
        );
        assert_eq!(
            context.document["Header"]["Comment"],
            "Preserve this metadata",
        );
        assert_eq!(context.document["GeneralSpecs"]["Owner"], "Signex");
        let metadata = batch.layers[0].metadata();
        assert!(
            metadata
                .attributes
                .iter()
                .any(|attribute| attribute == "Job file function: Copper,L1,Top")
        );
        assert!(
            metadata
                .attributes
                .iter()
                .any(|attribute| attribute == "Job file polarity: Positive")
        );
    }

    #[test]
    fn job_rejects_parent_and_absolute_member_paths() {
        assert!(!is_safe_relative_path("../outside.gbr"));
        assert!(!is_safe_relative_path("..\\outside.gbr"));
        assert!(!is_safe_relative_path("C:\\outside.gbr"));
        assert!(!is_safe_relative_path("C:/outside.gbr"));
        assert!(!is_safe_relative_path("C:outside.gbr"));
        assert!(!is_safe_relative_path("\\\\server\\outside.gbr"));
        assert!(!is_safe_relative_path("\\outside.gbr"));
        assert!(!is_safe_relative_path("/outside.gbr"));
        assert!(is_safe_relative_path("plots/board.gtl"));
        assert!(is_safe_relative_path("plots\\board.gtl"));
    }
}
