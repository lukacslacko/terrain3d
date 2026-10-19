use crate::dijkstra::{GlobePoint, GlobePoints, GridPoint, STEPS, cubic};
use bevy::math::Vec3;

#[test]
fn test_grid_point() {
    let point1: GridPoint = (1, 2, 3);
    let point2: GridPoint = (1, 2, 3);
    let point3: GridPoint = (1, 2, 4);
    let point4: GridPoint = (2, 2, 3);
    assert_eq!(point1, point2);
    assert_ne!(point1, point3);
    assert_ne!(point2, point3);
    assert_ne!(point1, point4);
}

fn all_grid_points(grid_size: u32) -> Vec<GridPoint> {
    let mut points = Vec::new();
    for face in 0..6 {
        for i in 0..=grid_size {
            for j in 0..=grid_size {
                points.push((face, i, j));
            }
        }
    }
    points
}

#[test]
fn test_neighbors_are_the_points_within_reach() {
    let grid_size = 16;
    let globe_points = GlobePoints {
        grid_size,
        ..Default::default()
    };
    let all = all_grid_points(grid_size);
    for &from in &all {
        let mut neighbors = Vec::new();
        globe_points.for_each_neighbor(from, |to| neighbors.push(to));
        neighbors.sort();

        // Brute force: every point at a distance of at most STEPS on the cube, except ourselves
        // and our copies on other faces.
        let this = cubic(from, grid_size);
        let expected: Vec<GridPoint> = all
            .iter()
            .copied()
            .filter(|&to| {
                let other = cubic(to, grid_size);
                let dist2: i32 = (0..3).map(|k| (other[k] - this[k]).pow(2)).sum();
                dist2 > 0 && dist2 <= STEPS * STEPS
            })
            .collect();
        assert_eq!(neighbors, expected, "neighbors of {from:?}");
    }
}

#[test]
fn test_discount_edge() {
    let grid_size = 16;
    let mut globe_points = GlobePoints {
        grid_size,
        climbing_cost: 5.0,
        reduction_factor: 2.0,
        ..Default::default()
    };
    for p in all_grid_points(grid_size) {
        let [x, y, z] = cubic(p, grid_size);
        let point = GlobePoint {
            pos: Vec3::new(x as f32, y as f32, z as f32),
            water: false,
            penalty: 1.0,
        };
        globe_points.points.insert(p, point);
    }
    let edge_cost = |globe_points: &GlobePoints, from: GridPoint, to: GridPoint| {
        let mut cost = None;
        globe_points.for_each_edge(from, |n| n != to, |_, c| cost = Some(c));
        cost.unwrap()
    };

    let (a, b, c) = ((0, 8, 8), (0, 8, 10), (0, 9, 8));
    let full_cost = edge_cost(&globe_points, a, b);
    let other_cost = edge_cost(&globe_points, a, c);

    globe_points.discount_edge(a, b);
    assert_eq!(edge_cost(&globe_points, a, b), full_cost / 2.0);
    assert_eq!(edge_cost(&globe_points, b, a), full_cost / 2.0);
    assert_eq!(edge_cost(&globe_points, a, c), other_cost);

    // Discounting again changes nothing.
    globe_points.discount_edge(b, a);
    assert_eq!(edge_cost(&globe_points, a, b), full_cost / 2.0);
}
