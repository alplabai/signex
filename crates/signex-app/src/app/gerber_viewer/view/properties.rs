//! Properties, read only (mockup row 193): for the selected object its
//! type, layer, file and line, D-code, aperture and its definition, X2
//! aperture function, X2 net, pin and component, position and polarity;
//! with nothing selected, the current layer's file function, units, object
//! count and creator.

use iced::widget::{Column, column, container, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use pcb_geom::PointNm;
use signex_gerber::{Object, ObjectKind, PackageLayer, Polarity, Units};
use signex_i18n::{MessageId, UserText, ids, t};
use signex_widgets::{Label, text};

use super::layers::layer_name;
use super::toolbar::unit_label;
use super::{Look, SP, TS};
use crate::app::gerber_viewer::GerberViewerMsg;
use crate::app::gerber_viewer::state::{GerberViewerState, LengthUnit, OpenPackage};
use crate::styles::ti;
use crate::styles::tokens::resolve;

/// One row: its name and its value.
type Fact = (MessageId, Label<'static>);

pub(super) fn view<'a>(
    look: Look<'a>,
    viewer: &'a GerberViewerState,
    open: &'a OpenPackage,
) -> Element<'a, GerberViewerMsg> {
    let facts = match open.selected.and_then(|at| Some((at, open.object(at)?))) {
        Some((at, object)) => {
            object_facts(look, viewer.unit, &open.package.layers[at.layer], object)
        }
        None => match open
            .current
            .and_then(|layer| open.package.layers.get(layer))
        {
            Some(layer) => layer_facts(look, layer),
            None => Vec::new(),
        },
    };
    let mut list: Column<Element<'a, GerberViewerMsg>> = column![
        text(look.t(ids::GERBER_VIEWER_PROPERTIES))
            .size(TS.section)
            .color(resolve(look.tokens).ink3)
            .boxed()
    ]
    .spacing(SP.xs);
    if facts.is_empty() {
        list = list.push(
            text(look.t(ids::GERBER_VIEWER_PROP_NOTHING))
                .size(TS.dense)
                .color(ti(look.tokens.text_secondary))
                .boxed(),
        );
    }
    for (name, value) in facts {
        list = list.push(fact_row(look, name, value));
    }
    container(scrollable(list.padding(SP.s)).height(Length::Fill))
        .height(Length::Fill)
        .style(crate::styles::panel_region(look.tokens))
        .boxed()
}

fn fact_row<'a>(
    look: Look<'a>,
    name: MessageId,
    value: Label<'static>,
) -> Element<'a, GerberViewerMsg> {
    row![
        text(look.t(name))
            .size(TS.dense)
            .color(resolve(look.tokens).ink3)
            .width(Length::FillPortion(2))
            .boxed(),
        text(value)
            .size(TS.dense)
            .color(ti(look.tokens.text))
            .width(Length::FillPortion(3))
            .boxed(),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Start)
    .boxed()
}

fn user(value: impl Into<String>) -> Label<'static> {
    UserText::new(value.into()).into()
}

/// The facts of one object.
fn object_facts(
    look: Look<'_>,
    unit: LengthUnit,
    layer: &PackageLayer,
    object: &Object,
) -> Vec<Fact> {
    let mut facts: Vec<Fact> = vec![
        (
            ids::GERBER_VIEWER_PROP_TYPE,
            look.t(type_name(&object.kind)).into(),
        ),
        (ids::GERBER_VIEWER_PROP_LAYER, layer_name(look, layer)),
        (
            ids::GERBER_VIEWER_PROP_FILE,
            t!(
                look.i18n,
                ids::GERBER_VIEWER_PROP_FILE_LINE,
                "file" => object.at.file.as_str(),
                "line" => object.at.line.to_string()
            )
            .into(),
        ),
    ];
    facts.extend(aperture_facts(look, layer, object));
    facts.extend(attribute_facts(look, object));
    if let Some(at) = object.anchors().first() {
        facts.push((ids::GERBER_VIEWER_PROP_POSITION, position(look, unit, *at)));
    }
    if let ObjectKind::Hole {
        diameter: Some(diameter),
        ..
    } = object.kind
    {
        let size = t!(
            look.i18n,
            ids::GERBER_VIEWER_PROP_LENGTH,
            "value" => unit.show(diameter),
            "unit" => look.t(unit_label(unit)).into_string()
        );
        facts.push((ids::GERBER_VIEWER_PROP_SIZE, size.into()));
    }
    let polarity = match object.polarity {
        Polarity::Dark => ids::GERBER_VIEWER_POLARITY_DARK,
        Polarity::Clear => ids::GERBER_VIEWER_POLARITY_CLEAR,
    };
    facts.push((ids::GERBER_VIEWER_PROP_POLARITY, look.t(polarity).into()));
    facts
}

