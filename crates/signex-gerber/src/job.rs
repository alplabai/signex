//! The Gerber job file (`.gbrjob`): JSON naming the images and drill files
//! of one package and the board they are for, with the keys the Signex
//! exporter writes (`signex_output` `fab::job`).

use serde_json::Value;

use crate::file_function::FileFunction;
use crate::image::{FilePolarity, Units};
use crate::number::mm_f64_to_nm;
use crate::problem::{ProblemKind, ReadProblem};
use crate::source::Source;

/// One file the job lists under `FilesAttributes`.
#[derive(Debug, Clone, PartialEq)]
pub struct JobEntry {
    pub path: String,
    pub function: FileFunction,
    pub polarity: Option<FilePolarity>,
}

/// A job file as read.
#[derive(Debug, Clone, PartialEq)]
pub struct JobFile {
    pub file: String,
    /// `GeneralSpecs.ProjectId.Name`.
    pub project_name: Option<String>,
    /// `GeneralSpecs.LayerNumber`.
    pub copper_layers: Option<u32>,
    /// `GeneralSpecs.Size.X` and `.Y`, mm in the file.
    pub board_size_nm: Option<(i64, i64)>,
    /// In `FilesAttributes` order.
    pub entries: Vec<JobEntry>,
    /// The `Path` of each `OtherFiles` entry.
    pub other_files: Vec<String>,
}

/// Read the job file `file`, or the one problem that stops it.
pub fn read_job(file: &str, text: &str) -> Result<JobFile, ReadProblem> {
    let source = Source::new(file, text);
    let bad = |line: usize, why: String| {
        let line = u32::try_from(line.max(1)).unwrap_or(u32::MAX);
        source.problem(line, ProblemKind::BadJobFile { why })
    };
    let value: Value = serde_json::from_str(text).map_err(|e| bad(e.line(), e.to_string()))?;
    let root = value
        .as_object()
        .ok_or_else(|| bad(1, "the job file is not a JSON object".into()))?;
    if !root.contains_key("Header") {
        return Err(bad(1, "the job file has no Header".into()));
    }
    let specs = root.get("GeneralSpecs");
    let spec = |path: &[&str]| path.iter().try_fold(specs?, |v, key| v.get(key));
    let size_nm = |axis| {
        spec(&["Size", axis])
            .and_then(Value::as_f64)
            .and_then(|mm| mm_f64_to_nm(mm, Units::Millimetres))
    };
    let entries = list(root, "FilesAttributes")
        .map_err(|why| bad(1, why))?
        .iter()
        .enumerate()
        .map(|(index, item)| entry(index, item).map_err(|why| bad(1, why)))
        .collect::<Result<Vec<_>, _>>()?;
    let other_files = list(root, "OtherFiles")
        .map_err(|why| bad(1, why))?
        .iter()
        .filter_map(|item| item.get("Path").and_then(Value::as_str))
        .map(str::to_string)
        .collect();
    Ok(JobFile {
        file: file.to_string(),
        project_name: spec(&["ProjectId", "Name"])
            .and_then(Value::as_str)
            .map(str::to_string),
        copper_layers: spec(&["LayerNumber"])
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok()),
        board_size_nm: size_nm("X").zip(size_nm("Y")),
        entries,
        other_files,
    })
}

/// The array under `key`; empty when absent.
fn list<'v>(root: &'v serde_json::Map<String, Value>, key: &str) -> Result<&'v [Value], String> {
    match root.get(key) {
        None => Ok(&[]),
        Some(Value::Array(items)) => Ok(items),
        Some(_) => Err(format!("{key} is not a list")),
    }
}

fn entry(index: usize, item: &Value) -> Result<JobEntry, String> {
    let text = |key: &str| item.get(key).and_then(Value::as_str);
    let path =
        text("Path").ok_or_else(|| format!("FilesAttributes entry {} has no Path", index + 1))?;
    let function = text("FileFunction")
        .ok_or_else(|| format!("FilesAttributes entry {} has no FileFunction", index + 1))?;
    let polarity = match text("FilePolarity") {
        Some("Positive") => Some(FilePolarity::Positive),
        Some("Negative") => Some(FilePolarity::Negative),
        _ => None,
    };
    Ok(JobEntry {
        path: path.to_string(),
        function: FileFunction::parse(function),
        polarity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_function::{CopperSide, Side};

    const JOB: &str = r#"{
  "Header": {
    "GenerationSoftware": { "Vendor": "Alp Lab", "Application": "Signex" }
  },
  "GeneralSpecs": {
    "ProjectId": { "Name": "fab" },
    "Size": { "X": 20.0, "Y": 10.0 },
    "LayerNumber": 2,
    "BoardThickness": 1.6
  },
  "FilesAttributes": [
    { "Path": "fab-copper-l1-top.gbr", "FileFunction": "Copper,L1,Top", "FilePolarity": "Positive" },
    { "Path": "fab-soldermask-top.gbr", "FileFunction": "Soldermask,Top", "FilePolarity": "Negative" }
  ],
  "OtherFiles": [ { "Path": "fab-centroid.csv", "Content": "PickAndPlace" } ]
}
"#;

    #[test]
    fn the_exporters_keys_read_back() {
        let job = read_job("fab-job.gbrjob", JOB).expect("reads");
        assert_eq!(job.project_name.as_deref(), Some("fab"));
        assert_eq!(job.copper_layers, Some(2));
        assert_eq!(job.board_size_nm, Some((20_000_000, 10_000_000)));
        assert_eq!(job.entries.len(), 2);
        assert_eq!(
            job.entries[0].function,
            FileFunction::Copper {
                number: 1,
                side: CopperSide::Top
            }
        );
        assert_eq!(job.entries[1].function, FileFunction::Soldermask(Side::Top));
        assert_eq!(job.entries[1].polarity, Some(FilePolarity::Negative));
        assert_eq!(job.other_files, vec!["fab-centroid.csv".to_string()]);
    }

    #[test]
    fn a_broken_job_file_is_one_problem_with_its_line() {
        let problem = read_job("j.gbrjob", "{\n  \"Header\": {},\n  oops\n}").expect_err("broken");
        assert_eq!(problem.at.line, 3);
        assert!(matches!(problem.kind, ProblemKind::BadJobFile { .. }));
        assert_eq!(problem.text, "oops");
        let no_header = read_job("j.gbrjob", "{}").expect_err("no header");
        assert_eq!(
            no_header.kind,
            ProblemKind::BadJobFile {
                why: "the job file has no Header".into()
            }
        );
        let no_path = read_job(
            "j.gbrjob",
            r#"{"Header":{},"FilesAttributes":[{"FileFunction":"Profile,NP"}]}"#,
        )
        .expect_err("no path");
        assert!(matches!(no_path.kind, ProblemKind::BadJobFile { .. }));
    }
}
