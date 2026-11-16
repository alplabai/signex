//! Board reports (#2404): the BOM, netlist, net lengths and component
//! placement of one board, as one table shape that the Board Reports dialog
//! shows, an agent reads as JSON and a CSV file holds. Built from the board
//! alone; routed length is `pcb_geom::net_routed_length_mm`.

mod build;

use serde::{Deserialize, Serialize};
use signex_types::pcb::{ComponentType, PcbBoard};

/// Which report a table is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardReportKind {
    /// The board's components grouped by value and footprint; types left
    /// out of the BOM (#2365) are not listed.
    Bom,
    /// Each net and the pads on it.
    Netlist,
    /// Each net's routed length, tracks and vias.
    NetLength,
    /// Each component's designator, footprint, position, rotation and side.
    Components,
}

impl BoardReportKind {
    /// Every report, in the order the dialog's tabs list them.
    pub const ALL: [Self; 4] = [Self::Bom, Self::Netlist, Self::NetLength, Self::Components];

    /// The stable id an operand, a file name and the JSON answer use.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Bom => "bom",
            Self::Netlist => "netlist",
            Self::NetLength => "net_length",
            Self::Components => "components",
        }
    }

    /// The report `id` names, if any.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.id() == id.trim())
    }

    /// The columns of this report, left to right.
    pub const fn columns(self) -> &'static [ReportColumn] {
        use ReportColumn as C;
        match self {
            Self::Bom => &[
                C::Designators,
                C::Quantity,
                C::Value,
                C::Footprint,
                C::Description,
                C::Type,
            ],
            Self::Netlist => &[C::Net, C::NodeCount, C::Nodes],
            Self::NetLength => &[C::Net, C::Length, C::Tracks, C::Vias],
            Self::Components => &[
                C::Designator,
                C::Footprint,
                C::Value,
                C::X,
                C::Y,
                C::Rotation,
                C::Side,
            ],
        }
    }
}

/// A report column. Its id heads the CSV column and names it in JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportColumn {
    Designators,
    Quantity,
    Value,
    Footprint,
    Description,
    Type,
    Net,
    NodeCount,
    Nodes,
    /// Routed length in mm.
    #[serde(rename = "length_mm")]
    Length,
    Tracks,
    Vias,
    Designator,
    /// X in mm from the board origin.
    #[serde(rename = "x_mm")]
    X,
    /// Y in mm from the board origin, up.
    #[serde(rename = "y_mm")]
    Y,
    #[serde(rename = "rotation_deg")]
    Rotation,
    Side,
}

impl ReportColumn {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Designators => "designators",
            Self::Quantity => "quantity",
            Self::Value => "value",
            Self::Footprint => "footprint",
            Self::Description => "description",
            Self::Type => "type",
            Self::Net => "net",
            Self::NodeCount => "node_count",
            Self::Nodes => "nodes",
            Self::Length => "length_mm",
            Self::Tracks => "tracks",
            Self::Vias => "vias",
            Self::Designator => "designator",
            Self::X => "x_mm",
            Self::Y => "y_mm",
            Self::Rotation => "rotation_deg",
            Self::Side => "side",
        }
    }
}

/// The board side a component sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportSide {
    Top,
    Bottom,
}

/// One cell. Numbers stay numbers, so the dialog formats a length in the
/// user's unit and JSON carries a number.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ReportCell {
    Text(String),
    Count(usize),
    Millimetres(f64),
    Degrees(f64),
    Side(ReportSide),
    Type(ComponentType),
    /// A value the board cannot give (a track end out of range).
    Missing,
}

impl ReportCell {
    /// The cell as a CSV field: mm to the nm, degrees to a thousandth.
    pub fn csv_text(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Count(n) => n.to_string(),
            Self::Millimetres(mm) => trimmed(*mm, 6),
            Self::Degrees(deg) => trimmed(*deg, 3),
            Self::Side(side) => match side {
                ReportSide::Top => "top".to_string(),
                ReportSide::Bottom => "bottom".to_string(),
            },
            Self::Type(kind) => serde_json::to_value(kind)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default(),
            Self::Missing => String::new(),
        }
    }
}

/// `value` with `places` decimals and no trailing zeros (`1.5`, `-2`).
fn trimmed(value: f64, places: usize) -> String {
    let text = format!("{:.*}", places, value);
    let text = if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        text
    };
    if text == "-0" { "0".to_string() } else { text }
}

/// A finished report: its kind, its columns and its rows of cells.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BoardReport {
    pub kind: BoardReportKind,
    pub columns: Vec<ReportColumn>,
    pub rows: Vec<Vec<ReportCell>>,
}

impl BoardReport {
    /// RFC 4180 CSV with a UTF-8 byte-order mark and CRLF line ends, the
    /// BOM export's form: a header row of column ids, then one line a row.
    pub fn to_csv(&self) -> Vec<u8> {
        let mut out = b"\xef\xbb\xbf".to_vec();
        let header: Vec<&str> = self.columns.iter().map(|c| c.id()).collect();
        push_row(&mut out, &header);
        for row in &self.rows {
            let fields: Vec<String> = row.iter().map(ReportCell::csv_text).collect();
            push_row(&mut out, &fields);
        }
        out
    }
}

fn push_row(out: &mut Vec<u8>, fields: &[impl AsRef<str>]) {
    // The BOM's writer: quoting, and spreadsheet formula leads neutralised.
    // It cannot fail; a failure would leave the row out, never the file.
    if crate::bom::csv::write_csv_row(out, fields).is_ok() {
        out.extend_from_slice(b"\r\n");
    }
}

/// The report `kind` of `board`.
pub fn board_report(board: &PcbBoard, kind: BoardReportKind) -> BoardReport {
    let rows = match kind {
        BoardReportKind::Bom => build::bom(board),
        BoardReportKind::Netlist => build::netlist(board),
        BoardReportKind::NetLength => build::net_length(board),
        BoardReportKind::Components => build::components(board),
    };
    BoardReport {
        kind,
        columns: kind.columns().to_vec(),
        rows,
    }
}

#[cfg(test)]
pub(crate) mod tests;
