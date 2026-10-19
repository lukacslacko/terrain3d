use bevy::math::{Vec2, Vec3};
use bevy::platform::collections::HashMap;
use bevy::platform::hash::FixedHasher;
use bevy::platform::time::Instant;
use ordered_float::OrderedFloat;
use priority_queue::PriorityQueue;
use std::collections::VecDeque;

// face index, row, column
pub type GridPoint = (u32, u32, u32);

/// How far an edge can reach, in grid steps.
pub const STEPS: i32 = 7;

#[derive(Debug, Clone, Copy)]
pub struct GlobePoint {
    pub pos: Vec3,
    pub water: bool,
    pub penalty: f32,
}

/// The points of the globe, and the graph on them that paths are searched in.
///
/// The edges aren't stored: there are ~60 million of them, which would take too long to build
/// and too much memory, especially on the web. Instead, they're computed on demand by
/// `for_each_edge`.
#[derive(Default)]
pub struct GlobePoints {
    pub points: HashMap<GridPoint, GlobePoint>,
    pub grid_size: u32,
    pub climbing_cost: f32,
    pub reduction_factor: f32, // cost reduction factor for discounted edges
    /// For each point, the neighbours it has a discounted edge to.
    pub discounted: HashMap<GridPoint, Vec<GridPoint>>,
}

pub fn cubic(grid: GridPoint, size: u32) -> [i32; 3] {
    let u = grid.1 as i32;
    let v = grid.2 as i32;
    let s = size as i32;
    match grid.0 {
        0 => [u, v, s],
        1 => [s - u, v, 0],
        2 => [s, v, s - u],
        3 => [0, v, u],
        4 => [u, s, s - v],
        5 => [u, 0, v],
        _ => unreachable!(),
    }
}

/// The inverse of `cubic`: the grid point of `face` at cube coordinates `c`.
fn grid_point(face: u32, c: [i32; 3], size: u32) -> GridPoint {
    let s = size as i32;
    let (u, v) = match face {
        0 => (c[0], c[1]),
        1 => (s - c[0], c[1]),
        2 => (s - c[2], c[1]),
        3 => (c[2], c[1]),
        4 => (c[0], s - c[2]),
        5 => (c[0], c[2]),
        _ => unreachable!(),
    };
    (face, u as u32, v as u32)
}

/// The plane `face` lies in, in cube coordinates: c[axis] == level.
fn face_plane(face: u32, size: u32) -> (usize, i32) {
    let s = size as i32;
    match face {
        0 => (2, s),
        1 => (2, 0),
        2 => (0, s),
        3 => (0, 0),
        4 => (1, s),
        5 => (1, 0),
        _ => unreachable!(),
    }
}

fn cost(p: &GlobePoint, q: &GlobePoint, climbing_cost: f32) -> f32 {
    let penalty = p.penalty.max(q.penalty);
    let p_height = p.pos.length();
    let q_height = q.pos.length();
    // Slightly prefer longer steps. 5^0.9 = 4.25, for example.
    let unscaled_cost =
        p.pos.distance(q.pos).powf(0.9) + climbing_cost * (p_height - q_height).abs();
    unscaled_cost * penalty
}

impl GlobePoints {
    /// Calls `f` with each neighbour of `from`: every other grid point within `STEPS` of it,
    /// measured in cube coordinates, on its own face or on a neighbouring one.
    pub fn for_each_neighbor(&self, from: GridPoint, mut f: impl FnMut(GridPoint)) {
        let size = self.grid_size as i32;
        let this = cubic(from, self.grid_size);
        for face in 0..6 {
            // Only faces whose plane is within `STEPS` of us can have neighbours: always our own
            // face, and the faces next to us if we're near an edge.
            let (axis, level) = face_plane(face, self.grid_size);
            let d = level - this[axis];
            let radius2 = STEPS * STEPS - d * d;
            if radius2 < 0 {
                continue;
            }
            let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
            for da in -STEPS..=STEPS {
                for db in -STEPS..=STEPS {
                    // Skip ourselves, and our copy on the other face if we're on an edge.
                    if da * da + db * db > radius2 || (d, da, db) == (0, 0, 0) {
                        continue;
                    }
                    let mut c = this;
                    c[axis] = level;
                    c[a] += da;
                    c[b] += db;
                    if (0..=size).contains(&c[a]) && (0..=size).contains(&c[b]) {
                        f(grid_point(face, c, self.grid_size));
                    }
                }
            }
        }
    }

