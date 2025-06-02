use crate::dijkstra::{GlobePoint, GlobePoints, GridPoint, dijkstra};
use crate::perlin::Perlin;
use crate::state::State;
use bevy::{
    asset::RenderAssetUsages,
    color::palettes::tailwind::*,
    input::{common_conditions::*, mouse::*},
    picking::pointer::PointerInteraction,
    prelude::*,
    render::mesh::{Mesh, Mesh3d},
};

pub fn init() {
    App::new()
        .add_plugins((DefaultPlugins, MeshPickingPlugin))
        .add_systems(Startup, startup)
        .add_systems(
            FixedUpdate,
            rotate_on_drag.run_if(input_pressed(MouseButton::Left)),
        )
        .add_systems(
            Update,
            on_mouse_right_click.run_if(input_just_pressed(MouseButton::Right)),
        )
        .insert_resource(State::default())
        .add_systems(Update, draw_pointer)
        .run();
}

#[derive(Component)]
struct Globe;

#[derive(Component)]
struct MainCamera;

#[derive(Resource)]
struct CityMaterialHandle {
    handle: Handle<StandardMaterial>,
}

#[derive(Resource)]
struct PathMaterialHandle {
    handle: Handle<StandardMaterial>,
}

#[derive(Resource)]
struct CityMeshHandle {
    handle: Handle<Mesh>,
}

#[derive(Resource)]
struct PathMeshHandle {
    handle: Handle<Mesh>,
}

fn make_globe(config: &crate::state::Config) -> (GlobePoints, Mesh) {
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    let mut globe_points = GlobePoints::default();
    let grid_size = config.grid_size;

    let m = grid_size + 1;

    let perlin = Perlin {
        config: config.perlin_config,
    };

    println!("Making globe");

    for face in 0..6 {
        println!("Making face {}", face);
        for i in 0..m {
            for j in 0..m {
                let u = i as f32 / grid_size as f32;
                let v = j as f32 / grid_size as f32;
                let sphere = |u: f32, v: f32| {
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
                let surface = |u, v| {
                    let (nx, ny, nz) = sphere(u, v);
                    let nr = 5.0;
                    // let color = [u, v, (1 + face) as f32 / 8.0, 1.0];
                    let noise = perlin.noise(nx, ny, nz) * 1.0;
                    (
                        nr + noise,
                        [nx * (nr + noise), ny * (nr + noise), nz * (nr + noise)],
                    )
                };
                let normvec = |u, v| {
                    let (_, p) = surface(u, v);
                    let (_, q) = surface(u + 0.001, v);
                    let (_, r) = surface(u, v + 0.001);
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
                let (noise, pos) = surface(u, v);

                let sea_level = 5.0;

                let snow = 0.5;
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
                    normals.push(normvec(u, v));
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
    globe_points.build_graph(grid_size);

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

fn startup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut state: ResMut<State>,
) {
    let (globe_points, globe_mesh) = make_globe(&state.config);
    state.globe_points = globe_points;
    let globe_mesh_handle = meshes.add(globe_mesh);

    let globe_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.0,
        metallic: 0.0,
        ..default()
    });

    commands.insert_resource(CityMaterialHandle {
        handle: materials.add(StandardMaterial {
            base_color: state.config.city_marker_color,
            perceptual_roughness: 0.0,
            metallic: 0.0,
            ..default()
        }),
    });

    let path_material_handle = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(165, 165, 165),
        perceptual_roughness: 0.0,
        metallic: 0.0,
        ..default()
    });
    commands.insert_resource(PathMaterialHandle {
        handle: path_material_handle.clone(),
    });

    let path_mesh_handle = meshes.add(Sphere { radius: 0.1 });
    commands.insert_resource(PathMeshHandle {
        handle: path_mesh_handle.clone(),
    });

    let city_mesh_handle = meshes.add(Cuboid {
        half_size: Vec3::splat(state.config.city_marker_size),
    });
    commands.insert_resource(CityMeshHandle {
        handle: city_mesh_handle.clone(),
    });

    println!("Spawning globe.");
    commands.spawn((
        Mesh3d(globe_mesh_handle),
        MeshMaterial3d(globe_material.clone()),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Globe,
    ));
    println!("Globe spawned.");

    commands.spawn((
        PointLight {
            shadows_enabled: true,
            intensity: 10_000_000.0,
            range: 100.0,
            // shadow_depth_bias: 0.2,
            ..default()
        },
        Transform::from_xyz(0.0, 5.0, 20.0),
    ));
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            intensity: 10_000_000.0,
            range: 100.0,
            // shadow_depth_bias: 0.2,
            ..default()
        },
        Transform::from_xyz(0.0, -5.0, -20.0),
    ));

    commands.spawn((
        Camera3d::default(),
        MainCamera,
        Transform::from_xyz(0.0, 11.0, 12.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Z),
    ));
}

