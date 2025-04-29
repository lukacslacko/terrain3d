use crate::perlin::Perlin;
use bevy::{
    asset::RenderAssetUsages,
    input::{common_conditions::*, mouse::*},
    prelude::*,
};

pub fn init() {
    App::new()
        .add_plugins((DefaultPlugins, MeshPickingPlugin))
        .add_systems(Startup, startup)
        .add_systems(
            Update,
            rotate_on_drag.run_if(input_pressed(MouseButton::Left)),
        )
        .run();
}

#[derive(Component)]
struct Globe;

fn make_globe(n: u32) -> Mesh {
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    let m = n + 1;

    let perlin = Perlin {
        seed: 0,
        frequency: 1.0,
        lacunarity: 2.13,
        persistence: 0.5,
        octaves: 4,
    };

    for face in 0..6 {
        for i in 0..m {
            for j in 0..m {
                let u = i as f32 / n as f32;
                let v = j as f32 / n as f32;
                let x = u - 0.5;
                let y = v - 0.5;
                let z = 0.5;
                let r = (x * x + y * y + z * z).sqrt();
                let (nx, ny, nz) = match face {
                    0 => (x / r, y / r, z / r),
                    1 => (-x / r, y / r, -z / r),
                    2 => (z / r, y / r, -x / r),
                    3 => (-z / r, y / r, x / r),
                    4 => (x / r, z / r, -y / r),
                    5 => (x / r, -z / r, y / r),
                    _ => unreachable!(),
                };
                let nr = 5.0;
                let pos = [nx * nr, ny * nr, nz * nr];
                // let color = [u, v, (1 + face) as f32 / 8.0, 1.0];
                let noise = perlin.noise(nx, ny, nz) * 5.0;
                use std::f32::consts::PI;
                use std::ops::Add;
                trait Scalable where Self: Add<f32, Output = f32> + Copy {
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
                positions.push(pos);
                colors.push(color);
                normals.push([nx, ny, nz]);
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
) {
    let cube = meshes.add(make_globe(100));
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.0,
        metallic: 0.0,
        ..default()
    });

    commands.spawn((
        Mesh3d(cube),
        MeshMaterial3d(material),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Globe,
    ));

    commands.spawn((
        PointLight {
            shadows_enabled: true,
            intensity: 10_000_000.0,
            range: 100.0,
            shadow_depth_bias: 0.2,
            ..default()
        },
        Transform::from_xyz(0.0, 5.0, 20.0),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 11.0, 12.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Z),
    ));
}

fn rotate_on_drag(
    mut motion_event_reader: EventReader<MouseMotion>,
    mut transform: Single<(&mut Transform, &Globe)>,
) {
    for event in motion_event_reader.read() {
        transform.0.rotate_x(-event.delta.y * 0.005);
        transform.0.rotate_y(-event.delta.x * 0.005);
    }
}
