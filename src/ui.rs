use std::sync::{Arc, RwLock};

use crate::dijkstra::{
    GlobePoint, GlobePoints, GridPoint, bidirectional_dijkstra, get_closest_gridpoint,
};
use crate::meshes_materials::{Materials, Meshes, make_globe};
use crate::state::{Config, Rail, RailInfo, State};
use crate::train::{SelectedTrain, Train};

use bevy::{
    color::palettes::tailwind::*,
    ecs::system::SystemParam,
    input::{common_conditions::*, mouse::*},
    picking::pointer::PointerInteraction,
    prelude::*,
    render::mesh::{Mesh, Mesh3d},
    ui::UiSystem,
    window::WindowResolution,
};
use crossbeam_channel::{Receiver, Sender, bounded};
use rand::Rng;

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
        .add_systems(
            Startup,
            (startup, spawn_menu, spawn_generating_planet_label),
        )
        .add_systems(
            Update,
            (
                toggle_menu.run_if(button_pressed(ButtonAction::ToggleMenu)),
                close_menu.run_if(
                    input_just_pressed(MouseButton::Left)
                        .and(not(button_pressed(ButtonAction::ToggleMenu))),
                ),
                show_usage_hints.run_if(button_pressed(ButtonAction::ShowUsageHints)),
                hide_usage_hints.run_if(button_pressed(ButtonAction::HideUsageHints)),
                highlight_hovered_buttons,
                show_generating_planet_label,
            ),
        )
        // In `PreUpdate`, so that its commands are applied before `Update`, whose systems would
        // otherwise still see the old planet's entities, after `State` is already the new one's.
        .add_systems(
            PreUpdate,
            new_planet
                .run_if(button_pressed(ButtonAction::NewPlanet))
                .after(UiSystem::Focus),
        )
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
        .insert_resource(State::new(Config::default()))
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

impl DijkstraCommunication {
    fn new() -> Self {
        let (sender, receiver) = bounded(1);
        Self {
            sender,
            receiver,
            task: None,
        }
    }
}

/// Runs `f` on a new thread. Threads are not available on the web, so there `f`
/// runs right away on the main thread instead.
fn spawn_background(f: impl FnOnce() + Send + 'static) {
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(f);
    #[cfg(target_arch = "wasm32")]
    f();
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

/// Generates the globe for `config`, for `try_getting_globe` to pick up.
fn start_generating_globe(commands: &mut Commands, config: &Config) {
    let (tx, rx) = bounded(1);
    let config_for_make_globe = config.clone();
    spawn_background(move || {
        let (globe_points, globe_mesh) = make_globe(&config_for_make_globe);
        // Fails if another planet was requested in the meantime. This one isn't needed then.
        tx.send((globe_points, globe_mesh)).ok();
    });

    commands.insert_resource(GlobeReceiver { receiver: rx });
}

fn camera_start_transform() -> Transform {
    Transform::from_xyz(0.0, 0.0, 15.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Z)
}

const LIGHT_START_POSITION: Vec3 = Vec3::new(-15.0, 0.0, 25.0);

fn startup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    state: Res<State>,
) {
    start_generating_globe(&mut commands, &state.config);
    commands.insert_resource(DijkstraCommunication::new());

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
        Transform::from_translation(LIGHT_START_POSITION),
    ));

    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: std::f32::consts::PI / 3.0, // 60 degrees in radians
            ..default()
        }),
        MainCamera,
        camera_start_transform(),
    ));
}

const USAGE_HINTS: &str = "\
Left drag: rotate the planet
Scroll: zoom in and out
Right click: create a city
Left click two cities: connect them
Left click a train: ride it
Esc: get off the train";

const PANEL_COLOR: Color = Color::srgba(0.0, 0.0, 0.0, 0.5);
const MENU_COLOR: Color = Color::srgb(0.25, 0.25, 0.25);
const HOVERED_BUTTON_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.15);

/// What a button does when pressed.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum ButtonAction {
    ToggleMenu,
    ShowUsageHints,
    HideUsageHints,
    NewPlanet,
}

/// The menu that opens from the hamburger button.
#[derive(Component)]
struct Menu;

