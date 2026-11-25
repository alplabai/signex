//! The file tokens of the block diagram's enums, for the callers outside
//! this crate that read or show them (the `block_*` commands, the
//! Properties pickers): the same tables the reader and writer use, so a
//! command accepts exactly the words the file holds.

use super::{
    BlockShape, INTERFACE_TOKENS, Interface, PortDirection, PortSide, direction_from_token,
    direction_token, shape_from_token, shape_token, side_from_token, side_token,
};

impl BlockShape {
    /// Every shape, in the order a picker lists them.
    pub const ALL: [Self; 4] = [
        Self::Rectangle,
        Self::RoundedRectangle,
        Self::Ellipse,
        Self::Hexagon,
    ];

    /// The token the file holds.
    pub fn token(self) -> &'static str {
        shape_token(self)
    }

    /// The shape `text` names, by its exact token.
    pub fn from_token(text: &str) -> Option<Self> {
        shape_from_token(text).ok()
    }
}

impl PortSide {
    /// Every side, in the order a picker lists them.
    pub const ALL: [Self; 4] = [Self::Left, Self::Right, Self::Top, Self::Bottom];

    /// The token the file holds.
    pub fn token(self) -> &'static str {
        side_token(self)
    }

    /// The side `text` names, by its exact token.
    pub fn from_token(text: &str) -> Option<Self> {
        side_from_token(text).ok()
    }
}

impl PortDirection {
    /// Every direction, in the order a picker lists them.
    pub const ALL: [Self; 3] = [Self::Input, Self::Output, Self::Bidirectional];

    /// The token the file holds.
    pub fn token(self) -> &'static str {
        direction_token(self)
    }

    /// The direction `text` names, by its exact token.
    pub fn from_token(text: &str) -> Option<Self> {
        direction_from_token(text).ok()
    }
}

impl Interface {
    /// Every interface this build names, without [`Interface::Other`], in
    /// the order a picker lists them.
    pub fn known() -> impl Iterator<Item = Interface> {
        INTERFACE_TOKENS
            .iter()
            .map(|(interface, _)| interface.clone())
    }

    /// The fixed token of a known interface; `None` for
    /// [`Interface::Other`].
    pub fn known_token(&self) -> Option<&'static str> {
        INTERFACE_TOKENS
            .iter()
            .find(|(interface, _)| interface == self)
            .map(|(_, token)| *token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_token_reads_back_as_its_value_and_others_are_refused() {
        for shape in BlockShape::ALL {
            assert_eq!(BlockShape::from_token(shape.token()), Some(shape));
        }
        for side in PortSide::ALL {
            assert_eq!(PortSide::from_token(side.token()), Some(side));
        }
        for direction in PortDirection::ALL {
            assert_eq!(
                PortDirection::from_token(direction.token()),
                Some(direction)
            );
        }
        assert_eq!(BlockShape::from_token("Rectangle"), None);
        assert_eq!(PortSide::from_token("middle"), None);
        assert_eq!(PortDirection::from_token(""), None);
    }

    #[test]
    fn the_known_interfaces_are_the_file_tokens_and_none_is_other() {
        let known: Vec<Interface> = Interface::known().collect();
        assert_eq!(known.len(), INTERFACE_TOKENS.len());
        assert!(known.iter().all(|i| !matches!(i, Interface::Other(_))));
        assert_eq!(known.first(), Some(&Interface::Signal));
        for interface in &known {
            assert_eq!(interface.known_token(), Some(interface.token()));
        }
        assert_eq!(Interface::Other("CAN".into()).known_token(), None);
    }
}
