use lib_gerber_edit::gerber_types::{
    ComponentCharacteristics, ComponentMounting, Net, ObjectAttribute,
};

/// Gerber X2 object attributes active when a primitive was emitted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GerberObjectAttributes {
    pub component: Option<String>,
    pub nets: Vec<String>,
    pub attributes: Vec<GerberAttributeValue>,
}

/// A normalized Gerber X2 object-attribute name and its ordered values.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GerberAttributeValue {
    pub name: String,
    pub values: Vec<String>,
}

impl std::fmt::Display for GerberAttributeValue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.values.is_empty() {
            formatter.write_str(&self.name)
        } else {
            write!(formatter, "{}: {}", self.name, self.values.join(", "))
        }
    }
}

pub(crate) fn normalize_object_attribute(attribute: &ObjectAttribute) -> GerberAttributeValue {
    let (name, values) = match attribute {
        ObjectAttribute::Net(net) => (
            ".N",
            match net {
                Net::None => Vec::new(),
                Net::NotConnected => vec!["N/C".into()],
                Net::Connected(nets) => nets.clone(),
            },
        ),
        ObjectAttribute::Pin(pin) => {
            let mut values = vec![pin.refdes.clone(), pin.name.clone()];
            if let Some(function) = &pin.function {
                values.push(function.clone());
            }
            (".P", values)
        }
        ObjectAttribute::Component(component) => (".C", vec![component.clone()]),
        ObjectAttribute::ComponentCharacteristics(characteristic) => {
            normalize_component_characteristic(characteristic)
        }
        ObjectAttribute::UserDefined { name, values } => {
            return GerberAttributeValue {
                name: name.clone(),
                values: values.clone(),
            };
        }
    };

    GerberAttributeValue {
        name: name.to_owned(),
        values,
    }
}

fn normalize_component_characteristic(
    characteristic: &ComponentCharacteristics,
) -> (&'static str, Vec<String>) {
    match characteristic {
        ComponentCharacteristics::Rotation(value) => (".CRot", vec![value.to_string()]),
        ComponentCharacteristics::Manufacturer(value) => (".CMfr", vec![value.clone()]),
        ComponentCharacteristics::MPN(value) => (".CMPN", vec![value.clone()]),
        ComponentCharacteristics::Value(value) => (".CVal", vec![value.clone()]),
        ComponentCharacteristics::Mount(value) => {
            let value = match value {
                ComponentMounting::ThroughHole => "TH",
                ComponentMounting::SMD => "SMD",
                ComponentMounting::PressFit => "Pressfit",
                ComponentMounting::Other => "Other",
            };
            (".CMnt", vec![value.to_owned()])
        }
        ComponentCharacteristics::Footprint(value) => (".CFtp", vec![value.clone()]),
        ComponentCharacteristics::PackageName(value) => (".CPgN", vec![value.clone()]),
        ComponentCharacteristics::PackageDescription(value) => (".CPgD", vec![value.clone()]),
        ComponentCharacteristics::Height(value) => (".CHgt", vec![value.to_string()]),
        ComponentCharacteristics::LibraryName(value) => (".CLbN", vec![value.clone()]),
        ComponentCharacteristics::LibraryDescription(value) => (".CLbD", vec![value.clone()]),
        ComponentCharacteristics::Supplier(parts) => (
            ".CSup",
            parts
                .iter()
                .flat_map(|part| {
                    [
                        part.supplier_name.clone(),
                        part.supplier_part_reference.clone(),
                    ]
                })
                .collect(),
        ),
    }
}
