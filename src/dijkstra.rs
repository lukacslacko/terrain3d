use std::collections::HashMap;

#[derive(Default)]
pub struct GlobePoints {
    pub points: HashMap<(u32, u32, u32), [f32; 3]>,
}
