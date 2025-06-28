use std::sync::{Arc, RwLock};
use std::thread;

use crate::dijkstra::{GlobePoint, GlobePoints, GridPoint, dijkstra, get_closest_gridpoint};
use crate::meshes_materials::{Materials, Meshes, make_globe};
use crate::state::{Rail, RailInfo, State};
use crate::train::{SelectedTrain, Train};

use bevy::{
    color::palettes::tailwind::*,
    input::{common_conditions::*, mouse::*},
    picking::pointer::PointerInteraction,
    prelude::*,
    render::mesh::{Mesh, Mesh3d},
    window::WindowResolution,
};
use crossbeam_channel::{Receiver, Sender, bounded};
use rand::{Rng, SeedableRng};

pub fn init() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    resolution: WindowResolution::new(600., 600.),
                    ..default()
                }),
                ..default()
            }),
            MeshPickingPlugin,
        ))
        .add_systems(Startup, startup)
        .add_systems(
            FixedUpdate,
            rotate_on_drag.run_if(input_pressed(MouseButton::Left)),
        )
        .add_systems(FixedUpdate, look_around_on_drag.run_if(ctrl_pressed))
        .add_systems(Update, zoom_with_scroll)
        .add_systems(
            Update,
            (
                update_train_camera,
                on_escape.run_if(input_just_pressed(KeyCode::Escape)),
            )
                .chain(),
        )
        .add_systems(
            Update,
            on_mouse_right_click.run_if(input_just_pressed(MouseButton::Right)),
        )
        .add_systems(
            Update,
            on_mouse_left_click.run_if(input_just_pressed(MouseButton::Left)),
        )
        .add_systems(Update, create_path_if_dijkstra_ready)
        .add_systems(Update, highlight_city)
        .insert_resource(State {
            config: crate::state::Config::default(),
            globe_points: Arc::new(RwLock::new(GlobePoints::default())),
            rails: crate::state::Rails::default(),
            rng: rand::rngs::StdRng::seed_from_u64(
                crate::state::Config::default().perlin_config.seed as u64,
            ),
            create_new_city_next: true,
            max_rail_usage: 0.into(),
        })
        .insert_resource(SelectedCity::default())
        .add_systems(Update, draw_pointer)
        .add_systems(Update, try_getting_globe)
        .add_systems(Update, move_trains)
        .run();
}

fn ctrl_pressed(
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
) -> bool {
    keys.pressed(KeyCode::ControlLeft)
        || keys.pressed(KeyCode::ControlRight)
        || mouse_buttons.pressed(MouseButton::Middle)
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

#[derive(Resource)]
struct DijkstraCommunication {
    sender: Sender<Option<Vec<GridPoint>>>,
    receiver: Receiver<Option<Vec<GridPoint>>>,
    task: Option<(GridPoint, GridPoint)>,
}

type CameraTransformQuery<'w, 's> =
    Query<'w, 's, &'static mut Transform, (With<MainCamera>, Without<Train>)>;
type LightsTransformQuery<'w, 's> =
    Query<'w, 's, &'static mut Transform, (Without<MainCamera>, Without<Train>, With<PointLight>)>;
type SelectedTrainQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Transform), (With<Train>, With<SelectedTrain>)>;

fn try_getting_globe(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut state: ResMut<State>,
    globe_receiver: Res<GlobeReceiver>,
) {
    if let Ok((globe_points, globe_mesh)) = globe_receiver.receiver.try_recv() {
        println!("Received globe points and mesh.");
        state.globe_points = Arc::new(RwLock::new(globe_points));
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

    let (dijkstra_tx, dijkstra_rx) = bounded(1);
    commands.insert_resource(DijkstraCommunication {
        sender: dijkstra_tx,
        receiver: dijkstra_rx,
        task: None,
    });

    commands.insert_resource(Meshes::new(&mut meshes));
    commands.insert_resource(Materials::new(&mut materials));
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            intensity: 25_000_000.0,
            range: 100.0,
            // shadow_depth_bias: 0.2,
            ..default()
        },
        Transform::from_xyz(-15.0, 0.0, 25.0),
    ));

    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: std::f32::consts::PI / 3.0, // 60 degrees in radians
            ..default()
        }),
        MainCamera,
        Transform::from_xyz(0.0, 0.0, 15.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Z),
    ));
}

