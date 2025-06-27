use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, RwLock};

use crate::dijkstra::{GlobePoints, GridPoint};
use crate::perlin;

use bevy::prelude::*;

#[derive(Debug, Clone)]
pub struct Config {
    pub grid_size: u32,
    pub sea_level: f32,  // sea level for the globe
    pub snow_level: f32, // snow level above sea level
    pub perlin_config: perlin::PerlinConfig,
    pub water_penalty: f32,
    pub snow_penalty: f32,
    pub min_city_distance: f32,
    pub reduction_factor: f32, // cost reduction factor for reused edges
    pub climbing_cost: f32,
    pub num_automatic_trains: i32, // number of automatic trains to spawn
}

impl Default for Config {
    fn default() -> Self {
        Self {
            grid_size: 256,
            sea_level: 4.95,
            snow_level: 0.5,
            perlin_config: perlin::PerlinConfig {
                seed: 485,
                frequency: 3.0,
                lacunarity: 1.57,
                persistence: 0.5,
                octaves: 6,
                amplitude: 0.75,
            },
            water_penalty: 5.0,
            snow_penalty: 3.0,
            min_city_distance: 1.0,
            reduction_factor: 2.0, // default reduction factor
            climbing_cost: 5.0,
            num_automatic_trains: 250,
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct Rail {
    pub from: GridPoint,
    pub to: GridPoint,
}

pub struct RailInfo {
    pub entity: Entity,
    pub counter: AtomicUsize,
    // Other details such as how frequently the rail is used can come here.
}

#[derive(Default)]
pub struct Rails {
    pub rails: HashMap<Rail, RailInfo>,
}

#[derive(Resource)]
pub struct State {
    pub globe_points: Arc<RwLock<GlobePoints>>,
    pub config: Config,
    pub rails: Rails,
    pub rng: rand::rngs::StdRng,
    pub create_new_city_next: bool,
    pub max_rail_usage: AtomicUsize,
}
