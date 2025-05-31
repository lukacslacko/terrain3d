use std::collections::HashMap;

pub type GridPoint = (u32, u32, u32);

#[derive(Default)]
pub struct GlobePoints {
    pub size: u32,
    pub points: HashMap<GridPoint, [f32; 3]>,
}

pub fn dijkstra(start: GridPoint, end: GridPoint, globe_points: &GlobePoints) -> Vec<GridPoint> {
    return vec![];
}
