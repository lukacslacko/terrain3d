use crate::dijkstra::get_closest_gridpoint;
use crate::meshes_materials::{Materials, Meshes};
use crate::scene::{
    City, DijkstraCommunication, MainCamera, NewPlanetRequested, Position, spawn_city,
};
use crate::state::State;
use crate::train::{SelectedTrain, Train};

use bevy::{
    color::palettes::tailwind::*,
    ecs::system::SystemParam,
    input::{common_conditions::*, mouse::*},
    picking::pointer::PointerInteraction,
    prelude::*,
};

/// Moving the camera, and clicking on the planet: creating and connecting cities, and riding
/// trains.
pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
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
        .add_systems(Update, highlight_city)
        .insert_resource(SelectedCity::default())
        // In `PreUpdate`, like `new_planet`, so that `Update` never sees a city of the old planet
        // as selected.
        .add_systems(
            PreUpdate,
            forget_selected_city.run_if(on_event::<NewPlanetRequested>),
        )
        .add_systems(Update, draw_pointer);
    }
}

fn ctrl_pressed(
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
) -> bool {
    keys.pressed(KeyCode::ControlLeft)
        || keys.pressed(KeyCode::ControlRight)
        || mouse_buttons.pressed(MouseButton::Middle)
}

#[derive(Resource, Default)]
struct SelectedCity(Option<Entity>);

type CameraTransformQuery<'w, 's> =
    Query<'w, 's, &'static mut Transform, (With<MainCamera>, Without<Train>)>;
type LightsTransformQuery<'w, 's> =
    Query<'w, 's, &'static mut Transform, (Without<MainCamera>, Without<Train>, With<PointLight>)>;
type SelectedTrainQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Transform), (With<Train>, With<SelectedTrain>)>;

fn forget_selected_city(mut selected: ResMut<SelectedCity>) {
    selected.0 = None;
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

/// Roughly how many logical pixels browsers scroll per mouse wheel notch.
const PIXELS_PER_SCROLL_LINE: f32 = 100.0;

fn zoom_with_scroll(
    mut scroll_evr: EventReader<MouseWheel>,
    windows: Query<&Window>,
    mut camera_transform: Query<(&mut Transform, &MainCamera)>,
    mut lights_transform: Query<(&mut Transform, &PointLight), Without<MainCamera>>,
) {
    // Native mouse wheels report lines, but browsers (and touchpads) report physical pixels,
    // so convert pixels to lines to get the same zoom speed everywhere.
    let scroll: f32 = scroll_evr
        .read()
        .map(|e| match e.unit {
            MouseScrollUnit::Line => e.y,
            MouseScrollUnit::Pixel => {
                let scale_factor = windows.get(e.window).map_or(1.0, |w| w.scale_factor());
                e.y / scale_factor / PIXELS_PER_SCROLL_LINE
            }
        })
        .sum();
    if scroll == 0.0 {
        return;
    }

    let (mut transform, _cam) = camera_transform.single_mut().unwrap();
    let step = scroll * 0.5;
    let direction = transform.forward().as_vec3() * step.signum();
    let step = step
        .abs()
        .min(max_step_toward_globe(transform.translation, direction));
    transform.translation += direction * step;

    for mut light_transform in lights_transform.iter_mut() {
        adjust_light(light_transform.0.as_mut(), transform.as_mut());
    }
}

/// The camera can't zoom closer than this to the globe's center (the highest peaks are at ~5.5).
const MIN_CAMERA_DISTANCE: f32 = 6.0;

/// How far the camera can move from `position` along the unit vector `direction` before it gets
/// closer than `MIN_CAMERA_DISTANCE` to the globe's center.
fn max_step_toward_globe(position: Vec3, direction: Vec3) -> f32 {
    let b = position.dot(direction);
    if b >= 0.0 {
        // Moving away from the center.
        return f32::INFINITY;
    }
    let c = position.length_squared() - MIN_CAMERA_DISTANCE * MIN_CAMERA_DISTANCE;
    if c <= 0.0 {
        // Already too close.
        return 0.0;
    }
    // Smallest t >= 0 with |position + t * direction| = MIN_CAMERA_DISTANCE, if there is one.
    let discriminant = b * b - c;
    if discriminant < 0.0 {
        return f32::INFINITY;
    }
    -b - discriminant.sqrt()
}

/// What the pointers point at in the world.
#[derive(SystemParam)]
struct WorldPointers<'w, 's> {
    interactions: Query<'w, 's, &'static PointerInteraction>,
    ui_nodes: Query<'w, 's, (), With<Node>>,
}

impl WorldPointers<'_, '_> {
    /// The nearest entity and point under each pointer. Pointers over the UI are left out: they
    /// don't point at the world, and the position of a hit on a UI node is within the node.
    fn hits(&self) -> impl Iterator<Item = (Entity, Vec3)> + '_ {
        self.interactions
            .iter()
            .filter_map(|interaction| interaction.get_nearest_hit())
            .filter(|(entity, _hit)| !self.ui_nodes.contains(*entity))
            .filter_map(|(entity, hit)| hit.position.map(|pos| (*entity, pos)))
    }
}

fn draw_pointer(pointers: WorldPointers, mut gizmos: Gizmos) {
    for (_entity, point) in pointers.hits() {
        gizmos.sphere(point, 0.05, RED_500);
    }
}

fn on_mouse_right_click(
    pointers: WorldPointers,
    state: Res<State>,
    mut commands: Commands,
    cities: Query<(Entity, &Position), With<City>>,
    meshes: Res<Meshes>,
    materials: Res<Materials>,
) {
    for (_entity, point) in pointers.hits() {
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

            spawn_city(&mut commands, gridpoint, globe_point, &meshes, &materials);
        } else {
            println!("No GlobePoint found for gridpoint: {gridpoint:?}");
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn on_mouse_left_click(
    pointers: WorldPointers,
    state: Res<State>,
    mut dijkstra_communication: ResMut<DijkstraCommunication>,
    mut commands: Commands,
    cities: Query<(Entity, &Position), With<City>>,
    mut selected: ResMut<SelectedCity>,
    materials: Res<Materials>,
    trains: Query<(&Train, &Transform), With<Train>>,
    mut camera_transform: Query<(&mut Transform, &MainCamera), Without<Train>>,
) {
    for (clicked_entity, clicked_point) in pointers.hits() {
        if let Ok((_train, train_transform)) = trains.get(clicked_entity) {
            let (mut transform, _camera) = camera_transform.single_mut().unwrap();
            move_camera_to_train(&mut transform, train_transform, true);
            commands.entity(clicked_entity).insert(SelectedTrain);
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
                        if dijkstra_communication.is_busy() {
                            println!("Dijkstra is busy, skipping connection.");
                        } else {
                            let start = cities.get(prev_selected).unwrap().1.gridpoint;
                            let end = cities.get(clicked_city).unwrap().1.gridpoint;
                            dijkstra_communication.start(start, end, &state.globe_points);
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
    pointers: WorldPointers,
    mut commands: Commands,
    cities: Query<(Entity, &Position), With<City>>,
    materials: Res<Materials>,
    selected: Res<SelectedCity>,
) {
    for (_entity, point) in pointers.hits() {
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
    if let Ok((train, train_transform, _)) = trains_q.single()
        && let Ok(mut camera_transform) = camera_transform_q.single_mut()
    {
        move_camera_to_train(&mut camera_transform, train_transform, train.forward);
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
