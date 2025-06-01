use crate::dijkstra::{GlobePoints, GridPoint};
use crate::perlin;

use bevy::prelude::*;

#[derive(Debug)]
pub struct Config {
    pub grid_size: u32,
    pub perlin_config: perlin::PerlinConfig,
    pub water_penalty: f32,
    pub snow_penalty: f32,
    pub min_city_distance: f32,
    pub city_marker_size: f32,
    pub city_marker_color: Color,
    pub reduction_factor: f32, // cost reduction factor for reused edges
}

impl Default for Config {
    fn default() -> Self {
        Self {
            grid_size: 128,
            perlin_config: perlin::PerlinConfig {
                seed: 5,
                frequency: 2.0,
                lacunarity: 1.57,
                persistence: 0.5,
                octaves: 6,
            },
            water_penalty: 5.0,
            snow_penalty: 3.0,
            min_city_distance: 0.5,
            city_marker_size: 0.1,
            city_marker_color: Color::srgb_u8(124, 144, 255),
            reduction_factor: 2.0, // default reduction factor
        }
    }
}

#[derive(Resource, Default)]
pub struct State {
    pub globe_points: GlobePoints,
    pub cities: Vec<GridPoint>,
    pub config: Config,
}
