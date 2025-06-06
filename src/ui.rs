use crate::dijkstra::{GlobePoint, GlobePoints, GridPoint, dijkstra, get_closest_gridpoint};
use crate::meshes_materials::{Materials, Meshes, make_globe};
use crate::state::State;
use bevy::{
    color::palettes::tailwind::*,
    input::{common_conditions::*, mouse::*},
    picking::pointer::PointerInteraction,
    prelude::*,
    render::mesh::{Mesh, Mesh3d},
};
use crossbeam_channel::{Receiver, bounded};
use std::thread;

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
        .add_systems(
            Update,
            on_mouse_left_click.run_if(input_just_pressed(MouseButton::Left)),
        )
        .add_systems(Update, highlight_city)
        .insert_resource(State::default())
        .insert_resource(SelectedCity::default())
        .add_systems(Update, draw_pointer)
        .add_systems(Update, try_getting_globe)
        .run();
}

#[derive(Component)]
struct Globe;

#[derive(Component)]
struct MainCamera;

#[derive(Component)]
struct City;

#[derive(Resource, Default)]
struct SelectedCity(Option<Entity>);

#[derive(Component)]
struct Position {
    gridpoint: GridPoint,
    globe_point: GlobePoint,
}

#[derive(Resource)]
struct GlobeReceiver {
    receiver: Receiver<(GlobePoints, Mesh)>,
}

fn try_getting_globe(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut state: ResMut<State>,
    globe_receiver: Res<GlobeReceiver>,
) {
    if let Ok((globe_points, globe_mesh)) = globe_receiver.receiver.try_recv() {
        println!("Received globe points and mesh.");
        state.globe_points = globe_points;
        let globe_mesh_handle = meshes.add(globe_mesh);
        let globe_material = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.0,
            metallic: 0.0,
            ..default()
        });

        println!("Spawning globe.");
        commands.spawn((
            Mesh3d(globe_mesh_handle),
            MeshMaterial3d(globe_material.clone()),
            Transform::from_xyz(0.0, 0.0, 0.0),
            Globe,
        ));
        println!("Globe spawned.");
    }
}

fn startup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    state: Res<State>,
) {
    let (tx, rx) = bounded(1);
    let config_for_make_globe = state.config.clone();
    thread::spawn(move || {
        let (globe_points, globe_mesh) = make_globe(&config_for_make_globe);
        tx.send((globe_points, globe_mesh)).unwrap();
    });

    commands.insert_resource(GlobeReceiver { receiver: rx });
    commands.insert_resource(Materials::create(&mut materials));
    commands.insert_resource(Meshes::create(&mut meshes));

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
        Camera3d::default(),
        MainCamera,
        Transform::from_xyz(0.0, 11.0, 12.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Z),
    ));
}

fn create_path(
    commands: &mut Commands<'_, '_>,
    state: &mut State,
    material: Handle<StandardMaterial>,
    cylinder: Handle<Mesh>,
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

    for line in path.windows(2) {
        let (from, to) = (line[0], line[1]);
        if let Some(from_point) = state.globe_points.points.get(&from) {
            if let Some(to_point) = state.globe_points.points.get(&to) {
                // Create cylinder mesh connecting from_point to to_point.
                let direction = to_point.pos - from_point.pos;
                let length = direction.length();
                let mid_point = (from_point.pos + to_point.pos) / 2.0;
                let rotation = Quat::from_rotation_arc(Vec3::Y, direction.normalize());
                commands.spawn((
                    Mesh3d(cylinder.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::from_scale(Vec3 {
                        x: 1.0,
                        y: length,
                        z: 1.0,
                    })
                    .with_translation(mid_point)
                    .with_rotation(rotation),
                    PointerInteraction::default(),
                ));
            }
        }
    }
}

fn rotate_on_drag(
    mut motion_event_reader: EventReader<MouseMotion>,
    mut camera_transform: Query<(&mut Transform, &MainCamera)>,
    mut lights_transform: Query<(&mut Transform, &PointLight), Without<MainCamera>>,
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

    for mut light_transform in lights_transform.iter_mut() {
        let above_camera = transform.translation + transform.up() * 5.0;
        light_transform.0.translation = above_camera;
    }
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
    state: Res<State>,
    mut commands: Commands,
    cities: Query<(Entity, &Position), With<City>>,
    meshes: Res<Meshes>,
    materials: Res<Materials>,
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

            if cities.iter().any(|(_, pos)| {
                (pos.globe_point.pos - globe_point.pos).length() < state.config.min_city_distance
            }) {
                println!("City already exists near gridpoint: {:?}", gridpoint);
                continue; // Skip if a city already exists at this point
            }

            commands.spawn((
                City,
                Position {
                    gridpoint,
                    globe_point,
                },
                Mesh3d(meshes.city.clone()),
                MeshMaterial3d(materials.city.clone()),
                Transform::from_xyz(globe_point.pos[0], globe_point.pos[1], globe_point.pos[2])
                    .looking_at(Vec3::ZERO, Vec3::Z),
            ));
        } else {
            println!("No GlobePoint found for gridpoint: {:?}", gridpoint);
        }
    }
}

