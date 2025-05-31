use std::collections::HashMap;

#[derive(Default)]
pub struct GlobePoints {
    pub size: u32,
    pub points: HashMap<(u32, u32, u32), [f32; 3]>,
}

pub fn dijkstra(
    start: (u32, u32, u32),
    end: (u32, u32, u32),
    globe_points: &GlobePoints,
) -> Vec<(u32, u32, u32)> {
    return vec![];
}
