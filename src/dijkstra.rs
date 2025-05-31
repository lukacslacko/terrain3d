use ordered_float::OrderedFloat;
use priority_queue::PriorityQueue;
use std::collections::{HashMap, HashSet};

pub type GridPoint = (u32, u32, u32);

pub struct Edge {
    pub to: GridPoint,
    pub cost: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct GlobePoint {
    pub pos: [f32; 3],
    pub water: bool,
    pub snow: bool,
}

#[derive(Default)]
pub struct GlobePoints {
    pub size: u32,
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

fn cost(p: &GlobePoint, q: &GlobePoint) -> f32 {
    let dx = p.pos[0] - q.pos[0];
    let dy = p.pos[1] - q.pos[1];
    let dz = p.pos[2] - q.pos[2];
    let penalty = if p.water || q.water {
        5.0
    } else {
        if p.snow || q.snow { 3.0 } else { 1.0 }
    };
    (dx * dx + dy * dy + dz * dz).sqrt() * penalty
}

impl GlobePoints {
    pub fn build_graph(&mut self) {
        let steps = 3i32;
        let size = self.size as i32;
        let mut pts_done = 0;
        let mut edges = 0;
        for (&grid, &p) in &self.points {
            if pts_done % 1000 == 0 {
                println!(
                    "Building graph: {}/{}, edges {}",
                    pts_done,
                    self.points.len(),
                    edges
                );
            }
            pts_done += 1;
            if grid.1 as i32 >= steps
                && (grid.1 as i32) < size - steps
                && grid.2 as i32 >= steps
                && (grid.2 as i32) < size - steps
            {
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
                                cost: cost(&p, &q),
                            });
                            edges += 1;
                        }
                    }
                }
            } else {
                for (&neighbor, &q) in &self.points {
                    let other = cubic(neighbor, self.size);
                    let this = cubic(grid, self.size);
                    let dist2 = (other[0] - this[0]).pow(2)
                        + (other[1] - this[1]).pow(2)
                        + (other[2] - this[2]).pow(2);
                    if dist2 > steps * steps || dist2 == 0 {
                        continue;
                    }
                    self.graph.entry(grid).or_default().push(Edge {
                        to: neighbor,
                        cost: cost(&p, &q),
                    });
                    edges += 1;
                }
            }
        }
    }
}

pub fn dijkstra(start: GridPoint, end: GridPoint, globe_points: &GlobePoints) -> Vec<GridPoint> {
    let mut queue = PriorityQueue::new();
    let mut visited = HashSet::new();
    let mut dist = HashMap::new();
    let mut come_from = HashMap::new();
    queue.push(start, OrderedFloat(0.0));
    while let Some((current, current_dist)) = queue.pop() {
        if visited.contains(&current) {
            continue;
        }
        visited.insert(current);
        if current == end {
            break;
        }
        if let Some(edges) = globe_points.graph.get(&current) {
            for edge in edges {
                if visited.contains(&edge.to) {
                    continue;
                }
                let new_dist = current_dist - OrderedFloat(edge.cost);
                if !dist.contains_key(&edge.to) || new_dist > dist[&edge.to] {
                    dist.insert(edge.to, new_dist);
                    come_from.insert(edge.to, current);
                    queue.push(edge.to, new_dist);
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
    path
}
