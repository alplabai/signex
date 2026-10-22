use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::GerberPageSize;

const MILLIMETRES_PER_MIL: f64 = 0.0254;
const MILLIMETRES_PER_INCH: f64 = 25.4;
const SETTINGS_FILE_NAME: &str = "gerber_viewer.toml";
// Use the en-US decimal point only when the operating-system locale cannot be read.
const DEFAULT_DECIMAL_SEPARATOR: &str = ".";
pub(crate) const DEFAULT_GRID_INDEX: usize = 1;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GerberGridStyle {
    #[default]
    Dots,
    Lines,
    SmallCrosses,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GridUnit {
    Mil,
    Mm,
    Inch,
}

impl GridUnit {
    pub const ALL: [Self; 3] = [Self::Mm, Self::Mil, Self::Inch];
}

impl fmt::Display for GridUnit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Mil => "mil",
            Self::Mm => "mm",
            Self::Inch => "inch",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct GridSizePreset {
    pub name: Option<String>,
    pub x: f64,
    pub y: f64,
    pub unit: GridUnit,
}

impl GridSizePreset {
    pub fn x_millimetres(&self) -> f64 {
        match self.unit {
            GridUnit::Mil => self.x * MILLIMETRES_PER_MIL,
            GridUnit::Mm => self.x,
            GridUnit::Inch => self.x * MILLIMETRES_PER_INCH,
        }
    }

    pub fn y_millimetres(&self) -> f64 {
        match self.unit {
            GridUnit::Mil => self.y * MILLIMETRES_PER_MIL,
            GridUnit::Mm => self.y,
            GridUnit::Inch => self.y * MILLIMETRES_PER_INCH,
        }
    }

    fn x_mils(&self) -> f64 {
        match self.unit {
            GridUnit::Mil => self.x,
            GridUnit::Mm => self.x / MILLIMETRES_PER_MIL,
            GridUnit::Inch => self.x * 1_000.0,
        }
    }

    fn y_mils(&self) -> f64 {
        match self.unit {
            GridUnit::Mil => self.y,
            GridUnit::Mm => self.y / MILLIMETRES_PER_MIL,
            GridUnit::Inch => self.y * 1_000.0,
        }
    }

    pub fn converted_to(&self, unit: GridUnit) -> Self {
        let (x, y) = match unit {
            GridUnit::Mil => (self.x_mils(), self.y_mils()),
            GridUnit::Mm => (self.x_millimetres(), self.y_millimetres()),
            GridUnit::Inch => (
                self.x_millimetres() / MILLIMETRES_PER_INCH,
                self.y_millimetres() / MILLIMETRES_PER_INCH,
            ),
        };
        Self {
            name: self.name.clone(),
            x,
            y,
            unit,
        }
    }

    pub fn display_label(&self, decimal_separator: &str) -> String {
        let mils = format_dimensions(self.x_mils(), self.y_mils(), 2, "mils", decimal_separator);
        let millimetres = format_dimensions(
            self.x_millimetres(),
            self.y_millimetres(),
            4,
            "mm",
            decimal_separator,
        );
        let inches = format_dimensions(
            self.x_millimetres() / MILLIMETRES_PER_INCH,
            self.y_millimetres() / MILLIMETRES_PER_INCH,
            4,
            "inch",
            decimal_separator,
        );
        let dimensions = match self.unit {
            GridUnit::Mil => format!("{mils} ({millimetres})"),
            GridUnit::Mm => format!("{millimetres} ({mils})"),
            GridUnit::Inch => format!("{inches} ({millimetres})"),
        };

        match self.name.as_deref() {
            Some(name) if !name.trim().is_empty() => format!("{name}: {dimensions}"),
            _ => dimensions,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct GerberViewerSettings {
    grid_sizes: Vec<GridSizePreset>,
    #[serde(default)]
    grid_display: GridDisplaySettings,
    #[serde(default)]
    page_size: PageSizeSettings,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct GridDisplaySettings {
    style: GerberGridStyle,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct PageSizeSettings {
    size: GerberPageSize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GridSizeChoice {
    pub index: usize,
    label: String,
}

impl fmt::Display for GridSizeChoice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.label)
    }
}

pub(crate) fn default_grid_catalog() -> Vec<GridSizePreset> {
    let settings: GerberViewerSettings = toml::from_str(include_str!(
        "../../../../assets/gerber-viewer/default-settings.toml"
    ))
    .expect("bundled Gerber viewer settings must parse");
    settings.grid_sizes
}

#[derive(Debug, Deserialize)]
struct BundledDisplaySettings {
    drawing_mode: BundledDrawingModeSettings,
}

#[derive(Debug, Deserialize)]
struct BundledDrawingModeSettings {
    forced_opacity: f32,
    inactive_layer_opacity: f32,
}

pub(super) fn default_forced_opacity() -> f32 {
    let settings: BundledDisplaySettings = toml::from_str(include_str!(
        "../../../../assets/gerber-viewer/default-settings.toml"
    ))
    .expect("bundled Gerber viewer settings must parse");
    settings.drawing_mode.forced_opacity.clamp(0.0, 1.0)
}

pub(super) fn default_inactive_layer_opacity() -> f32 {
    let settings: BundledDisplaySettings = toml::from_str(include_str!(
        "../../../../assets/gerber-viewer/default-settings.toml"
    ))
    .expect("bundled Gerber viewer settings must parse");
    settings.drawing_mode.inactive_layer_opacity.clamp(0.0, 1.0)
}

pub(super) fn load_grid_catalog() -> Vec<GridSizePreset> {
    grid_settings_path()
        .and_then(|path| load_grid_catalog_from(&path).ok())
        .filter(|catalog| !catalog.is_empty())
        .unwrap_or_else(default_grid_catalog)
}

pub(super) fn load_page_size() -> GerberPageSize {
    grid_settings_path()
        .and_then(|path| load_settings_from(&path).ok())
        .unwrap_or_else(default_settings)
        .page_size
        .size
}

pub(super) fn load_grid_style() -> GerberGridStyle {
    grid_settings_path()
        .and_then(|path| load_settings_from(&path).ok())
        .unwrap_or_else(default_settings)
        .grid_display
        .style
}

pub(crate) fn persist_grid_catalog(catalog: &[GridSizePreset]) -> Result<(), String> {
    let path = grid_settings_path()
        .ok_or_else(|| "No operating-system configuration directory is available.".to_owned())?;
    let page_size = load_page_size();
    persist_settings_to(&path, catalog, page_size, load_grid_style())
}

pub(super) fn persist_page_size(
    page_size: GerberPageSize,
    catalog: &[GridSizePreset],
) -> Result<(), String> {
    let path = grid_settings_path()
        .ok_or_else(|| "No operating-system configuration directory is available.".to_owned())?;
    persist_settings_to(&path, catalog, page_size, load_grid_style())
}

pub(crate) fn create_grid_definition(
    name: &str,
    x: &str,
    y: &str,
    unit: GridUnit,
    decimal_separator: &str,
) -> Result<GridSizePreset, String> {
    let x = parse_distance(x, decimal_separator)
        .ok_or_else(|| "X must be a finite number greater than zero.".to_owned())?;
    if x <= 0.0 {
        return Err("X must be a finite number greater than zero.".to_owned());
    }

    let y = parse_distance(y, decimal_separator)
        .ok_or_else(|| "Y must be a finite non-negative number.".to_owned())?;
    if y < 0.0 {
        return Err("Y must be a finite non-negative number.".to_owned());
    }

    let name = name.trim();
    Ok(GridSizePreset {
        name: (!name.is_empty()).then(|| name.to_owned()),
        x,
        y,
        unit,
    })
}

pub(crate) fn grid_size_choices(
    catalog: &[GridSizePreset],
    decimal_separator: &str,
) -> Vec<GridSizeChoice> {
    catalog
        .iter()
        .enumerate()
        .map(|(index, grid)| GridSizeChoice {
            index,
            label: grid.display_label(decimal_separator),
        })
        .collect()
}

pub(super) fn system_decimal_separator() -> String {
    platform_decimal_separator().unwrap_or_else(|| DEFAULT_DECIMAL_SEPARATOR.to_owned())
}

#[cfg(windows)]
fn platform_decimal_separator() -> Option<String> {
    use windows_sys::Win32::Globalization::{GetLocaleInfoEx, LOCALE_SDECIMAL};

    let mut buffer = [0_u16; 8];
    let length = unsafe {
        GetLocaleInfoEx(
            std::ptr::null(),
            LOCALE_SDECIMAL,
            buffer.as_mut_ptr(),
            buffer.len() as i32,
        )
    };
    if length <= 1 {
        return None;
    }

    String::from_utf16(&buffer[..length as usize - 1])
        .ok()
        .filter(|separator| !separator.is_empty())
}

#[cfg(unix)]
fn platform_decimal_separator() -> Option<String> {
    let output = std::process::Command::new("locale")
        .arg("decimal_point")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    parse_posix_decimal_separator(&output.stdout)
}

#[cfg(any(unix, test))]
fn parse_posix_decimal_separator(output: &[u8]) -> Option<String> {
    let output = std::str::from_utf8(output).ok()?.trim();
    let value = output
        .split_once('=')
        .map_or(output, |(_, value)| value)
        .trim()
        .trim_matches('"');

    (!value.is_empty()).then(|| value.to_owned())
}

#[cfg(not(any(unix, windows)))]
fn platform_decimal_separator() -> Option<String> {
    None
}

fn format_dimensions(
    x: f64,
    y: f64,
    decimals: usize,
    unit: &str,
    decimal_separator: &str,
) -> String {
    if x == y {
        format!("{} {unit}", format_decimal(x, decimals, decimal_separator),)
    } else {
        format!(
            "{} {unit} ⨯ {} {unit}",
            format_decimal(x, decimals, decimal_separator),
            format_decimal(y, decimals, decimal_separator),
        )
    }
}

fn format_decimal(value: f64, decimals: usize, decimal_separator: &str) -> String {
    format!("{value:.decimals$}").replace('.', decimal_separator)
}

pub(crate) fn format_distance_input(value: f64, decimal_separator: &str) -> String {
    value.to_string().replace('.', decimal_separator)
}

fn parse_distance(value: &str, decimal_separator: &str) -> Option<f64> {
    let value = value.trim();
    let normalized = if decimal_separator == DEFAULT_DECIMAL_SEPARATOR {
        value.to_owned()
    } else {
        value.replace(decimal_separator, DEFAULT_DECIMAL_SEPARATOR)
    };
    normalized
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn grid_settings_path() -> Option<PathBuf> {
    let root = if cfg!(test) {
        std::env::temp_dir().join(format!("signex-widget-test-prefs-{}", std::process::id(),))
    } else {
        dirs::config_dir()?.join("signex")
    };
    Some(root.join(SETTINGS_FILE_NAME))
}

fn load_grid_catalog_from(path: &Path) -> Result<Vec<GridSizePreset>, String> {
    let settings = load_settings_from(path)?;
    if settings
        .grid_sizes
        .iter()
        .any(|grid| !grid.x.is_finite() || grid.x <= 0.0 || !grid.y.is_finite() || grid.y < 0.0)
    {
        return Err("The persisted grid catalog contains invalid distances.".to_owned());
    }
    Ok(settings.grid_sizes)
}

#[cfg(test)]
fn persist_grid_catalog_to(path: &Path, catalog: &[GridSizePreset]) -> Result<(), String> {
    persist_settings_to(
        path,
        catalog,
        GerberPageSize::default(),
        GerberGridStyle::default(),
    )
}

fn default_settings() -> GerberViewerSettings {
    toml::from_str(include_str!(
        "../../../../assets/gerber-viewer/default-settings.toml"
    ))
    .expect("bundled Gerber viewer settings must parse")
}

fn load_settings_from(path: &Path) -> Result<GerberViewerSettings, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    toml::from_str(&source).map_err(|error| format!("Could not parse {}: {error}", path.display()))
}

fn persist_settings_to(
    path: &Path,
    catalog: &[GridSizePreset],
    page_size: GerberPageSize,
    grid_style: GerberGridStyle,
) -> Result<(), String> {
    let settings = GerberViewerSettings {
        grid_sizes: catalog.to_vec(),
        grid_display: GridDisplaySettings { style: grid_style },
        page_size: PageSizeSettings { size: page_size },
    };
    let source = toml::to_string_pretty(&settings)
        .map_err(|error| format!("Could not serialize Gerber viewer settings: {error}"))?;
    signex_types::atomic_io::atomic_write(path, source.as_bytes())
        .map_err(|error| format!("Could not save {}: {error}", path.display()))
}

#[cfg(test)]
#[path = "../../tests/gerber_viewer/grid.rs"]
mod gerber_grid_test_definitions;

#[cfg(test)]
gerber_grid_test_definitions::gerber_grid_tests!();
