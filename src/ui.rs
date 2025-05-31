use crate::dijkstra::GlobePoints;
use crate::dijkstra::dijkstra;
use crate::perlin::Perlin;
use crate::state::State;
use bevy::{
    asset::RenderAssetUsages,
    color::palettes::tailwind::*,
    input::{common_conditions::*, mouse::*},
    picking::pointer::PointerInteraction,
    prelude::*,
};

pub fn init() {
    App::new()
        .add_plugins((DefaultPlugins, MeshPickingPlugin))
        .add_systems(Startup, startup)
        .add_systems(
            FixedUpdate,
            rotate_on_drag.run_if(input_pressed(MouseButton::Left)),
        )
        .insert_resource(State::default())
        .add_systems(Update, draw_pointer)
        .run();
}

#[derive(Component)]
struct Globe;

#[derive(Component)]
struct MainCamera;

fn make_globe(n: u32, globe_points: &mut GlobePoints) -> Mesh {
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    globe_points.size = n;

    let m = n + 1;

    let perlin = Perlin {
        seed: 5,
        frequency: 2.0,
        lacunarity: 1.57,
        persistence: 0.5,
        octaves: 6,
    };

    println!("Making globe");

    for face in 0..6 {
        println!("Making face {}", face);
        for i in 0..m {
            for j in 0..m {
                let u = i as f32 / n as f32;
                let v = j as f32 / n as f32;
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

                globe_points.points.insert((face, i, j), pos);

                /*
                use std::f32::consts::PI;
                use std::ops::Add;
                trait Scalable
                where
                    Self: Add<f32, Output = f32> + Copy,
                {
                    fn scale(&self) -> f32 {
                        (*self + 1.0) / 2.0
                    }
                }
                impl Scalable for f32 {}
                let color = [
                    noise.sin().scale(),
                    (noise + 2.0 * PI / 3.0).sin().scale(),
                    (noise + 4.0 * PI / 3.0).sin().scale(),
                    1.0,
                ];
                */
                let sea_level = 5.0;
                let snow = 0.5;
                let height = noise - sea_level;
                let color = if height > snow {
                    [1.0, 1.0, 1.0, 1.0]
                } else {
                    let v = height / snow;
                    [v / 2.5, (1.5 - v) / 3.0, v / 5.0, 1.0]
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
            }
        }
        for i in 0..n {
            for j in 0..n {
                let a = i + m * j + face * m * m;
                let b = a + 1;
                let c = a + m + 1;
                let d = a + m;
                indices.extend([b, a, c, d, c, a]);
            }
        }
    }

    println!("Building graph.");
    globe_points.build_graph();

    println!("Making mesh.");

    let mut mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));
    mesh
}

fn startup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut state: ResMut<State>,
) {
    let mesh = make_globe(128, &mut state.globe_points);
    let cube = meshes.add(mesh);
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.0,
        metallic: 0.0,
        ..default()
    });

    println!("Spawning globe.");

    commands.spawn((
        Mesh3d(cube),
        MeshMaterial3d(material.clone()),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Globe,
    ));

    println!("Globe spawned.");

    println!("Dijkstra");
    let path = dijkstra(
        (2, 0, 0), (3, 0, 0),
        &state.globe_points,
    );
    println!("Dijkstra done, path length: {}", path.len());

    for (_, point) in path.iter().enumerate() {
        let pos = state.globe_points.points[point];
        let s = meshes.add(Sphere::new(0.1));
        commands.spawn((
            Mesh3d(s),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(pos[0], pos[1], pos[2]),
            PointerInteraction::default(),
        ));
    }


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
        let up = transform.up().as_vec3();      // local up
    
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
