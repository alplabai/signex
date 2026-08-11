use super::*;

#[derive(Debug, Deserialize)]
struct LayerPalette {
    negative_ghost_color: String,
    grid_color: String,
    d_code_color: String,
    compare_colors: Vec<String>,
    picker_families: Vec<MaterialColorFamily>,
    layer_slots: LayerSlots,
}

#[derive(Debug, Deserialize)]
struct MaterialColorFamily {
    name: String,
    shades: Vec<String>,
    colors: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LayerSlots {
    colors: Vec<String>,
}

pub(in crate::gerber_viewer) fn material_color_palette() -> Vec<GerberMaterialColor> {
    let palette: LayerPalette = toml::from_str(include_str!(
        "../../../../../assets/gerber-viewer/material-layer-colors.toml"
    ))
    .expect("bundled Material Design Gerber layer palette must parse");
    let colors = palette
        .picker_families
        .iter()
        .enumerate()
        .flat_map(|(family_index, family)| {
            family
                .shades
                .iter()
                .zip(&family.colors)
                .filter_map(move |(shade, value)| {
                    parse_hex_color(value).map(|color| GerberMaterialColor {
                        color,
                        family_index,
                        label: format!("{} {}", family.name, shade),
                    })
                })
        })
        .collect::<Vec<_>>();
    if colors.is_empty() {
        vec![GerberMaterialColor {
            color: Color::from_rgb8(211, 47, 47),
            family_index: 0,
            label: "Red 700".to_owned(),
        }]
    } else {
        colors
    }
}

pub(in crate::gerber_viewer) fn material_layer_palette() -> Vec<Color> {
    let palette: LayerPalette = toml::from_str(include_str!(
        "../../../../../assets/gerber-viewer/material-layer-colors.toml"
    ))
    .expect("bundled Material Design Gerber layer palette must parse");
    let colors = palette
        .layer_slots
        .colors
        .iter()
        .filter_map(|value| parse_hex_color(value))
        .collect::<Vec<_>>();
    if colors.is_empty() {
        vec![Color::from_rgb8(211, 47, 47)]
    } else {
        colors
    }
}

pub(in crate::gerber_viewer) fn material_negative_ghost_color() -> Color {
    let palette: LayerPalette = toml::from_str(include_str!(
        "../../../../../assets/gerber-viewer/material-layer-colors.toml"
    ))
    .expect("bundled Material Design Gerber layer palette must parse");
    parse_hex_color(&palette.negative_ghost_color)
        .unwrap_or_else(|| Color::from_rgb8(117, 117, 117))
}

pub(in crate::gerber_viewer) fn material_grid_color() -> Color {
    let palette: LayerPalette = toml::from_str(include_str!(
        "../../../../../assets/gerber-viewer/material-layer-colors.toml"
    ))
    .expect("bundled Material Design Gerber layer palette must parse");
    parse_hex_color(&palette.grid_color).unwrap_or_else(|| Color::from_rgb8(117, 117, 117))
}

pub(in crate::gerber_viewer) fn material_d_code_color() -> Color {
    let palette: LayerPalette = toml::from_str(include_str!(
        "../../../../../assets/gerber-viewer/material-layer-colors.toml"
    ))
    .expect("bundled Material Design Gerber layer palette must parse");
    parse_hex_color(&palette.d_code_color).unwrap_or_else(|| Color::from_rgb8(250, 250, 250))
}

pub(in crate::gerber_viewer) fn material_compare_palette() -> Vec<Color> {
    let palette: LayerPalette = toml::from_str(include_str!(
        "../../../../../assets/gerber-viewer/material-layer-colors.toml"
    ))
    .expect("bundled Material Design Gerber layer palette must parse");
    let colors = palette
        .compare_colors
        .iter()
        .filter_map(|value| parse_hex_color(value))
        .collect::<Vec<_>>();
    if colors.is_empty() {
        vec![Color::from_rgb8(211, 47, 47), Color::from_rgb8(0, 188, 212)]
    } else {
        colors
    }
}

pub(in crate::gerber_viewer) fn parse_hex_color(value: &str) -> Option<Color> {
    let value = value.strip_prefix('#')?;
    if value.len() != 6 {
        return None;
    }
    let red = u8::from_str_radix(&value[0..2], 16).ok()?;
    let green = u8::from_str_radix(&value[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&value[4..6], 16).ok()?;
    Some(Color::from_rgb8(red, green, blue))
}
