//! Side mirroring for the string layer names the board model stores today.
//! Replace with a typed layer mirror once the board carries typed layers.

const TOP_PREFIX: &str = "F.";
const BOTTOM_PREFIX: &str = "B.";

/// The name of the layer on the other side of the board: `F.<x>` and `B.<x>`
/// swap, every other name (inner copper, `*.Cu`, mechanical, user) is kept.
/// Mirroring twice gives back the input.
pub fn mirror_layer_name(name: &str) -> String {
    if let Some(rest) = name.strip_prefix(TOP_PREFIX) {
        format!("{BOTTOM_PREFIX}{rest}")
    } else if let Some(rest) = name.strip_prefix(BOTTOM_PREFIX) {
        format!("{TOP_PREFIX}{rest}")
    } else {
        name.to_owned()
    }
}

/// Mirror a layer name in place.
pub(super) fn mirror_in_place(name: &mut String) {
    *name = mirror_layer_name(name);
}
