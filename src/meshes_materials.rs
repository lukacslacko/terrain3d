use crate::dijkstra::{GlobePoint, GlobePoints};
use crate::perlin::Perlin;
use bevy::{asset::RenderAssetUsages, prelude::*, render::mesh::Mesh};

#[derive(Resource)]
pub struct Materials {
    pub city: Handle<StandardMaterial>,
    pub selected_city: Handle<StandardMaterial>,
    pub highlighted_city: Handle<StandardMaterial>,
    pub train: Handle<StandardMaterial>,
}

impl Materials {
    pub fn new(material_assets: &mut Assets<StandardMaterial>) -> Self {
        let city = material_assets.add(StandardMaterial {
            base_color: Color::srgb_u8(124, 144, 255),
            perceptual_roughness: 0.0,
            metallic: 0.0,
            ..default()
        });
        let selected_city = material_assets.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.0,
            metallic: 0.0,
            ..default()
        });
        let highlighted_city = material_assets.add(StandardMaterial {
            base_color: Color::srgb_u8(190, 200, 255),
            perceptual_roughness: 0.0,
            metallic: 0.0,
            ..default()
        });
        let train: Handle<StandardMaterial> = material_assets.add(StandardMaterial {
            base_color: Color::srgb_u8(240, 240, 240),
            perceptual_roughness: 0.0,
            metallic: 0.0,
            ..default()
        });

        Self {
            city,
            selected_city,
            highlighted_city,
            train,
        }
    }
}

#[derive(Resource)]
pub struct Meshes {
    pub city: Handle<Mesh>,
    pub path: Handle<Mesh>,
    pub train: Handle<Mesh>,
}

impl Meshes {
    pub fn new(mesh_assets: &mut Assets<Mesh>) -> Self {
        let city = mesh_assets.add(Cuboid {
            half_size: Vec3::splat(0.1),
        });
        let path = mesh_assets.add(Cuboid::default());
        let train: Handle<Mesh> = mesh_assets.add(Cylinder {
            radius: 0.05,
            half_height: 0.1,
        });

        Self { city, path , train }
    }
}

pub fn make_globe(config: &crate::state::Config) -> (GlobePoints, Mesh) {
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    let mut globe_points = GlobePoints::default();
    let grid_size = config.grid_size;
    let m = grid_size + 1;
    let sea_level = config.sea_level;
    let snow = config.snow_level;

    let perlin = Perlin {
        config: config.perlin_config,
    };

    println!("Making globe");

    let sphere = |u: f32, v: f32, face: u32| {
        let x = u - 0.5;
        let y = v - 0.5;
        let z = 0.5;
        let r = (x * x + y * y + z * z).sqrt();
        match face {
            0 => (x / r, y / r, z / r),
            1 => (-x / r, y / r, -z / r),
            2 => (z / r, y / r, -x / r),
            3 => (-z / r, y / r, x / r),
            4 => (x / r, z / r, -y / r),
            5 => (x / r, -z / r, y / r),
            _ => unreachable!(),
        }
    };

    let surface = |u, v, face: u32| {
        let (nx, ny, nz) = sphere(u, v, face);
        let nr = 5.0;
        // let color = [u, v, (1 + face) as f32 / 8.0, 1.0];
        let noise = perlin.noise(nx, ny, nz) * 1.0;
        (
            nr + noise,
            [nx * (nr + noise), ny * (nr + noise), nz * (nr + noise)],
        )
    };

    let normvec = |u, v, face| {
        let (_, p) = surface(u, v, face);
        let (_, q) = surface(u + 0.001, v, face);
        let (_, r) = surface(u, v + 0.001, face);
        let a = [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
        let b = [p[0] - r[0], p[1] - r[1], p[2] - r[2]];
        let n = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let r = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        [n[0] / r, n[1] / r, n[2] / r]
    };

    for face in 0..6 {
        println!("Making face {}", face);
        for i in 0..m {
            for j in 0..m {
                let u = i as f32 / grid_size as f32;
                let v = j as f32 / grid_size as f32;
                
                let (noise, pos) = surface(u, v, face);

                let height = noise - sea_level;
                let color = if cfg!(debug_assertions) {
                    // for debugging, use different colors for each face
                    match face {
                        0 => [1.0, 0.0, 0.0, 1.0], // red
                        1 => [0.0, 1.0, 0.0, 1.0], // green
                        2 => [0.0, 0.0, 1.0, 1.0], // blue
                        3 => [1.0, 1.0, 0.0, 1.0], // yellow
                        4 => [1.0, 0.4, 0.4, 1.0], // pink
                        _ => [0.5, 0.5, 0.5, 1.0], // gray
                    }
                } else if height > snow {
                    [1.0, 1.0, 1.0, 1.0] // white for snow
                } else {
                    let v = height / snow;
                    [v / 2.5, (1.5 - v) / 3.0, v / 5.0, 1.0] // gradient color for land
                };
                if height > 0.0 {
                    positions.push(pos);
                    colors.push(color);
                    normals.push(normvec(u, v, face));
                } else {
                    let normpos = [pos[0] / noise, pos[1] / noise, pos[2] / noise];
                    positions.push([
                        normpos[0] * sea_level,
                        normpos[1] * sea_level,
                        normpos[2] * sea_level,
                    ]);
                    let blueify = |c: [f32; 4], depth: f32| {
                        let depth_ratio = (depth / 0.02).clamp(0.0, 1.0).sqrt();
                        [
                            c[0] * (1.0 - depth_ratio),
                            c[1] * (1.0 - depth_ratio),
                            c[2] * (1.0 - depth_ratio) + depth_ratio,
                            c[3],
                        ]
                    };
                    colors.push(blueify(color, sea_level - noise));
                    normals.push(normpos);
                }
                let render_pos =
                    pos.map(|x| (sea_level + height.max(0.0)) / (sea_level + height) * x);
                globe_points.points.insert(
                    (face, i, j),
                    GlobePoint {
                        pos: Vec3::from(render_pos),
                        water: height <= 0.0,
                        penalty: if height <= 0.0 {
                            config.water_penalty
                        } else if height >= snow {
                            config.snow_penalty
                        } else {
                            1.0
                        },
                    },
                );
            }
        }
        for i in 0..grid_size {
            for j in 0..grid_size {
                let a = i + m * j + face * m * m;
                let b = a + 1;
                let c = a + m + 1;
                let d = a + m;
                indices.extend([b, a, c, d, c, a]);
            }
        }
    }

    println!("Building graph.");
    globe_points.build_graph(grid_size, config.climbing_cost);

    println!("Making mesh.");
    let mut mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));
    (globe_points, mesh)
}
