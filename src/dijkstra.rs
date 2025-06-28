use bevy::math::{Vec2, Vec3};
use ordered_float::OrderedFloat;
use priority_queue::PriorityQueue;
use std::collections::{HashMap, VecDeque};
use std::time::Instant;

// face index, row, column
pub type GridPoint = (u32, u32, u32);

pub struct Edge {
    pub to: GridPoint,
    pub cost: f32,
    pub discounted: bool, // true if cost reduction has been applied
}

#[derive(Debug, Clone, Copy)]
pub struct GlobePoint {
    pub pos: Vec3,
    pub water: bool,
    pub penalty: f32,
}

#[derive(Default)]
pub struct GlobePoints {
    pub points: HashMap<GridPoint, GlobePoint>,
    pub graph: HashMap<GridPoint, Vec<Edge>>,
}

fn cubic(grid: GridPoint, size: u32) -> [i32; 3] {
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
    pub fn build_graph(&mut self, grid_size: u32, climbing_cost: f32) {
        let steps = 7i32;
        let size = grid_size as i32;
        let mut edges = 0;
        for (pts_done, (&grid, &p)) in self.points.iter().enumerate() {
            if pts_done % 10000 == 0 {
                println!(
                    "Building graph: {}/{}, edges {}",
                    pts_done,
                    self.points.len(),
                    edges
                );
            }
            for di in -steps..=steps {
                for dj in -steps..=steps {
                    if di == 0 && dj == 0 {
                        continue;
                    }
                    if di * di + dj * dj > steps * steps {
                        continue;
                    }
                    let neighbor = (
                        grid.0,
                        (grid.1 as i32 + di) as u32,
                        (grid.2 as i32 + dj) as u32,
                    );
                    if let Some(&q) = self.points.get(&neighbor) {
                        self.graph.entry(grid).or_default().push(Edge {
                            to: neighbor,
                            cost: cost(&p, &q, climbing_cost),
                            discounted: false,
                        });
                        edges += 1;
                    }
                }
            }
            if !(grid.1 as i32 >= steps
                && (grid.1 as i32) <= size - steps
                && grid.2 as i32 >= steps
                && (grid.2 as i32) <= size - steps)
            {
                // We're within `steps` of the edge of a face, and we want to step at most `steps` far,
                // so the next point is either on our face, within steps of us (checked above), or on a
                // different face, within `steps`` of the edge that face.
                let this = cubic(grid, grid_size);
                for big in 0..=grid_size as i32 {
                    for small in 0..=steps {
                        for which_edge in 0..4 {
                            for other_face in 0..6 {
                                if other_face == grid.0 {
                                    continue; // skip the same face
                                }
                                let neighbor = (
                                    other_face,
                                    match which_edge {
                                        0 => small,
                                        1 => grid_size as i32 - small,
                                        2 => big,
                                        3 => grid_size as i32 - big,
                                        _ => unreachable!(),
                                    } as u32,
                                    match which_edge {
                                        0 => big,
                                        1 => grid_size as i32 - big,
                                        2 => small,
                                        3 => grid_size as i32 - small,
                                        _ => unreachable!(),
                                    } as u32,
                                );
                                let other = cubic(neighbor, grid_size);
                                let dist2 = (other[0] - this[0]).pow(2)
                                    + (other[1] - this[1]).pow(2)
                                    + (other[2] - this[2]).pow(2);
                                if dist2 > steps * steps || dist2 == 0 {
                                    continue;
                                }
                                if let Some(q) = self.points.get(&neighbor) {
                                    self.graph.entry(grid).or_default().push(Edge {
                                        to: neighbor,
                                        cost: cost(&p, q, climbing_cost),
                                        discounted: false,
                                    });
                                    edges += 1;
                                }
                            }
                        }
                    }
                }
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

    let mut queue = PriorityQueue::new();
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
        if let Some(edges) = globe_points.graph.get(&current) {
            for edge in edges {
                if visited.contains_key(&edge.to) {
                    continue;
                }
                let new_neg_dist = current_dist - OrderedFloat(edge.cost);
                if queue.get_priority(&edge.to).is_none() {
                    queue.push(edge.to, new_neg_dist);
                    come_from.insert(edge.to, current);
                } else if new_neg_dist > *queue.get_priority(&edge.to).unwrap() {
                    queue.change_priority(&edge.to, new_neg_dist);
                    come_from.insert(edge.to, current);
                }
            }
        }
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

pub fn bidirectional_dijkstra(
    start: GridPoint,
    end: GridPoint,
    globe_points: &GlobePoints,
) -> Vec<GridPoint> {
    if start == end {
        return vec![start];
    }

    let start_time = Instant::now();

    #[derive(Hash, PartialEq, Eq, Clone)]
    struct NodeInfo {
        node: GridPoint,
        from_start: bool, // true if this node is from the start side of the search
        prev: Option<GridPoint>,
    }

    // Priority queues for both directions,
    // items are NodeInfo, priority is the distance
    // Using OrderedFloat to allow for priority queue with f32
    let mut queue = PriorityQueue::new();
    let mut visited: HashMap<GridPoint, NodeInfo> = HashMap::new();
    queue.push(
        NodeInfo {
            node: start,
            from_start: true,
            prev: None,
        },
        OrderedFloat(0.0),
    );

    queue.push(
        NodeInfo {
            node: end,
            from_start: false,
            prev: None,
        },
        OrderedFloat(0.0),
    );

    let mut meet_point: Option<GridPoint> = None;
    let mut meet_point_prev: Option<GridPoint> = None;

    while let Some((current_info, current_dist)) = queue.pop() {
        let current = current_info.node;
        if let Some(info) = visited.get(&current) {
            if info.from_start == current_info.from_start {
                continue; // Skip if this node is already visited in the same direction
            } else {
                // If we have visited this node from the other direction, we can stop
                meet_point = Some(current);
                meet_point_prev = current_info.prev;
                break;
            }
        }

        // Mark the node as visited in the opposite direction
        visited.insert(current, current_info.clone());

        // Process neighbors
        if let Some(edges) = globe_points.graph.get(&current) {
            for edge in edges {
                let neighbor_info = NodeInfo {
                    node: edge.to,
                    from_start: current_info.from_start,
                    prev: Some(current),
                };
                let new_neg_dist = current_dist - OrderedFloat(edge.cost);
                if !visited.contains_key(&edge.to)
                    || (visited[&edge.to].from_start != current_info.from_start
                        && new_neg_dist
                            > *queue
                                .get_priority(&neighbor_info)
                                .unwrap_or(&OrderedFloat(f32::NEG_INFINITY)))
                {
                    queue.push(neighbor_info, new_neg_dist);
                }
            }
        }
    }

    let mut path = VecDeque::new();
    if let Some(meet) = meet_point {
        let mut current = Some(meet);
        while current.is_some() {
            let current_node = current.unwrap();
            if let Some(info) = visited.get(&current_node) {
                path.push_front(current_node);
                current = info.prev;
            } else {
                break;
            }
        }
    }
    if let Some(prev) = meet_point_prev {
        let mut current = Some(prev);
        while current.is_some() {
            let current_node = current.unwrap();
            if let Some(info) = visited.get(&current_node) {
                path.push_back(current_node);
                current = info.prev;
            } else {
                break;
            }
        }
    }
    // path.make_contiguous();
    let path: Vec<GridPoint> = path.into_iter().collect();
    // if cfg!(debug_assertions) {
    println!(
        "Bidirectional path found in {} ms, {} steps",
        start_time.elapsed().as_millis(),
        path.len()
    );
    // }
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
        println!("face color: {:?}", color);
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
        println!("gridpoint: {:?}", gridpoint);
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