    /// Calls `f` with each neighbour `to` of `from` for which `skip(to)` is false, and the cost of
    /// the edge to it. Computing the cost is relatively slow, especially on the web, so callers
    /// should skip the neighbours they don't need.
    pub fn for_each_edge(
        &self,
        from: GridPoint,
        skip: impl Fn(GridPoint) -> bool,
        mut f: impl FnMut(GridPoint, f32),
    ) {
        let Some(p) = self.points.get(&from) else {
            return;
        };
        let discounted = self.discounted.get(&from);
        self.for_each_neighbor(from, |to| {
            if skip(to) {
                return;
            }
            if let Some(q) = self.points.get(&to) {
                let mut cost = cost(p, q, self.climbing_cost);
                if discounted.is_some_and(|d| d.contains(&to)) {
                    cost /= self.reduction_factor;
                }
                f(to, cost);
            }
        });
    }

    /// Makes the edge between `a` and `b` cheaper in both directions. Discounting an edge again
    /// has no further effect.
    pub fn discount_edge(&mut self, a: GridPoint, b: GridPoint) {
        for (from, to) in [(a, b), (b, a)] {
            let neighbors = self.discounted.entry(from).or_default();
            if !neighbors.contains(&to) {
                neighbors.push(to);
            }
        }
    }
}

#[allow(dead_code)]
pub fn dijkstra(start: GridPoint, end: GridPoint, globe_points: &GlobePoints) -> Vec<GridPoint> {
    if start == end {
        return vec![start];
    }

    let start_time = Instant::now();

    let mut queue: PriorityQueue<GridPoint, OrderedFloat<f32>, FixedHasher> =
        PriorityQueue::with_default_hasher();
    let mut visited = HashMap::new();
    let mut come_from = HashMap::new();
    queue.push(start, OrderedFloat(0.0));
    while let Some((current, current_dist)) = queue.pop() {
        if visited.contains_key(&current) {
            continue;
        }
        visited.insert(current, current_dist);
        if current == end {
            break;
        }
        globe_points.for_each_edge(
            current,
            |to| visited.contains_key(&to),
            |to, cost| {
                let new_neg_dist = current_dist - OrderedFloat(cost);
                if queue.get_priority(&to).is_none() {
                    queue.push(to, new_neg_dist);
                    come_from.insert(to, current);
                } else if new_neg_dist > *queue.get_priority(&to).unwrap() {
                    queue.change_priority(&to, new_neg_dist);
                    come_from.insert(to, current);
                }
            },
        );
    }
    let mut path = Vec::new();
    let mut current = end;
    while let Some(&prev) = come_from.get(&current) {
        path.push(current);
        if prev == start {
            path.push(start);
            break;
        }
        current = prev;
    }

    if cfg!(debug_assertions) {
        println!(
            "Path found in {} ms, {} steps",
            start_time.elapsed().as_millis(),
            path.len()
        );
    }

    path
}


#[derive(Hash, PartialEq, Eq, Clone)]
struct NodeInfo {
    node: GridPoint,
    from_start: bool, // true if this node is from the start side of the search
}


#[derive(Hash, PartialEq, Eq, Clone, Debug)]
struct Priority {
    dist: OrderedFloat<f32>,
    prev: Option<GridPoint>,
}

impl Ord for Priority {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.dist.cmp(&other.dist)
    }
}

impl PartialOrd for Priority {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}


