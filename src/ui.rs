use bevy::{asset::RenderAssetUsages, prelude::*};

pub fn init() {
    App::new()
        .add_plugins((DefaultPlugins, MeshPickingPlugin))
        .add_systems(Startup, startup)
        .run();
}

#[derive(Component)]
struct MyCube;

fn make_cube() -> Mesh {
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    for i in 0..=10 {
        for j in 0..=10 {
            let u = i as f32 / 2.0;
            let v = j as f32 / 2.0;
            let pos = [u, v, 5.0];
            let color = [u, v, 0.5, 1.0];
            positions.push(pos);
            colors.push(color);
            normals.push([0.0, 0.0, 1.0]);
            uvs.push([u, v]);
        }
    }

    for i in 0..10 {
        for j in 0..10 {
            let a = i + 11 * j;
            let b = a + 1;
            let c = a + 12;
            let d = a + 11;
            indices.extend([b, a, c, d, c, a]);
        }
    }

    let mut mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));
    mesh
}

fn startup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cube = meshes.add(make_cube());

    commands.spawn((
        Mesh3d(cube),
        MeshMaterial3d(materials.add(Color::WHITE).clone()),
        Transform::from_xyz(0.0, 0.0, 0.0),
        MyCube,
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
        Transform::from_xyz(10.0, 11.0, 12.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Z),
    ));
}