/// The D-code (or drill tool), its definition and its X2 function.
fn aperture_facts(look: Look<'_>, layer: &PackageLayer, object: &Object) -> Vec<Fact> {
    let Some(code) = object.aperture else {
        return Vec::new();
    };
    let prefix = if matches!(object.kind, ObjectKind::Hole { .. }) {
        "T"
    } else {
        "D"
    };
    let mut facts = vec![(
        ids::GERBER_VIEWER_PROP_D_CODE,
        user(format!("{prefix}{code}")),
    )];
    let Some(definition) = layer.image.apertures.get(&code) else {
        facts.push((
            ids::GERBER_VIEWER_PROP_APERTURE,
            look.t(ids::GERBER_VIEWER_PROP_NONE).into(),
        ));
        return facts;
    };
    facts.push((
        ids::GERBER_VIEWER_PROP_APERTURE,
        user(definition.definition.clone()),
    ));
    if let Some(function) = &definition.function {
        facts.push((
            ids::GERBER_VIEWER_PROP_APERTURE_FUNCTION,
            user(function.clone()),
        ));
    }
    facts
}

/// The X2 net, pin and component, where the file gives them.
fn attribute_facts(look: Look<'_>, object: &Object) -> Vec<Fact> {
    let attributes = &object.attributes;
    let mut facts: Vec<Fact> = Vec::new();
    if let Some(net) = &attributes.net {
        facts.push((ids::GERBER_VIEWER_PROP_NET, user(net.clone())));
    }
    if let Some((designator, pad)) = &attributes.pin {
        let pin = t!(
            look.i18n,
            ids::GERBER_VIEWER_PROP_PIN_VALUE,
            "component" => designator.as_str(),
            "pin" => pad.as_str()
        );
        facts.push((ids::GERBER_VIEWER_PROP_PIN, pin.into()));
    }
    if let Some(component) = &attributes.component {
        facts.push((ids::GERBER_VIEWER_PROP_COMPONENT, user(component.clone())));
    }
    facts
}

fn position(look: Look<'_>, unit: LengthUnit, at: PointNm) -> Label<'static> {
    t!(
        look.i18n,
        ids::GERBER_VIEWER_PROP_XY,
        "x" => unit.show(at.x),
        "y" => unit.show(at.y),
        "unit" => look.t(unit_label(unit)).into_string()
    )
    .into()
}

/// What an object is, as the Type row names it.
fn type_name(kind: &ObjectKind) -> MessageId {
    match kind {
        ObjectKind::Flash { .. } => ids::GERBER_VIEWER_TYPE_FLASH,
        ObjectKind::Line { .. } => ids::GERBER_VIEWER_TYPE_LINE,
        ObjectKind::Arc { .. } => ids::GERBER_VIEWER_TYPE_ARC,
        ObjectKind::Region { .. } => ids::GERBER_VIEWER_TYPE_REGION,
        ObjectKind::Hole { slot_to: None, .. } => ids::GERBER_VIEWER_TYPE_HOLE,
        ObjectKind::Hole { .. } => ids::GERBER_VIEWER_TYPE_SLOT,
    }
}

/// The facts of a layer with nothing selected.
fn layer_facts(look: Look<'_>, layer: &PackageLayer) -> Vec<Fact> {
    let image = &layer.image;
    let function = match &layer.function {
        Some(function) => user(function.as_x2()),
        None => look.t(ids::GERBER_VIEWER_PROP_NONE).into(),
    };
    let units = match image.units {
        Some(Units::Millimetres) => ids::GERBER_VIEWER_UNITS_MM,
        Some(Units::Inches) => ids::GERBER_VIEWER_UNITS_INCH,
        None => ids::GERBER_VIEWER_PROP_NONE,
    };
    let creator = image
        .generation_software
        .clone()
        .map_or_else(|| look.t(ids::GERBER_VIEWER_PROP_NONE).into(), user);
    vec![
        (ids::GERBER_VIEWER_PROP_LAYER, layer_name(look, layer)),
        (ids::GERBER_VIEWER_PROP_FILE, user(image.file.clone())),
        (ids::GERBER_VIEWER_PROP_FUNCTION, function),
        (ids::GERBER_VIEWER_PROP_UNITS, look.t(units).into()),
        (
            ids::GERBER_VIEWER_PROP_OBJECTS,
            user(image.objects.len().to_string()),
        ),
        (ids::GERBER_VIEWER_PROP_CREATOR, creator),
    ]
}
