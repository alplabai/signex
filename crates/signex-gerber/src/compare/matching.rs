//! Which board layer each package layer is compared with: copper by its
//! number L1..Ln, mask, paste and legend by side, the profile with the
//! board shape, a drill file with the holes of the same plating and span.

use crate::package::Package;
use crate::{FileFunction, Image, Side};

use super::LayerMatch;

/// What a layer is compared by. Two functions with the same key are the
/// same board layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    Copper(u32),
    Soldermask(Side),
    Paste(Side),
    Legend(Side),
    Profile,
    Drill { plated: bool, from: u32, to: u32 },
}

/// The key of `function` on a board of `copper` copper layers; `None`
/// when the board has no such layer (a drawing, a copper number or a drill
/// span past its stack).
fn key(function: &FileFunction, copper: u32) -> Option<Key> {
    match *function {
        FileFunction::Copper { number, .. } => (1..=copper)
            .contains(&number)
            .then_some(Key::Copper(number)),
        FileFunction::Soldermask(side) => Some(Key::Soldermask(side)),
        FileFunction::Paste(side) => Some(Key::Paste(side)),
        FileFunction::Legend(side) => Some(Key::Legend(side)),
        FileFunction::Profile { .. } => Some(Key::Profile),
        FileFunction::Drill {
            plated, from, to, ..
        } => ((1..=to).contains(&from) && to <= copper).then_some(Key::Drill { plated, from, to }),
        FileFunction::Other(_) => None,
    }
}

/// The board's image of `board_layer`; `None` when the board draws
/// nothing there, so everything in the package's file is extra.
pub(super) fn reference_image<'a>(
    reference: &'a Package,
    board_layer: &FileFunction,
    copper: u32,
) -> Option<&'a Image> {
    let wanted = key(board_layer, copper)?;
    reference
        .layers
        .iter()
        .find(|layer| {
            layer
                .function
                .as_ref()
                .and_then(|function| key(function, copper))
                == Some(wanted)
        })
        .map(|layer| &layer.image)
}

/// One match per package layer, in layer order.
pub(super) fn match_layers(
    package: &Package,
    reference: &Package,
    copper: u32,
    by_hand: &[(usize, Option<FileFunction>)],
) -> Vec<LayerMatch> {
    package
        .layers
        .iter()
        .enumerate()
        .map(|(index, layer)| {
            let chosen = by_hand.iter().rev().find(|(at, _)| *at == index);
            let (function, by_hand) = match chosen {
                Some((_, function)) => (function.as_ref(), true),
                None => (layer.function.as_ref(), false),
            };
            LayerMatch {
                layer: index,
                board: function.and_then(|f| board_layer(f, reference, copper)),
                by_hand,
            }
        })
        .collect()
}

/// The board layer `function` stands for: the board image's own function
/// when the board draws that layer, else `function` as given; `None` when
/// the board has no such layer.
fn board_layer(function: &FileFunction, reference: &Package, copper: u32) -> Option<FileFunction> {
    key(function, copper)?;
    let image = reference_image(reference, function, copper);
    Some(
        image
            .and_then(|image| image.function.clone())
            .unwrap_or_else(|| function.clone()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CopperSide;

    #[test]
    fn copper_matches_by_number_and_drills_by_span() {
        let copper = |number| FileFunction::Copper {
            number,
            side: CopperSide::Inner,
        };
        assert_eq!(key(&copper(2), 4), Some(Key::Copper(2)));
        assert_eq!(key(&copper(5), 4), None, "past the board's stack");
        let drill = |from, to| FileFunction::Drill {
            plated: true,
            from,
            to,
            kind: crate::DrillKind::Pth,
        };
        assert_eq!(
            key(&drill(1, 2), 2),
            Some(Key::Drill {
                plated: true,
                from: 1,
                to: 2
            })
        );
        assert_eq!(key(&drill(1, 4), 2), None);
        assert_eq!(key(&FileFunction::Other("Vcut".into()), 2), None);
        assert_eq!(
            key(&FileFunction::Legend(Side::Bottom), 2),
            Some(Key::Legend(Side::Bottom))
        );
    }
}
