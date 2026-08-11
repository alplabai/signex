use pdf_writer::{Finish, Pdf, Rect, Ref};
use signex_gerber::{ApertureShape, Bounds, GerberPrimitive, Point, PrimitivePolarity};

use super::{GerberPageSize, GerberViewerState, ViewerLayer};

const MM_TO_PT: f64 = 72.0 / 25.4;
const PRINT_MARGIN_MM: f64 = 10.0;
const MIN_FULL_SIZE_DIMENSION_MM: f64 = 1.0;
const CIRCLE_BEZIER_FACTOR: f64 = 0.552_284_749_830_793_6;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GerberPrintPlan {
    pub page_size: GerberPageSize,
    pub page_width_mm: f64,
    pub page_height_mm: f64,
    pub printable_bounds_mm: Bounds,
    pub artwork_bounds_mm: Bounds,
    pub content_scale: f64,
    pub layers: Vec<GerberPrintLayer>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GerberPrintLayer {
    pub name: String,
    pub color: [f32; 3],
    pub primitives: Vec<GerberPrimitive>,
}

pub(crate) fn has_visible_layers(state: &GerberViewerState) -> bool {
    state.layers.iter().any(|layer| layer.visible)
}

pub(crate) fn build_plan(state: &GerberViewerState) -> Result<GerberPrintPlan, String> {
    let layout = state
        .print_layout()
        .ok_or_else(|| "There are no visible fabrication layers to print.".to_owned())?;
    let (page_width_mm, page_height_mm) = match layout.page_size {
        GerberPageSize::FullSize => (
            layout.bounds.width().max(MIN_FULL_SIZE_DIMENSION_MM) + PRINT_MARGIN_MM * 2.0,
            layout.bounds.height().max(MIN_FULL_SIZE_DIMENSION_MM) + PRINT_MARGIN_MM * 2.0,
        ),
        page_size => page_size
            .dimensions_millimetres()
            .expect("fixed Gerber page size has dimensions"),
    };
    let printable_width = (page_width_mm - PRINT_MARGIN_MM * 2.0).max(0.001);
    let printable_height = (page_height_mm - PRINT_MARGIN_MM * 2.0).max(0.001);
    let content_scale = (printable_width / layout.bounds.width().max(0.001))
        .min(printable_height / layout.bounds.height().max(0.001));
    let layers = state
        .layers
        .iter()
        .filter(|layer| layer.visible)
        .map(print_layer)
        .collect();

    Ok(GerberPrintPlan {
        page_size: layout.page_size,
        page_width_mm,
        page_height_mm,
        printable_bounds_mm: Bounds {
            min: Point {
                x: PRINT_MARGIN_MM,
                y: PRINT_MARGIN_MM,
            },
            max: Point {
                x: page_width_mm - PRINT_MARGIN_MM,
                y: page_height_mm - PRINT_MARGIN_MM,
            },
        },
        artwork_bounds_mm: layout.bounds,
        content_scale,
        layers,
    })
}

pub(crate) fn build_pdf(state: &GerberViewerState) -> Result<Vec<u8>, String> {
    let plan = build_plan(state)?;
    Ok(render_pdf(&plan))
}

fn print_layer(layer: &ViewerLayer) -> GerberPrintLayer {
    GerberPrintLayer {
        name: layer.layer.name.clone(),
        color: [layer.color.r, layer.color.g, layer.color.b],
        primitives: layer.layer.geometry.primitives.clone(),
    }
}

fn render_pdf(plan: &GerberPrintPlan) -> Vec<u8> {
    let catalog_id = Ref::new(1);
    let page_tree_id = Ref::new(2);
    let page_id = Ref::new(3);
    let content_id = Ref::new(4);
    let page_width_pt = (plan.page_width_mm * MM_TO_PT) as f32;
    let page_height_pt = (plan.page_height_mm * MM_TO_PT) as f32;
    let content = render_content(plan);
    let mut pdf = Pdf::new();

    pdf.catalog(catalog_id).pages(page_tree_id);
    pdf.pages(page_tree_id).kids([page_id]).count(1);
    pdf.stream(content_id, content.as_bytes());
    pdf.page(page_id)
        .parent(page_tree_id)
        .media_box(Rect::new(0.0, 0.0, page_width_pt, page_height_pt))
        .contents(content_id)
        .finish();
    pdf.finish()
}

fn render_content(plan: &GerberPrintPlan) -> String {
    let printable = plan.printable_bounds_mm;
    let printable_width = printable.width();
    let printable_height = printable.height();
    let drawn_width = plan.artwork_bounds_mm.width().max(0.001) * plan.content_scale;
    let drawn_height = plan.artwork_bounds_mm.height().max(0.001) * plan.content_scale;
    let offset_x = printable.min.x + (printable_width - drawn_width) * 0.5;
    let offset_y = printable.min.y + (printable_height - drawn_height) * 0.5;
    let transform = |point: Point| Point {
        x: offset_x + (point.x - plan.artwork_bounds_mm.min.x) * plan.content_scale,
        y: offset_y + (point.y - plan.artwork_bounds_mm.min.y) * plan.content_scale,
    };
    let mut content = String::new();
    content.push_str("q\n");
    append_rectangle(
        &mut content,
        printable.min.x * MM_TO_PT,
        printable.min.y * MM_TO_PT,
        printable.width() * MM_TO_PT,
        printable.height() * MM_TO_PT,
    );
    content.push_str("W n\n1 J\n1 j\n");

    for layer in &plan.layers {
        append_color(&mut content, layer.color);
        for primitive in &layer.primitives {
            append_primitive(
                &mut content,
                primitive,
                &transform,
                plan.content_scale,
                layer.color,
            );
        }
    }
    content.push_str("Q\n");
    content
}

fn append_primitive(
    content: &mut String,
    primitive: &GerberPrimitive,
    transform: &impl Fn(Point) -> Point,
    scale: f64,
    layer_color: [f32; 3],
) {
    match primitive {
        GerberPrimitive::Stroke {
            start,
            end,
            width,
            polarity,
            ..
        } => {
            append_polarity_color(content, *polarity, layer_color);
            append_line(content, transform(*start), transform(*end), *width * scale);
        }
        GerberPrimitive::Flash {
            position,
            aperture,
            polarity,
            ..
        } => {
            append_polarity_color(content, *polarity, layer_color);
            append_flash(content, transform(*position), aperture, scale);
        }
        GerberPrimitive::Region { points, polarity } => {
            if points.len() < 3 {
                return;
            }
            append_polarity_color(content, *polarity, layer_color);
            let first = transform(points[0]);
            content.push_str(&format!("{} {} m\n", pt(first.x), pt(first.y)));
            for point in &points[1..] {
                let point = transform(*point);
                content.push_str(&format!("{} {} l\n", pt(point.x), pt(point.y)));
            }
            content.push_str("h f\n");
        }
        GerberPrimitive::DrillHit {
            position, diameter, ..
        } => {
            append_color(content, layer_color);
            append_circle(content, transform(*position), diameter * scale * 0.5);
        }
        GerberPrimitive::DrillSlot {
            start, end, width, ..
        } => {
            append_color(content, layer_color);
            append_line(content, transform(*start), transform(*end), *width * scale);
        }
    }
}

fn append_flash(content: &mut String, center: Point, aperture: &ApertureShape, scale: f64) {
    match aperture {
        ApertureShape::Circle { diameter } => {
            append_circle(content, center, diameter * scale * 0.5);
        }
        ApertureShape::Rectangle { width, height } | ApertureShape::Obround { width, height } => {
            append_rectangle(
                content,
                (center.x - width * scale * 0.5) * MM_TO_PT,
                (center.y - height * scale * 0.5) * MM_TO_PT,
                width * scale * MM_TO_PT,
                height * scale * MM_TO_PT,
            );
            content.push_str("f\n");
        }
        ApertureShape::Polygon {
            diameter,
            vertices,
            rotation_degrees,
        } => {
            let count = usize::from((*vertices).max(3));
            let radius = diameter * scale * 0.5;
            let rotation = rotation_degrees.to_radians();
            for index in 0..count {
                let angle = rotation + std::f64::consts::TAU * index as f64 / count as f64;
                let point = Point {
                    x: center.x + radius * angle.cos(),
                    y: center.y + radius * angle.sin(),
                };
                content.push_str(&format!(
                    "{} {} {}\n",
                    pt(point.x),
                    pt(point.y),
                    if index == 0 { "m" } else { "l" },
                ));
            }
            content.push_str("h f\n");
        }
        ApertureShape::Macro { .. } => {
            append_circle(content, center, 0.075 * scale);
        }
    }
}

fn append_line(content: &mut String, start: Point, end: Point, width_mm: f64) {
    content.push_str(&format!(
        "{} w\n{} {} m\n{} {} l\nS\n",
        (width_mm * MM_TO_PT).max(0.1),
        pt(start.x),
        pt(start.y),
        pt(end.x),
        pt(end.y),
    ));
}

fn append_circle(content: &mut String, center: Point, radius_mm: f64) {
    let radius = radius_mm * MM_TO_PT;
    let center_x = pt(center.x);
    let center_y = pt(center.y);
    let control = radius * CIRCLE_BEZIER_FACTOR;
    content.push_str(&format!("{} {} m\n", center_x + radius, center_y));
    content.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        center_x + radius,
        center_y + control,
        center_x + control,
        center_y + radius,
        center_x,
        center_y + radius,
    ));
    content.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        center_x - control,
        center_y + radius,
        center_x - radius,
        center_y + control,
        center_x - radius,
        center_y,
    ));
    content.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        center_x - radius,
        center_y - control,
        center_x - control,
        center_y - radius,
        center_x,
        center_y - radius,
    ));
    content.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        center_x + control,
        center_y - radius,
        center_x + radius,
        center_y - control,
        center_x + radius,
        center_y,
    ));
    content.push_str("h f\n");
}

fn append_rectangle(content: &mut String, x: f64, y: f64, width: f64, height: f64) {
    content.push_str(&format!("{x} {y} {width} {height} re\n"));
}

fn append_polarity_color(content: &mut String, polarity: PrimitivePolarity, layer_color: [f32; 3]) {
    append_color(
        content,
        match polarity {
            PrimitivePolarity::Dark => layer_color,
            PrimitivePolarity::Clear => [1.0, 1.0, 1.0],
        },
    );
}

fn append_color(content: &mut String, color: [f32; 3]) {
    content.push_str(&format!(
        "{} {} {} RG\n{} {} {} rg\n",
        color[0], color[1], color[2], color[0], color[1], color[2],
    ));
}

fn pt(millimetres: f64) -> f64 {
    millimetres * MM_TO_PT
}

#[cfg(test)]
#[path = "../../tests/gerber_viewer/print.rs"]
mod gerber_print_test_definitions;

#[cfg(test)]
gerber_print_test_definitions::gerber_print_tests!();