fn on_mouse_left_click(
    pointers: Query<&PointerInteraction>,
    mut state: ResMut<State>,
    mut commands: Commands,
    cities: Query<(Entity, &Position), With<City>>,
    mut selected: ResMut<SelectedCity>,
    meshes: Res<Meshes>,
    materials: Res<Materials>,
) {
    for clicked_point in pointers
        .iter()
        .filter_map(|interaction| interaction.get_nearest_hit())
        .filter_map(|(_entity, hit)| hit.position)
    {
        // Check if a city exists near the clicked point
        if let Some((clicked_city, _pos)) = cities.iter().find(|(_, pos)| {
            (pos.globe_point.pos - clicked_point).length() < state.config.min_city_distance / 2.0
        }) {
            match selected.0 {
                None => {
                    // Select the city
                    selected.0 = Some(clicked_city);
                    commands
                        .entity(clicked_city)
                        .insert(MeshMaterial3d(materials.selected_city.clone()));
                    println!("Selected city {:?}", clicked_city);
                }
                Some(prev_selected) => {
                    if prev_selected != clicked_city {
                        // Connect the cities
                        println!("Connecting {:?} and {:?}", prev_selected, clicked_city);
                        create_path(
                            &mut commands,
                            &mut state,
                            materials.path.clone(),
                            meshes.path.clone(),
                            cities.get(prev_selected).unwrap().1.gridpoint,
                            cities.get(clicked_city).unwrap().1.gridpoint,
                        );
                    }
                    // Clear selection
                    selected.0 = None;
                    commands
                        .entity(prev_selected)
                        .insert(MeshMaterial3d(materials.city.clone()));
                }
            }
        } else {
            // no city near clicked point
        }
    }
}

fn highlight_city(
    state: Res<State>,
    pointers: Query<&PointerInteraction>,
    mut commands: Commands,
    cities: Query<(Entity, &Position), With<City>>,
    materials: Res<Materials>,
    selected: Res<SelectedCity>,
) {
    for point in pointers
        .iter()
        .filter_map(|interaction| interaction.get_nearest_hit())
        .filter_map(|(_entity, hit)| hit.position)
    {
        if let Some((entity, _pos)) = cities.iter().find(|(_, pos)| {
            (pos.globe_point.pos - point).length() < state.config.min_city_distance / 2.0
        }) {
            if selected.0.is_none() || selected.0.unwrap() != entity {
                commands
                    .entity(entity)
                    .insert(MeshMaterial3d(materials.highlighted_city.clone()));
            }
        } else {
            // Remove highlight from all cities
            for (entity, _) in cities.iter() {
                if selected.0.is_some() && selected.0.unwrap() == entity {
                    continue; // Skip the selected city
                }
                commands
                    .entity(entity)
                    .insert(MeshMaterial3d(materials.city.clone()));
            }
        }
    }
}
