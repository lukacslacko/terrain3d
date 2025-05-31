use crate::dijkstra::{GlobePoints, GridPoint};

use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct State {
    pub globe_points: GlobePoints,
    pub cities: Vec<GridPoint>,
}
