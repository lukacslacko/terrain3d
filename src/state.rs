use crate::dijkstra::GlobePoints;

use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct State {
    pub globe_points: GlobePoints,
}