fn create_path_if_dijkstra_ready(
    mut commands: Commands,
    mut state: ResMut<State>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut dijkstra_communication: ResMut<DijkstraCommunication>,
    cities: Query<(Entity, &Position), With<City>>,
    meshes: Res<Meshes>,
    custom_materials: Res<Materials>,
) {
    let Some(_) = dijkstra_communication.task else {
        return;
    };
    let Ok(dijkstra_result) = dijkstra_communication.receiver.try_recv() else {
        return;
    };
    let Some(path) = dijkstra_result else {
        println!("Dijkstra returned None, skipping path creation.");
        return;
    };

    // Clear task to signal Dijkstra is ready for another task.
    dijkstra_communication.task = None;

    let path_mesh = meshes.path.clone();
    let train_mesh = meshes.train.clone();

    let material = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(255, 255, 255),
        perceptual_roughness: 0.0,
        metallic: 0.0,
        ..default()
    });

    let globe_points_lock = Arc::clone(&state.globe_points);
    let Ok(mut globe_points) = globe_points_lock.write() else {
        println!("Failed to lock globe points. This should never happen.");
        return;
    };

    let reduction_factor = state.config.reduction_factor;
    // Apply cost reduction to edges in the path (only once per edge)
    for w in path.windows(2) {
        let (from, to) = (w[1], w[0]);
        if let Some(edges) = globe_points.graph.get_mut(&from) {
            for edge in edges.iter_mut() {
                if edge.to == to && !edge.discounted {
                    edge.cost /= reduction_factor;
                    edge.discounted = true;
                }
            }
        }
        if let Some(edges) = globe_points.graph.get_mut(&to) {
            for edge in edges.iter_mut() {
                if edge.to == from && !edge.discounted {
                    edge.cost /= reduction_factor;
                    edge.discounted = true;
                }
            }
        }
    }

    println!("Dijkstra done, path length: {}", path.len());

    let mut train_transforms = Vec::new();

    for line in path.windows(2) {
        let (from, to) = (line[0], line[1]);
        if let Some(from_point) = globe_points.points.get(&from) {
            if let Some(to_point) = globe_points.points.get(&to) {
                let rail = Rail {
                    from: from.min(to),
                    to: from.max(to),
                };

                // compute the transform of both the path segment and train
                let direction = to_point.pos - from_point.pos;
                let length = direction.length();
                let mid_point = (from_point.pos + to_point.pos) / 2.0;

                let dir_norm = direction.normalize();
                let up = Vec3::cross(Vec3::cross(dir_norm, mid_point.normalize()), dir_norm);
                let rotation =
                    Quat::from_mat3(&Mat3::from_cols(Vec3::cross(dir_norm, up), dir_norm, up));

                // If this piece of rail already exists, just change its material
                // corresponding to the current path.
                // We don't _really_ need this, but this demonstrates how to update
                // existig rail piece entities.
                if let std::collections::hash_map::Entry::Vacant(e) = state.rails.rails.entry(rail) {
                    // Otherwise, create a new entity for the rail and store it in the
                    // Rails resource.
                    //
                    // We first create an empty entity in order to already have its ID
                    // which we can key the RailInfo with.
                    //
                    // We'll update it with all the details the same way as we've updated
                    // the existing rail piece above.
                    let entity = commands.spawn_empty().id();
                    e.insert(
                        RailInfo {
                            entity,
                            counter: 0.into(),
                            // Other details can be added here.
                        },
                    );
                commands.entity(entity).insert((
                    Mesh3d(path_mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::from_scale(Vec3 {
                        x: 0.06,
                        y: length,
                        z: 0.04,
                    })
                    .with_translation(mid_point)
                    .with_rotation(rotation),
                    PointerInteraction::default(),
                ));
                }

                train_transforms.push((
                    Transform::from_translation(mid_point * 1.005).with_rotation(rotation),
                    rail,
                ));

            }
        }
    }

    // spawn a train at the first point of the path
    if let Some(mut train) = Train::new(train_transforms) {
        let first_transform = train.current_transform();
        commands.spawn((
            train,
            Mesh3d(train_mesh),
            MeshMaterial3d(custom_materials.train.clone()),
            first_transform,
            PointerInteraction::default(),
        ));
    }

    let add_another_train = state.config.num_automatic_trains > 0;
    if add_another_train {
        state.config.num_automatic_trains -= 1;
        println!(
            "Adding another train, {} left to add.",
            state.config.num_automatic_trains
        );
        let grid_size = state.config.grid_size;

        let prev_city_index = state.rng.random_range(0..cities.iter().len());
        let Some(prev_city) = cities
            .iter()
            .nth(prev_city_index)
            .map(|(_, pos)| pos.gridpoint)
        else {
            panic!("Getting previous city failed.");
        };

        let mut other_city;

        let other_prev_city_index = loop {
            let candidate = state.rng.random_range(0..cities.iter().len());
            if candidate != prev_city_index {
                break candidate;
            }
        };

        other_city = cities
            .iter()
            .nth(other_prev_city_index)
            .map(|(_, pos)| pos.gridpoint);

        if state.create_new_city_next {
            // Find a place for a new city.
            let new_city = loop {
                let candidate_gridpoint = (
                    state.rng.random_range(0..6),
                    state.rng.random_range(0..=grid_size),
                    state.rng.random_range(0..=grid_size),
                );
                let height_threshold = state.rng.random::<f32>().powf(3.0) + 0.01;
                let Some(&globe_point) = globe_points.points.get(&candidate_gridpoint) else {
                    continue; // Skip if no GlobePoint found for this gridpoint
                };
                if globe_point.water {
                    continue; // Skip water points
                }
                if cities.iter().any(|(_, pos)| {
                    (pos.globe_point.pos - globe_point.pos).length()
                        < state.config.min_city_distance
                }) {
                    continue; // Skip if a city already exists at this point
                }
                let height_ratio =
                    (globe_point.pos.length() - state.config.sea_level) / state.config.snow_level;
                if height_ratio < height_threshold {
                    // Spawn a city at this point
                    commands.spawn((
                        City,
                        Position {
                            gridpoint: candidate_gridpoint,
                            globe_point,
                        },
                        Mesh3d(meshes.city.clone()),
                        MeshMaterial3d(custom_materials.city.clone()),
                        Transform::from_xyz(
                            globe_point.pos[0],
                            globe_point.pos[1],
                            globe_point.pos[2],
                        )
                        .looking_at(Vec3::ZERO, Vec3::Z),
                    ));
                    break candidate_gridpoint; // Exit the loop after spawning a city
                }
            };
            other_city = Some(new_city);
        }

        state.create_new_city_next = !state.create_new_city_next;
        let Some(target_city) = other_city else {
            panic!("No target city found, skipping train creation.");
        };

        let globe_points_lock = Arc::clone(&state.globe_points);
        let sender = dijkstra_communication.sender.clone();
        dijkstra_communication.task = Some((prev_city, target_city));
        thread::spawn({
            move || {
                let Ok(globe_points) = globe_points_lock.read() else {
                    println!("Failed to lock globe points. This should never happen.");
                    sender.send(None).unwrap();
                    return;
                };
                let path = dijkstra(prev_city, target_city, &globe_points);
                sender.send(Some(path)).unwrap();
            }
        });
    }
}

