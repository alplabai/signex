#[derive(Debug, Clone)]
pub enum SelectionMessage {
    SelectAll,
    HitAt { world_x: f64, world_y: f64 },
    BoxSelect { x1: f64, y1: f64, x2: f64, y2: f64 },
}
