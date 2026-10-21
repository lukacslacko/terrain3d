use std::sync::{Arc, RwLock};

use crate::dijkstra::{GlobePoint, GlobePoints, GridPoint, bidirectional_dijkstra};
use crate::meshes_materials::{Materials, Meshes, make_globe};
use crate::state::{Config, Rail, RailInfo, State};
use crate::train::Train;

use bevy::{
    picking::pointer::PointerInteraction,
    prelude::*,
    render::mesh::{Mesh, Mesh3d},
};
use crossbeam_channel::{Receiver, Sender, bounded};
use rand::Rng;

/// The planet and everything on it, and the camera and light that show it.
pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<NewPlanetRequested>()
            .add_systems(Startup, startup)
            // In `PreUpdate`, so that its commands are applied before `Update`, whose systems would
            // otherwise still see the old planet's entities, after `State` is already the new one's.
            .add_systems(PreUpdate, new_planet.run_if(on_event::<NewPlanetRequested>))
            .add_systems(Update, create_path_if_dijkstra_ready)
            .insert_resource(State::new(Config::default()))
            .add_systems(Update, try_getting_globe)
            .add_systems(Update, move_trains);
    }
}

/// Replaces the planet, and everything on it, with a new one, in the next frame's `PreUpdate`.
#[derive(Event)]
pub struct NewPlanetRequested;

#[derive(Component)]
pub struct Globe;

#[derive(Component)]
pub struct MainCamera;

#[derive(Component)]
pub struct City;

#[derive(Component)]
pub struct Position {
    pub gridpoint: GridPoint,
    pub globe_point: GlobePoint,
}

#[derive(Resource)]
struct GlobeReceiver {
    receiver: Receiver<(GlobePoints, Mesh)>,
}

#[derive(Resource)]
pub struct DijkstraCommunication {
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

    /// Whether a path is being searched for.
    pub fn is_busy(&self) -> bool {
        self.task.is_some()
    }

    /// Searches for a path from `start` to `end` in the background, for
    /// `create_path_if_dijkstra_ready` to build rails and a train on.
    pub fn start(
        &mut self,
        start: GridPoint,
        end: GridPoint,
        globe_points: &Arc<RwLock<GlobePoints>>,
    ) {
        let globe_points_lock = Arc::clone(globe_points);
        let sender = self.sender.clone();
        self.task = Some((start, end));
        spawn_background(move || {
            let Ok(globe_points) = globe_points_lock.read() else {
                println!("Failed to lock globe points. This should never happen.");
                sender.send(None).ok();
                return;
            };
            let path = bidirectional_dijkstra(start, end, &globe_points);
            // Fails if a new planet was generated in the meantime. The path isn't needed then.
            sender.send(Some(path)).ok();
        });
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

pub fn spawn_city(
    commands: &mut Commands,
    gridpoint: GridPoint,
    globe_point: GlobePoint,
    meshes: &Meshes,
    materials: &Materials,
) {
    commands.spawn((
        City,
        Position {
            gridpoint,
            globe_point,
        },
        Mesh3d(meshes.city.clone()),
        MeshMaterial3d(materials.city.clone()),
        Transform::from_translation(globe_point.pos).looking_at(Vec3::ZERO, Vec3::Z),
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
        // The train only leaves its first rail, it doesn't come in on it.
        train.count_current_rail(1, &state, &mut commands, &mut materials);
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
                    spawn_city(
                        &mut commands,
                        candidate_gridpoint,
                        globe_point,
                        &meshes,
                        &custom_materials,
                    );
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
        dijkstra_communication.start(prev_city, target_city, &state.globe_points);
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