fn adjust_light(light_transform: &mut Transform, camera_transform: &Transform) {
    let above_camera = camera_transform.translation + camera_transform.up() * 15.0
        - camera_transform.forward() * 10.0;
    light_transform.translation = above_camera;
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

    // Step 1: Get camera's local axes
    let right = transform.right().as_vec3(); // local right
    let up = transform.up().as_vec3(); // local up

    // Step 2: Apply rotations
    let rot_horizontal = Quat::from_axis_angle(up, dx);
    let rot_vertical = Quat::from_axis_angle(right, dy);
    let rotation = rot_horizontal * rot_vertical;

    transform.translation = rotation * transform.translation;
    transform.rotation = rotation * transform.rotation;

    for mut light_transform in lights_transform.iter_mut() {
        adjust_light(light_transform.0.as_mut(), transform.as_mut());
    }
}

fn look_around_on_drag(
    mut motion_event_reader: EventReader<MouseMotion>,
    mut camera_transform: Query<&mut Transform, With<MainCamera>>,
    mut lights_transform: Query<(&mut Transform, &PointLight), Without<MainCamera>>,
) {
    let (dx, dy) = motion_event_reader
        .read()
        .fold((0.0, 0.0), |(x, y), event| {
            (x - event.delta.x * 0.002, y - event.delta.y * 0.002)
        });

    if dx != 0.0 || dy != 0.0 {
        let mut transform = camera_transform.single_mut().unwrap();
        transform.rotate_local_y(-dx);
        transform.rotate_local_x(-dy);
        for mut light_transform in lights_transform.iter_mut() {
            adjust_light(light_transform.0.as_mut(), transform.as_mut());
        }
    }
}

