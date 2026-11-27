//! Rows and a template shaped like mockup row 52's Interface IC category.

use std::cmp::Ordering;

use crate::component::ComponentRow;
use crate::identity::InternalPn;
use crate::param::ParamValue;
use crate::templates::ParameterTemplate;

pub(super) fn m(value: f64, unit: &str) -> ParamValue {
    ParamValue::Measurement {
        value,
        unit: unit.into(),
    }
}

pub(super) fn text(s: &str) -> ParamValue {
    ParamValue::Text(s.into())
}

pub(super) fn row(pn: &str, params: &[(&str, ParamValue)]) -> ComponentRow {
    let mut row = crate::component::tests::fixture_row();
    row.internal_pn = InternalPn::new(pn);
    for (name, value) in params {
        row.parameters.insert((*name).into(), value.clone());
    }
    row
}

/// Case-folded order, standing in for the panel's display collation.
pub(super) fn text_order(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase())
}

pub(super) const INTERFACE_IC: &str = r#"
class = "interface_ic"

[[required_params]]
name = "protocol"
kind = "text"

[[required_params]]
name = "rate"
kind = "measurement"
unit = "bps"

[[optional_params]]
name = "supply_min"
kind = "measurement"
unit = "V"

[[optional_params]]
name = "supply_max"
kind = "measurement"
unit = "V"

[[optional_params]]
name = "package"
kind = "text"

[[optional_params]]
name = "temp_min"
kind = "measurement"
unit = "degC"
allow_negative = true

[[optional_params]]
name = "temp_max"
kind = "measurement"
unit = "degC"
allow_negative = true

[[optional_params]]
name = "tolerance"
kind = "measurement"
unit = "%"
display_plus_minus = true

[[ranges]]
name = "supply"
min = "supply_min"
max = "supply_max"

[[ranges]]
name = "temperature"
min = "temp_min"
max = "temp_max"
sort_by = "max"

[view]
columns = ["row:part_number", "protocol", "rate", "supply", "package", "temperature", "row:stock"]
sort = [{ column = "rate", descending = true }]
group = [{ column = "protocol" }]
pinned = ["protocol", "rate"]
"#;

pub(super) fn interface_ic() -> ParameterTemplate {
    ParameterTemplate::parse(INTERFACE_IC).expect("fixture template parses")
}

/// A CAN, an RS-485 and a LIN transceiver, plus one with no supply.
pub(super) fn interface_rows() -> Vec<ComponentRow> {
    vec![
        row(
            "CAN100",
            &[
                ("protocol", text("CAN FD")),
                ("rate", m(5e6, "bps")),
                ("supply_min", m(4.5, "V")),
                ("supply_max", m(5.5, "V")),
                ("temp_min", m(-40.0, "degC")),
                ("temp_max", m(125.0, "degC")),
                ("package", text("SOIC-8")),
            ],
        ),
        row(
            "RS485A",
            &[
                ("protocol", text("RS-485")),
                ("rate", m(20.0, "Mbps")),
                ("supply_min", m(3.0, "V")),
                ("supply_max", m(3.6, "V")),
                ("temp_min", m(-40.0, "°C")),
                ("temp_max", m(85.0, "°C")),
                ("package", text("MSOP-10")),
            ],
        ),
        row(
            "LIN7",
            &[
                ("protocol", text("LIN")),
                ("rate", m(20.0, "kbps")),
                ("supply_min", m(5.5, "V")),
                ("supply_max", m(28.0, "V")),
                ("temp_min", m(-40.0, "degC")),
                ("temp_max", m(105.0, "degC")),
                ("package", text("SOIC-8")),
            ],
        ),
        row(
            "can200",
            &[
                ("protocol", text("can fd")),
                ("rate", m(2e6, "bps")),
                ("package", text("DFN-8")),
            ],
        ),
    ]
}
