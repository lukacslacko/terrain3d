use crate::dijkstra::{GlobePoints, GridPoint};
use crate::perlin;

use bevy::prelude::*;

#[derive(Debug)]
pub struct Config {
    pub grid_size: u32,
    pub perlin_config: perlin::PerlinConfig,
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
        }
    }
}

#[derive(Resource, Default)]
pub struct State {
    pub globe_points: GlobePoints,
    pub cities: Vec<GridPoint>,
    pub config: Config,
}