fn zoom_with_scroll(
    mut scroll_evr: EventReader<MouseWheel>,
    mut camera_transform: Query<(&mut Transform, &MainCamera)>,
    mut lights_transform: Query<(&mut Transform, &PointLight), Without<MainCamera>>,
) {
    let scroll: f32 = scroll_evr.read().map(|e| e.y).sum();
    if scroll == 0.0 {
        return;
    }

    let (mut transform, _cam) = camera_transform.single_mut().unwrap();
    let motion = transform.forward().normalize() * scroll * 0.5;
    transform.translation += motion;

    for mut light_transform in lights_transform.iter_mut() {
        adjust_light(light_transform.0.as_mut(), transform.as_mut());
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
        let Ok(globe_points) = state.globe_points.read() else {
            println!("Failed to lock globe points. This should never happen.");
            return;
        };
        if let Some(&globe_point) = globe_points.points.get(&gridpoint) {
            if globe_point.water {
                println!("Can't place city on water: {gridpoint:?}");
                continue; // Skip water points
            }

            if cities.iter().any(|(_, pos)| {
                (pos.globe_point.pos - globe_point.pos).length() < state.config.min_city_distance
            }) {
                println!("City already exists near gridpoint: {gridpoint:?}");
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
            println!("No GlobePoint found for gridpoint: {gridpoint:?}");
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn on_mouse_left_click(
    pointers: Query<&PointerInteraction>,
    state: Res<State>,
    mut dijkstra_communication: ResMut<DijkstraCommunication>,
    mut commands: Commands,
    cities: Query<(Entity, &Position), With<City>>,
    mut selected: ResMut<SelectedCity>,
    materials: Res<Materials>,
    trains: Query<(&Train, &Transform), With<Train>>,
    mut camera_transform: Query<(&mut Transform, &MainCamera), Without<Train>>,
) {
    for (clicked_entity, clicked_point) in pointers
        .iter()
        .filter_map(|interaction| interaction.get_nearest_hit())
        .filter_map(|(entity, hit)| hit.position.map(|pos| (entity, pos)))
    {
        if let Ok((_train, train_transform)) = trains.get(*clicked_entity) {
            let (mut transform, _camera) = camera_transform.single_mut().unwrap();
            move_camera_to_train(&mut transform, train_transform, true);
            commands.entity(*clicked_entity).insert(SelectedTrain);
            return;
        }

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
                    println!("Selected city {clicked_city:?}");
                }
                Some(prev_selected) => {
                    if prev_selected != clicked_city {
                        // Connect the cities
                        println!("Connecting {prev_selected:?} and {clicked_city:?}");
                        if dijkstra_communication.task.is_some() {
                            println!("Dijkstra is busy, skipping connection.");
                        } else {
                            let globe_points_lock = Arc::clone(&state.globe_points);
                            let sender = dijkstra_communication.sender.clone();
                            let start = cities.get(prev_selected).unwrap().1.gridpoint;
                            let end = cities.get(clicked_city).unwrap().1.gridpoint;
                            dijkstra_communication.task = Some((start, end));
                            thread::spawn({
                                move || {
                                    let Ok(globe_points) = globe_points_lock.read() else {
                                        println!(
                                            "Failed to lock globe points. This should never happen."
                                        );
                                        sender.send(None).unwrap();
                                        return;
                                    };
                                    let path = dijkstra(start, end, &globe_points);
                                    sender.send(Some(path)).unwrap();
                                }
                            });
                        }
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

fn move_trains(
    mut commands: Commands,
    state: Res<State>,
    time: Res<Time>,
    mut trains: Query<(&mut Train, &mut Transform), With<Train>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let time_passed_seconds = time.delta().as_secs_f32();

    for (mut train, mut transform) in trains.iter_mut() {
        train.update(
            &mut transform,
            time_passed_seconds,
            &state,
            &mut commands,
            &mut materials,
        );
    }
}

fn move_camera_to_train(
    camera_transform: &mut Transform,
    train_transform: &Transform,
    forward: bool,
) {
    let rot = if forward {
        Quat::from_rotation_arc(Vec3::Y, Vec3::Z)
    } else {
        Quat::from_rotation_arc(Vec3::Y, -Vec3::Z)
    };
    camera_transform.translation = train_transform.translation + train_transform.local_z() * 0.12;
    camera_transform.rotation = train_transform.rotation * rot;
    if !forward {
        camera_transform.rotate_local_z(std::f32::consts::PI);
    }
    // rotate the camera a little bit downwards
    camera_transform.rotate_local_x(-0.2);
}

fn update_train_camera(
    mut camera_transform_q: Query<&mut Transform, (With<MainCamera>, Without<Train>)>,
    trains_q: Query<(&Train, &Transform, &SelectedTrain), With<Train>>,
) {
    if let Ok((train, train_transform, _)) = trains_q.single() {
        if let Ok(mut camera_transform) = camera_transform_q.single_mut() {
            move_camera_to_train(&mut camera_transform, train_transform, train.forward);
        }
    }
}

fn on_escape(
    mut commands: Commands,
    selected_train: SelectedTrainQuery,
    mut camera_transform_q: CameraTransformQuery,
    mut lights_transform_q: LightsTransformQuery,
) {
    // if there is a selected train, deselect it, and reset the camera
    if let Some((train_entity, train_transform)) = selected_train.iter().next() {
        commands.entity(train_entity).remove::<SelectedTrain>();

        if let Ok(mut camera_transform) = camera_transform_q.single_mut() {
            // println!("Resetting camera to orbital position above the train.");
            camera_transform.translation = train_transform.translation.normalize() * 15.0;
            camera_transform.look_at(Vec3::ZERO, Vec3::Z);
            for mut light_transform in lights_transform_q.iter_mut() {
                adjust_light(light_transform.as_mut(), camera_transform.as_mut());
            }
        }
    }
}
