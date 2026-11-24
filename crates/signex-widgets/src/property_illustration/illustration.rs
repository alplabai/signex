//! The illustration as data: parts linked to field ids, and the features
//! switched off.

use iced::Size;

use super::field::FieldId;
use super::primitive::{Callout, Leader, Primitive, TextLabel};
use super::style::{Ink, Style};

/// Default height of an illustration, in multiples of the caption size.
pub const DEFAULT_HEIGHT_EM: f32 = 16.0;

/// One drawn part: a primitive, its style, the fields it shows and the
/// feature switch it depends on.
#[derive(Debug, Clone)]
pub struct Part {
    pub primitive: Primitive,
    pub style: Style,
    /// The fields this part shows. The first one is the field the part holds
    /// the value of. Hovering the row of any of them highlights the part.
    pub fields: Vec<FieldId>,
    /// The feature switch this part needs; the part is dropped while the
    /// feature is off.
    pub requires: Option<FieldId>,
}

impl Part {
    pub fn new(primitive: Primitive, style: Style) -> Self {
        Self {
            primitive,
            style,
            fields: Vec::new(),
            requires: None,
        }
    }

    /// A dimension callout in the callout ink.
    pub fn callout(callout: Callout) -> Self {
        Self::new(Primitive::Callout(callout), Style::callout())
    }

    /// A leader in the callout ink.
    pub fn leader(leader: Leader) -> Self {
        Self::new(Primitive::Leader(leader), Style::callout())
    }

    /// A label in `ink`.
    pub fn label(label: TextLabel, ink: Ink) -> Self {
        Self::new(
            Primitive::Label(label),
            Style::outline(ink, super::Weight::Thin),
        )
    }

    /// Links the part to one more field.
    pub fn linked(mut self, field: impl Into<FieldId>) -> Self {
        self.fields.push(field.into());
        self
    }

    /// Links the part to several fields, first one first.
    pub fn linked_all<I, F>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = F>,
        F: Into<FieldId>,
    {
        self.fields.extend(fields.into_iter().map(Into::into));
        self
    }

    /// Drops the part while `feature` is off.
    pub fn requires(mut self, feature: impl Into<FieldId>) -> Self {
        self.requires = Some(feature.into());
        self
    }

    /// The field the part holds the value of.
    pub fn primary_field(&self) -> Option<&FieldId> {
        self.fields.first()
    }

    pub fn is_linked_to(&self, field: &FieldId) -> bool {
        self.fields.contains(field)
    }
}

/// A property illustration: parts in drawing order on a view box.
#[derive(Debug, Clone)]
pub struct Illustration {
    view: Size,
    height_em: f32,
    parts: Vec<Part>,
    disabled: Vec<FieldId>,
}

impl Illustration {
    /// An empty illustration on a `width` x `height` view box.
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            view: Size::new(width.max(f32::EPSILON), height.max(f32::EPSILON)),
            height_em: DEFAULT_HEIGHT_EM,
            parts: Vec::new(),
            disabled: Vec::new(),
        }
    }

    /// The widget height, in multiples of the caption size.
    pub fn height_em(mut self, height_em: f32) -> Self {
        self.height_em = height_em.max(1.0);
        self
    }

    /// Appends a part; later parts draw on top.
    pub fn part(mut self, part: Part) -> Self {
        self.parts.push(part);
        self
    }

    /// Appends several parts in order.
    pub fn parts(mut self, parts: impl IntoIterator<Item = Part>) -> Self {
        self.parts.extend(parts);
        self
    }

    /// Sets whether a feature switch is on; parts that require it are
    /// dropped while it is off.
    pub fn feature(mut self, feature: impl Into<FieldId>, is_on: bool) -> Self {
        let feature = feature.into();
        self.disabled.retain(|f| f != &feature);
        if !is_on {
            self.disabled.push(feature);
        }
        self
    }

    pub fn view_size(&self) -> Size {
        self.view
    }

    pub fn height_in_em(&self) -> f32 {
        self.height_em
    }

    pub fn is_feature_on(&self, feature: &FieldId) -> bool {
        !self.disabled.contains(feature)
    }

    /// The parts drawn, in drawing order: every part whose feature is on.
    pub fn visible_parts(&self) -> impl DoubleEndedIterator<Item = &Part> {
        self.parts.iter().filter(|part| {
            part.requires
                .as_ref()
                .is_none_or(|feature| self.is_feature_on(feature))
        })
    }
}