fn create_path(
    commands: &mut Commands<'_, '_>,
    state: &mut State,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    start: GridPoint,
    end: GridPoint,
) {
    println!("Dijkstra");
    let path = dijkstra(start, end, &state.globe_points);

    let reduction_factor = state.config.reduction_factor;
    // Apply cost reduction to edges in the path (only once per edge)
    for w in path.windows(2) {
        let (from, to) = (w[1], w[0]);
        if let Some(edges) = state.globe_points.graph.get_mut(&from) {
            for edge in edges.iter_mut() {
                if edge.to == to && !edge.discounted {
                    edge.cost /= reduction_factor;
                    edge.discounted = true;
                }
            }
        }
        if let Some(edges) = state.globe_points.graph.get_mut(&to) {
            for edge in edges.iter_mut() {
                if edge.to == from && !edge.discounted {
                    edge.cost /= reduction_factor;
                    edge.discounted = true;
                }
            }
        }
    }


    println!("Dijkstra done, path length: {}", path.len());

    for point in path.iter() {
        let pos = &state.globe_points.points[point];
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(pos.pos[0], pos.pos[1], pos.pos[2]),
            PointerInteraction::default(),
        ));
    }
}

fn rotate_on_drag(
    mut motion_event_reader: EventReader<MouseMotion>,
    mut camera_transform: Query<(&mut Transform, &MainCamera)>,
) {
    let (dx, dy) = motion_event_reader
        .read()
        .fold((0.0, 0.0), |(x, y), event| {
            (x - event.delta.x * 0.005, y - event.delta.y * 0.005)
        });
    let (mut transform, _camera) = camera_transform.single_mut().unwrap();

    let origin = Vec3::ZERO;
    let direction = transform.translation - origin;
    let radius = 15.0;

    // Step 1: Get camera's local axes
    let right = transform.right().as_vec3(); // local right
    let up = transform.up().as_vec3(); // local up

    // Step 2: Apply rotations
    let rot_horizontal = Quat::from_axis_angle(up, dx);
    let rot_vertical = Quat::from_axis_angle(right, dy);
    let rotation = rot_horizontal * rot_vertical;

    let new_direction = rotation * direction;

    // Step 3: Update position and look at the origin
    transform.translation = origin + new_direction.normalize() * radius;
    transform.look_at(origin, up);
}

fn draw_pointer(pointers: Query<&PointerInteraction>, mut gizmos: Gizmos) {
    for point in pointers
        .iter()
        .filter_map(|interaction| interaction.get_nearest_hit())
        .filter_map(|(_entity, hit)| hit.position)
    {
        gizmos.sphere(point, 0.05, RED_500);
    }
}

fn on_mouse_right_click(
    pointers: Query<&PointerInteraction>,
    mut state: ResMut<State>,
    mut commands: Commands,
    city_mesh: Res<CityMeshHandle>,
    city_material: Res<CityMaterialHandle>,
    path_mesh: Res<PathMeshHandle>,
    path_material: Res<PathMaterialHandle>,
) {
    for point in pointers
        .iter()
        .filter_map(|interaction| interaction.get_nearest_hit())
        .filter_map(|(_entity, hit)| hit.position)
    {
        let gridpoint = get_closest_gridpoint(point, state.config.grid_size);
        if let Some(&globe_point) = state.globe_points.points.get(&gridpoint) {
            if globe_point.water {
                println!("Can't place city on water: {:?}", gridpoint);
                continue; // Skip water points
            }

            // Check if a city already exists nearby
            if state.cities.iter().any(|city| {
                let city_pos = state.globe_points.points[city].pos;
                let distance = (city_pos - globe_point.pos).length();
                distance < state.config.min_city_distance // Adjust the threshold as needed
            }) {
                println!("City already exists near gridpoint: {:?}", gridpoint);
                continue; // Skip if a city already exists nearby
            }

            state.cities.push(gridpoint);
            if cfg!(debug_assertions) {
                println!("Placing city at globe_point: {:?}", globe_point);
                println!("Cities: {:?}", state.cities);
            }

            // if the number of cities is even, create a path between the two last cities
            if state.cities.len() % 2 == 0 {
                let last_city = *state.cities.last().unwrap();
                let second_last_city = *state.cities.get(state.cities.len() - 2).unwrap();
                create_path(
                    &mut commands,
                    &mut state,
                    path_mesh.handle.clone(),
                    path_material.handle.clone(),
                    second_last_city,
                    last_city,
                );
            }

            commands.spawn((
                Mesh3d(city_mesh.handle.clone()),
                MeshMaterial3d(city_material.handle.clone()),
                Transform::from_xyz(globe_point.pos[0], globe_point.pos[1], globe_point.pos[2])
                    .looking_at(Vec3::ZERO, Vec3::Z),
            ));
        } else {
            println!("No GlobePoint found for gridpoint: {:?}", gridpoint);
        }
    }
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

fn get_closest_gridpoint(pos: Vec3, grid_size: u32) -> GridPoint {
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