#[derive(Component)]
struct UsageHints;

#[derive(Component)]
struct GeneratingPlanetLabel;

/// A run condition that is true when a button with `action` was just pressed.
fn button_pressed(
    action: ButtonAction,
) -> impl FnMut(Query<(&Interaction, &ButtonAction), Changed<Interaction>>) -> bool + Clone {
    move |buttons| {
        buttons.iter().any(|(interaction, button_action)| {
            *interaction == Interaction::Pressed && *button_action == action
        })
    }
}

fn label(text: &'static str, font_size: f32) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size,
            ..default()
        },
    )
}

fn menu_item(action: ButtonAction, text: &'static str) -> impl Bundle {
    (
        action,
        Button,
        Node {
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(Color::NONE),
        children![label(text, 14.0)],
    )
}

/// Spawns the hamburger button, the menu it opens, and the usage hints, which are open at the
/// start.
fn spawn_menu(mut commands: Commands) {
    let hamburger_bar = || {
        (
            Node {
                width: Val::Px(16.0),
                height: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(Color::WHITE),
        )
    };
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            ..default()
        },
        BackgroundColor(PANEL_COLOR),
        BorderRadius::all(Val::Px(4.0)),
        children![(
            ButtonAction::ToggleMenu,
            Button,
            Node {
                width: Val::Px(32.0),
                height: Val::Px(32.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderRadius::all(Val::Px(4.0)),
            children![hamburger_bar(), hamburger_bar(), hamburger_bar()],
        )],
    ));

    commands.spawn((
        Menu,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(48.0),
            left: Val::Px(8.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::vertical(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(MENU_COLOR),
        BorderRadius::all(Val::Px(4.0)),
        BoxShadow::new(
            Color::BLACK.with_alpha(0.8),
            Val::Px(0.0),
            Val::Px(2.0),
            Val::Px(0.0),
            Val::Px(8.0),
        ),
        Visibility::Hidden,
        // Over the usage hints, which are at the same place.
        GlobalZIndex(1),
        children![
            menu_item(ButtonAction::ShowUsageHints, "How to use"),
            menu_item(ButtonAction::NewPlanet, "Generate new planet"),
        ],
    ));

    commands.spawn((
        UsageHints,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(48.0),
            left: Val::Px(8.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(PANEL_COLOR),
        BorderRadius::all(Val::Px(4.0)),
        children![
            (
                Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                },
                children![
                    label("How to use", 14.0),
                    (
                        ButtonAction::HideUsageHints,
                        Button,
                        Node {
                            padding: UiRect::horizontal(Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                        BorderRadius::all(Val::Px(4.0)),
                        children![label("x", 14.0)],
                    ),
                ],
            ),
            label(USAGE_HINTS, 13.0),
        ],
    ));
}

/// Spawns the label that `show_generating_planet_label` shows while there's no planet.
fn spawn_generating_planet_label(mut commands: Commands) {
    commands.spawn((
        GeneratingPlanetLabel,
        label("Generating planet...", 16.0),
        TextLayout::new_with_justify(JustifyText::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(50.0),
            width: Val::Percent(100.0),
            ..default()
        },
    ));
}

fn toggle_menu(mut menu: Single<&mut Visibility, With<Menu>>) {
    menu.toggle_visible_hidden();
}

fn close_menu(mut menu: Single<&mut Visibility, With<Menu>>) {
    menu.set_if_neq(Visibility::Hidden);
}

fn show_usage_hints(mut hints: Single<&mut Visibility, With<UsageHints>>) {
    hints.set_if_neq(Visibility::Visible);
}

fn hide_usage_hints(mut hints: Single<&mut Visibility, With<UsageHints>>) {
    hints.set_if_neq(Visibility::Hidden);
}

type ChangedButtonColorsQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static mut BackgroundColor),
    (Changed<Interaction>, With<Button>),
>;

fn highlight_hovered_buttons(mut buttons: ChangedButtonColorsQuery) {
    for (interaction, mut color) in &mut buttons {
        color.0 = match interaction {
            Interaction::None => Color::NONE,
            Interaction::Hovered | Interaction::Pressed => HOVERED_BUTTON_COLOR,
        };
    }
}

fn show_generating_planet_label(
    globes: Query<(), With<Globe>>,
    mut label: Single<&mut Visibility, With<GeneratingPlanetLabel>>,
) {
    label.set_if_neq(if globes.is_empty() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
}

/// The seeds of new planets are below this. The noise adds the seed to the coordinates as an
/// `f32`, so a large seed would leave too little precision for the terrain's details.
const MAX_SEED: u32 = 1000;

/// The globe, the cities and the trains. The rails are in `State`.
type PlanetEntitiesQuery<'w, 's> =
    Query<'w, 's, Entity, Or<(With<Globe>, With<City>, With<Train>)>>;

/// Replaces the planet, and everything on it, with a new one with a random seed.
fn new_planet(
    mut commands: Commands,
    mut state: ResMut<State>,
    mut selected: ResMut<SelectedCity>,
    mut dijkstra_communication: ResMut<DijkstraCommunication>,
    entities: PlanetEntitiesQuery,
    mut camera_transform: Single<&mut Transform, With<MainCamera>>,
    mut lights_transform: Query<&mut Transform, (With<PointLight>, Without<MainCamera>)>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    for rail_info in state.rails.rails.values() {
        commands.entity(rail_info.entity).despawn();
    }
    selected.0 = None;
    // Forget the path being searched for, if any: it's on the old planet.
    *dijkstra_communication = DijkstraCommunication::new();

    let seed = loop {
        let seed = rand::rng().random_range(0..MAX_SEED);
        if seed != state.config.perlin_config.seed {
            break seed;
        }
    };
    println!("Generating a new planet with seed {seed}.");
    let mut config = Config::default();
    config.perlin_config.seed = seed;
    *state = State::new(config);
    start_generating_globe(&mut commands, &state.config);

    // Back to the start, which also gets the camera off a train if it was riding one.
    **camera_transform = camera_start_transform();
    for mut light_transform in &mut lights_transform {
        light_transform.translation = LIGHT_START_POSITION;
    }
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

    // Apply cost reduction to edges in the path (only once per edge)
    for w in path.windows(2) {
        globe_points.discount_edge(w[0], w[1]);
    }

    println!("Dijkstra done, path length: {}", path.len());

    let mut train_transforms = Vec::new();

    for line in path.windows(2) {
        let (from, to) = (line[0], line[1]);
        if let Some(from_point) = globe_points.points.get(&from)
            && let Some(to_point) = globe_points.points.get(&to)
        {
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
                e.insert(RailInfo {
                    entity,
                    counter: 0.into(),
                    // Other details can be added here.
                });
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

        // Release the write lock, the background task needs to read the globe points.
        drop(globe_points);
        let globe_points_lock = Arc::clone(&state.globe_points);
        let sender = dijkstra_communication.sender.clone();
        dijkstra_communication.task = Some((prev_city, target_city));
        spawn_background({
            move || {
                let Ok(globe_points) = globe_points_lock.read() else {
                    println!("Failed to lock globe points. This should never happen.");
                    sender.send(None).unwrap();
                    return;
                };
                let path = bidirectional_dijkstra(prev_city, target_city, &globe_points);
                // Fails if a new planet was generated in the meantime. The path isn't needed then.
                sender.send(Some(path)).ok();
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
                        if dijkstra_communication.task.is_some() {
                            println!("Dijkstra is busy, skipping connection.");
                        } else {
                            let globe_points_lock = Arc::clone(&state.globe_points);
                            let sender = dijkstra_communication.sender.clone();
                            let start = cities.get(prev_selected).unwrap().1.gridpoint;
                            let end = cities.get(clicked_city).unwrap().1.gridpoint;
                            dijkstra_communication.task = Some((start, end));
                            spawn_background({
                                move || {
                                    let Ok(globe_points) = globe_points_lock.read() else {
                                        println!(
                                            "Failed to lock globe points. This should never happen."
                                        );
                                        sender.send(None).unwrap();
                                        return;
                                    };
                                    let path = bidirectional_dijkstra(start, end, &globe_points);
                                    // Fails if a new planet was generated in the meantime.
                                    sender.send(Some(path)).ok();
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
