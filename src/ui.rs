use crate::scene::{Globe, NewPlanetRequested};

use bevy::{input::common_conditions::*, prelude::*};

/// The 2D UI over the scene: the menu, the usage hints, and the label shown while a planet is
/// generated.
pub struct OverlayPlugin;

impl Plugin for OverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_menu, spawn_generating_planet_label))
            .add_systems(
                Update,
                (
                    toggle_menu.run_if(button_pressed(ButtonAction::ToggleMenu)),
                    close_menu.run_if(
                        input_just_pressed(MouseButton::Left)
                            .and(not(button_pressed(ButtonAction::ToggleMenu))),
                    ),
                    show_usage_hints.run_if(button_pressed(ButtonAction::ShowUsageHints)),
                    hide_usage_hints.run_if(
                        button_pressed(ButtonAction::HideUsageHints)
                            .or(button_pressed(ButtonAction::ToggleMenu)),
                    ),
                    request_new_planet.run_if(button_pressed(ButtonAction::NewPlanet)),
                    highlight_hovered_buttons,
                    show_generating_planet_label,
                ),
            );
    }
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

fn request_new_planet(mut requests: EventWriter<NewPlanetRequested>) {
    requests.write(NewPlanetRequested);
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