pub fn bidirectional_dijkstra(
    start: GridPoint,
    end: GridPoint,
    globe_points: &GlobePoints,
) -> Vec<GridPoint> {
    if start == end {
        return vec![start];
    }

    let start_time = Instant::now();


    // Priority queue for both directions
    let mut queue: PriorityQueue<NodeInfo, Priority, FixedHasher> =
        PriorityQueue::with_default_hasher();
    let mut visited_start: HashMap<GridPoint, Option<GridPoint>> = HashMap::new();
    let mut visited_end: HashMap<GridPoint, Option<GridPoint>> = HashMap::new();

    queue.push(
        NodeInfo {
            node: start,
            from_start: true,
        },
        Priority {
            dist: OrderedFloat(0.0),
            prev: None,
        },
    );
    queue.push(
        NodeInfo {
            node: end,
            from_start: false,
        },
        Priority {
            dist: OrderedFloat(0.0),
            prev: None,
        },
    );

    let mut meet_point: Option<GridPoint> = None;

    while let Some((current_info, current_prio)) = queue.pop() {
        let current = current_info.node;

        let this_visited: &mut HashMap<GridPoint, Option<GridPoint>>;
        let other_visited: &HashMap<GridPoint, Option<GridPoint>>;
        if current_info.from_start {
            this_visited = &mut visited_start;
            other_visited = &visited_end;
        } else {
            this_visited = &mut visited_end;
            other_visited = &visited_start;
        }

        if this_visited.contains_key(&current) {
            continue;
        }
        this_visited.insert(current, current_prio.prev);
        if other_visited.contains_key(&current) {
            meet_point = Some(current);
            break; // We found a meeting point
        }

        // Process neighbors
        globe_points.for_each_edge(
            current,
            |to| this_visited.contains_key(&to),
            |to, cost| {
                let neighbor_info = NodeInfo {
                    node: to,
                    from_start: current_info.from_start,
                };
                let new_neg_dist = current_prio.dist - OrderedFloat(cost);
                let new_prio = Priority {
                    dist: new_neg_dist,
                    prev: Some(current),
                };
                queue.push_increase(neighbor_info, new_prio);
            },
        );
    }

    let mut path = VecDeque::new();
    if let Some(meet) = meet_point {
        // Reconstruct the path from start to meet point
        let mut current = meet;
        while let Some(prev) = visited_start.get(&current) {
            path.push_front(current);
            if let Some(p) = prev {
                current = *p;
            } else {
                break; // Reached the start point
            }
        }
        // Reconstruct the path from end to meet point
        current = meet;
        while let Some(prev) = visited_end.get(&current) {
            if let Some(p) = prev {
                path.push_back(*p);
                current = *p;
            } else {
                break; // Reached the end point
            }
        }
    }

    let path: Vec<GridPoint> = path.into_iter().collect();
    if cfg!(debug_assertions) {
        println!(
            "Bidirectional path found in {} ms, {} steps",
            start_time.elapsed().as_millis(),
            path.len()
        );
    }
    path
}

pub fn get_closest_gridpoint(pos: Vec3, grid_size: u32) -> GridPoint {
    let idx = argmax(pos.abs());
    let sign_at_max = if pos[idx] < 0.0 { -1 } else { 1 };

    let face = match (idx, sign_at_max) {
        (0, 1) => 2,
        (0, -1) => 3,
        (1, 1) => 4,
        (1, -1) => 5,
        (2, 1) => 0,
        (2, -1) => 1,
        _ => unreachable!(),
    };

    if cfg!(debug_assertions) {
        let color = match face {
            0 => "red",
            1 => "green",
            2 => "blue",
            3 => "yellow",
            4 => "pink",
            5 => "gray",
            _ => unreachable!(),
        };
        println!("face color: {color:?}");
    }

    let norm_pos = pos * 0.5 / pos[idx];

    let xy = match face {
        0 => Vec2::new(norm_pos.x, norm_pos.y),
        1 => Vec2::new(norm_pos.x, -norm_pos.y),
        2 => Vec2::new(-norm_pos.z, norm_pos.y),
        3 => Vec2::new(-norm_pos.z, -norm_pos.y),
        4 => Vec2::new(norm_pos.x, -norm_pos.z),
        5 => Vec2::new(-norm_pos.x, -norm_pos.z),
        _ => unreachable!(),
    };
    let grid_x = ((xy.x + 0.5) * grid_size as f32).round() as u32;
    let grid_y = ((xy.y + 0.5) * grid_size as f32).round() as u32;

    let gridpoint = (face, grid_x, grid_y);
    if cfg!(debug_assertions) {
        println!("gridpoint: {gridpoint:?}");
    }
    gridpoint
}

fn argmax(v: Vec3) -> usize {
    let [x, y, z] = v.to_array();
    if x >= y && x >= z {
        0
    } else if y >= z {
        1
    } else {
        2
    }
}
