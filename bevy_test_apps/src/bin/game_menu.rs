//! A multi-screen menu for exercising Bevy states, UI navigation, and controlled time.
use std::borrow::Cow;

use bevy::{
    app::AppExit,
    color::palettes::css::{CRIMSON, DARK_RED, LIME, NAVY},
    prelude::*,
    window::{Window, WindowResolution},
};

#[cfg(feature = "automation")]
use automation_control::AutomationTarget;

const TEXT_COLOR: Color = Color::srgb(0.92, 0.92, 0.92);
const NORMAL_BUTTON: Color = Color::srgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::srgb(0.25, 0.25, 0.25);
const SELECTED_BUTTON: Color = Color::srgb(0.35, 0.75, 0.35);
const SPLASH_SECONDS: f32 = 1.0;
const GAME_SECONDS: f32 = 5.0;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Reflect, States)]
enum ScreenState {
    #[default]
    Splash,
    Menu,
    Game,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Reflect, States)]
enum MenuState {
    Main,
    Settings,
    SettingsDisplay,
    SettingsSound,
    #[default]
    Disabled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Reflect, Resource)]
enum DisplayQuality {
    Low,
    Medium,
    High,
}

impl DisplayQuality {
    const fn slug(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }
}

impl Default for DisplayQuality {
    fn default() -> Self {
        Self::Medium
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Reflect, Resource)]
struct Volume(u32);

#[derive(Clone, Debug, Component, Reflect)]
#[reflect(Component)]
struct SessionObservation {
    game_state: ScreenState,
    menu_state: MenuState,
    display_quality: DisplayQuality,
    volume: u32,
    splash_elapsed_seconds: f32,
    splash_duration_seconds: f32,
    game_elapsed_seconds: f32,
    game_duration_seconds: f32,
}

impl Default for SessionObservation {
    fn default() -> Self {
        Self {
            game_state: ScreenState::Splash,
            menu_state: MenuState::Disabled,
            display_quality: DisplayQuality::Medium,
            volume: 7,
            splash_elapsed_seconds: 0.0,
            splash_duration_seconds: SPLASH_SECONDS,
            game_elapsed_seconds: 0.0,
            game_duration_seconds: GAME_SECONDS,
        }
    }
}

#[derive(Clone, Copy, Component)]
enum ButtonIntent {
    Play,
    Settings,
    DisplaySettings,
    SoundSettings,
    MainMenu,
    SettingsMenu,
    Quality(DisplayQuality),
    Volume(Volume),
    Quit,
}

#[derive(Component)]
struct SelectedSetting;

#[derive(Resource, Deref, DerefMut)]
struct SplashTimer(Timer);

#[derive(Resource, Deref, DerefMut)]
struct ReturnTimer(Timer);

fn main() {
    let mut app = App::new();
    bevy_test_apps::composition::rendered(
        &mut app,
        Window {
            title: "Controlled game menu test".into(),
            resolution: WindowResolution::new(800, 600).with_scale_factor_override(1.0),
            resizable: false,
            ..default()
        },
    );
    add_game_menu(&mut app);
    app.run();
}

fn add_game_menu(app: &mut App) {
    app.insert_resource(DisplayQuality::default())
        .insert_resource(Volume(7))
        .init_state::<ScreenState>()
        .init_state::<MenuState>()
        .register_type::<ScreenState>()
        .register_type::<MenuState>()
        .register_type::<DisplayQuality>()
        .register_type::<Volume>()
        .register_type::<SessionObservation>()
        .add_systems(Startup, setup)
        .add_systems(OnEnter(ScreenState::Splash), present_splash)
        .add_systems(
            Update,
            splash_countdown.run_if(in_state(ScreenState::Splash)),
        )
        .add_systems(OnEnter(ScreenState::Menu), enter_menu)
        .add_systems(OnEnter(ScreenState::Game), start_game)
        .add_systems(Update, game_countdown.run_if(in_state(ScreenState::Game)))
        .add_systems(OnEnter(MenuState::Main), show_main_menu)
        .add_systems(OnEnter(MenuState::Settings), open_settings_menu)
        .add_systems(OnEnter(MenuState::SettingsDisplay), choose_display_quality)
        .add_systems(OnEnter(MenuState::SettingsSound), choose_volume)
        .add_systems(
            Update,
            (button_colors, button_actions, keyboard_navigation)
                .run_if(in_state(ScreenState::Menu)),
        )
        .add_systems(PostUpdate, update_observation);
}

fn setup(mut commands: Commands) {
    commands.spawn((Name::new("menu-camera"), Camera2d));
    let entity = commands
        .spawn((Name::new("game-menu-state"), SessionObservation::default()))
        .id();
    mark_target(&mut commands, entity);
}

fn present_splash(mut commands: Commands) {
    let root = create_screen(
        &mut commands,
        "splash-screen",
        ScreenState::Splash,
        NAVY.into(),
    );
    commands.entity(root).with_child((
        Name::new("splash-title"),
        Text::new("Bevy Game Menu"),
        TextFont {
            font_size: FontSize::Px(72.0),
            ..default()
        },
        TextColor(TEXT_COLOR),
        Pickable::IGNORE,
    ));
    commands.insert_resource(SplashTimer(Timer::from_seconds(
        SPLASH_SECONDS,
        TimerMode::Once,
    )));
}

fn splash_countdown(
    time: Res<Time>,
    mut timer: ResMut<SplashTimer>,
    mut next_state: ResMut<NextState<ScreenState>>,
) {
    if timer.tick(time.delta()).is_finished() {
        next_state.set(ScreenState::Menu);
    }
}

fn enter_menu(mut next_menu: ResMut<NextState<MenuState>>) {
    next_menu.set(MenuState::Main);
}

fn start_game(mut commands: Commands, quality: Res<DisplayQuality>, volume: Res<Volume>) {
    let root = create_screen(
        &mut commands,
        "game-screen",
        ScreenState::Game,
        DARK_RED.into(),
    );
    commands.entity(root).with_children(|parent| {
        parent.spawn((
            Name::new("game-title"),
            Text::new("Game running"),
            TextFont {
                font_size: FontSize::Px(72.0),
                ..default()
            },
            TextColor(TEXT_COLOR),
            Pickable::IGNORE,
        ));
        parent.spawn((
            Name::new("game-settings-summary"),
            Text::new(format!("quality: {:?}  volume: {}", *quality, volume.0)),
            TextFont {
                font_size: FontSize::Px(40.0),
                ..default()
            },
            TextColor(LIME.into()),
            Pickable::IGNORE,
        ));
    });
    commands.insert_resource(ReturnTimer(Timer::from_seconds(
        GAME_SECONDS,
        TimerMode::Once,
    )));
}

fn game_countdown(
    time: Res<Time>,
    mut timer: ResMut<ReturnTimer>,
    mut next_game: ResMut<NextState<ScreenState>>,
) {
    if timer.tick(time.delta()).is_finished() {
        next_game.set(ScreenState::Menu);
    }
}

fn show_main_menu(mut commands: Commands) {
    let root = menu_screen(&mut commands, "main-menu-screen", MenuState::Main);
    insert_heading(&mut commands, root, "main-menu-title", "Bevy Game Menu UI");
    add_button(
        &mut commands,
        root,
        "new-game-button",
        "New Game",
        ButtonIntent::Play,
        300.0,
    );
    add_button(
        &mut commands,
        root,
        "settings-button",
        "Settings",
        ButtonIntent::Settings,
        300.0,
    );
    add_button(
        &mut commands,
        root,
        "quit-button",
        "Quit",
        ButtonIntent::Quit,
        300.0,
    );
}

fn open_settings_menu(mut commands: Commands) {
    let root = menu_screen(&mut commands, "settings-menu-screen", MenuState::Settings);
    insert_heading(&mut commands, root, "settings-title", "Settings");
    add_button(
        &mut commands,
        root,
        "display-settings-button",
        "Display",
        ButtonIntent::DisplaySettings,
        260.0,
    );
    add_button(
        &mut commands,
        root,
        "sound-settings-button",
        "Sound",
        ButtonIntent::SoundSettings,
        260.0,
    );
    add_button(
        &mut commands,
        root,
        "settings-back-button",
        "Back",
        ButtonIntent::MainMenu,
        260.0,
    );
}

fn choose_display_quality(mut commands: Commands, selected: Res<DisplayQuality>) {
    let root = menu_screen(
        &mut commands,
        "display-settings-screen",
        MenuState::SettingsDisplay,
    );
    insert_heading(
        &mut commands,
        root,
        "display-settings-title",
        "Display Quality",
    );
    for quality in [
        DisplayQuality::Low,
        DisplayQuality::Medium,
        DisplayQuality::High,
    ] {
        let button = add_button(
            &mut commands,
            root,
            format!("quality-{}-button", quality.slug()),
            quality.label(),
            ButtonIntent::Quality(quality),
            220.0,
        );
        if *selected == quality {
            commands
                .entity(button)
                .insert((SelectedSetting, BackgroundColor(SELECTED_BUTTON)));
        }
    }
    add_button(
        &mut commands,
        root,
        "display-back-button",
        "Back",
        ButtonIntent::SettingsMenu,
        220.0,
    );
}

fn choose_volume(mut commands: Commands, selected: Res<Volume>) {
    let root = menu_screen(
        &mut commands,
        "sound-settings-screen",
        MenuState::SettingsSound,
    );
    insert_heading(&mut commands, root, "sound-settings-title", "Volume");
    let row = commands
        .spawn((
            Name::new("volume-options"),
            Node {
                flex_direction: FlexDirection::Row,
                ..default()
            },
        ))
        .id();
    commands.entity(root).add_child(row);
    for value in (0..=9).map(Volume) {
        let button = add_button(
            &mut commands,
            row,
            format!("volume-{}-button", value.0),
            &value.0.to_string(),
            ButtonIntent::Volume(value),
            54.0,
        );
        if *selected == value {
            commands
                .entity(button)
                .insert((SelectedSetting, BackgroundColor(SELECTED_BUTTON)));
        }
    }
    add_button(
        &mut commands,
        root,
        "sound-back-button",
        "Back",
        ButtonIntent::SettingsMenu,
        220.0,
    );
}

fn create_screen<S: States + Copy>(
    commands: &mut Commands,
    name: &'static str,
    state: S,
    color: Color,
) -> Entity {
    let entity = commands
        .spawn((
            Name::new(name),
            DespawnOnExit(state),
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(18),
                ..default()
            },
            BackgroundColor(color),
        ))
        .id();
    mark_target(commands, entity);
    entity
}

fn menu_screen(commands: &mut Commands, name: &'static str, state: MenuState) -> Entity {
    create_screen(commands, name, state, CRIMSON.into())
}

fn insert_heading(commands: &mut Commands, parent: Entity, name: &'static str, text: &'static str) {
    let heading = commands
        .spawn((
            Name::new(name),
            Text::new(text),
            TextFont {
                font_size: FontSize::Px(58.0),
                ..default()
            },
            TextColor(TEXT_COLOR),
            Node {
                margin: UiRect::bottom(px(24)),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(parent).add_child(heading);
}

fn add_button(
    commands: &mut Commands,
    parent: Entity,
    name: impl Into<Cow<'static, str>>,
    label: &str,
    intent: ButtonIntent,
    width: f32,
) -> Entity {
    let button = commands
        .spawn((
            Name::new(name),
            Button,
            Node {
                width: px(width),
                height: px(58),
                margin: UiRect::all(px(4)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(NORMAL_BUTTON),
            intent,
        ))
        .observe(set_pressed_interaction)
        .id();
    commands.entity(button).with_child((
        Text::new(label.to_owned()),
        TextFont {
            font_size: FontSize::Px(30.0),
            ..default()
        },
        TextColor(TEXT_COLOR),
        Pickable::IGNORE,
    ));
    commands.entity(parent).add_child(button);
    mark_target(commands, button);
    button
}

fn set_pressed_interaction(
    press: On<Pointer<Press>>,
    mut buttons: Query<&mut Interaction, With<Button>>,
) {
    if let Ok(mut interaction) = buttons.get_mut(press.event_target()) {
        *interaction = Interaction::Pressed;
    }
}

fn button_colors(
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor, Has<SelectedSetting>),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, mut color, selected) in &mut buttons {
        *color = match (*interaction, selected) {
            (Interaction::Pressed, _) => SELECTED_BUTTON.into(),
            (Interaction::Hovered, _) => HOVERED_BUTTON.into(),
            (Interaction::None, true) => SELECTED_BUTTON.into(),
            (Interaction::None, false) => NORMAL_BUTTON.into(),
        };
    }
}

fn button_actions(
    buttons: Query<(&Interaction, &ButtonIntent, Entity), (Changed<Interaction>, With<Button>)>,
    mut selected: Query<(Entity, &mut BackgroundColor), With<SelectedSetting>>,
    mut commands: Commands,
    mut quality: ResMut<DisplayQuality>,
    mut volume: ResMut<Volume>,
    mut next_game: ResMut<NextState<ScreenState>>,
    mut next_menu: ResMut<NextState<MenuState>>,
    mut exit: MessageWriter<AppExit>,
) {
    for (interaction, intent, entity) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *intent {
            ButtonIntent::Play => {
                next_menu.set(MenuState::Disabled);
                next_game.set(ScreenState::Game);
            }
            ButtonIntent::Settings => next_menu.set(MenuState::Settings),
            ButtonIntent::DisplaySettings => next_menu.set(MenuState::SettingsDisplay),
            ButtonIntent::SoundSettings => next_menu.set(MenuState::SettingsSound),
            ButtonIntent::MainMenu => next_menu.set(MenuState::Main),
            ButtonIntent::SettingsMenu => next_menu.set(MenuState::Settings),
            ButtonIntent::Quality(value) => {
                quality.set_if_neq(value);
                replace_selected_setting(&mut commands, &mut selected, entity);
            }
            ButtonIntent::Volume(value) => {
                volume.set_if_neq(value);
                replace_selected_setting(&mut commands, &mut selected, entity);
            }
            ButtonIntent::Quit => {
                exit.write(AppExit::Success);
            }
        }
    }
}

fn replace_selected_setting(
    commands: &mut Commands,
    selected: &mut Query<(Entity, &mut BackgroundColor), With<SelectedSetting>>,
    replacement: Entity,
) {
    for (entity, mut color) in selected {
        *color = NORMAL_BUTTON.into();
        commands.entity(entity).remove::<SelectedSetting>();
    }
    commands.entity(replacement).insert(SelectedSetting);
}

fn keyboard_navigation(
    keys: Res<ButtonInput<KeyCode>>,
    game: Res<State<ScreenState>>,
    menu: Res<State<MenuState>>,
    mut next_game: ResMut<NextState<ScreenState>>,
    mut next_menu: ResMut<NextState<MenuState>>,
) {
    if *game.get() != ScreenState::Menu {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        match menu.get() {
            MenuState::Main => {}
            MenuState::Settings => next_menu.set(MenuState::Main),
            MenuState::SettingsDisplay | MenuState::SettingsSound => {
                next_menu.set(MenuState::Settings)
            }
            MenuState::Disabled => {}
        }
    } else if *menu.get() == MenuState::Main && keys.just_pressed(KeyCode::KeyN) {
        next_menu.set(MenuState::Disabled);
        next_game.set(ScreenState::Game);
    } else if *menu.get() == MenuState::Main && keys.just_pressed(KeyCode::KeyS) {
        next_menu.set(MenuState::Settings);
    }
}

fn update_observation(
    game: Res<State<ScreenState>>,
    menu: Res<State<MenuState>>,
    quality: Res<DisplayQuality>,
    volume: Res<Volume>,
    splash_timer: Option<Res<SplashTimer>>,
    game_timer: Option<Res<ReturnTimer>>,
    mut observation: Single<&mut SessionObservation>,
) {
    observation.game_state = *game.get();
    observation.menu_state = *menu.get();
    observation.display_quality = *quality;
    observation.volume = volume.0;
    observation.splash_elapsed_seconds = splash_timer
        .as_ref()
        .map_or(0.0, |timer| timer.elapsed_secs());
    observation.game_elapsed_seconds = game_timer
        .as_ref()
        .map_or(0.0, |timer| timer.elapsed_secs());
}

#[cfg(feature = "automation")]
fn mark_target(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).insert(AutomationTarget);
}

#[cfg(not(feature = "automation"))]
fn mark_target(_commands: &mut Commands, _entity: Entity) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn build_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .insert_resource(Time::<()>::default())
            .insert_resource(ButtonInput::<KeyCode>::default())
            .init_resource::<Messages<AppExit>>();
        add_game_menu(&mut app);
        app.update();
        app
    }

    fn advance(app: &mut App, seconds: f32) {
        app.world_mut()
            .resource_mut::<Time<()>>()
            .advance_by(Duration::from_secs_f32(seconds));
        app.update();
    }

    fn apply_transitions(app: &mut App) {
        advance(app, 0.0);
        advance(app, 0.0);
    }

    fn entity_by_name(app: &mut App, name: &str) -> Entity {
        app.world_mut()
            .query::<(Entity, &Name)>()
            .iter(app.world())
            .find_map(|(entity, candidate)| (candidate.as_str() == name).then_some(entity))
            .unwrap_or_else(|| panic!("entity {name:?} not found"))
    }

    fn press_button(app: &mut App, name: &str) -> Entity {
        let entity = entity_by_name(app, name);
        *app.world_mut().get_mut::<Interaction>(entity).unwrap() = Interaction::Pressed;
        app.update();
        apply_transitions(app);
        entity
    }

    mod game_menu {
        use super::*;

        #[test]
        fn controlled_time_drives_splash_and_game_timers() {
            let mut app = build_test_app();
            assert_eq!(
                *app.world().resource::<State<ScreenState>>().get(),
                ScreenState::Splash
            );
            advance(&mut app, 0.5);
            assert_eq!(
                *app.world().resource::<State<ScreenState>>().get(),
                ScreenState::Splash
            );
            advance(&mut app, 0.5);
            apply_transitions(&mut app);
            assert_eq!(
                *app.world().resource::<State<ScreenState>>().get(),
                ScreenState::Menu
            );

            press_button(&mut app, "new-game-button");
            assert_eq!(
                *app.world().resource::<State<ScreenState>>().get(),
                ScreenState::Game
            );
            advance(&mut app, 4.9);
            assert_eq!(
                *app.world().resource::<State<ScreenState>>().get(),
                ScreenState::Game
            );
            advance(&mut app, 0.1);
            apply_transitions(&mut app);
            assert_eq!(
                *app.world().resource::<State<ScreenState>>().get(),
                ScreenState::Menu
            );
        }

        #[test]
        fn navigation_updates_settings_and_despawns_old_screens() {
            let mut app = build_test_app();
            advance(&mut app, SPLASH_SECONDS);
            apply_transitions(&mut app);
            let main_button = press_button(&mut app, "settings-button");
            assert!(app.world().get_entity(main_button).is_err());

            press_button(&mut app, "display-settings-button");
            let medium = entity_by_name(&mut app, "quality-medium-button");
            let high = press_button(&mut app, "quality-high-button");
            assert_eq!(
                *app.world().resource::<DisplayQuality>(),
                DisplayQuality::High
            );
            assert_eq!(
                app.world().get::<BackgroundColor>(medium).unwrap().0,
                NORMAL_BUTTON
            );
            assert_eq!(
                app.world().get::<BackgroundColor>(high).unwrap().0,
                SELECTED_BUTTON
            );
            press_button(&mut app, "display-back-button");
            press_button(&mut app, "sound-settings-button");
            press_button(&mut app, "volume-3-button");
            assert_eq!(*app.world().resource::<Volume>(), Volume(3));
            press_button(&mut app, "sound-back-button");

            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Escape);
            app.update();
            apply_transitions(&mut app);
            assert_eq!(
                *app.world().resource::<State<MenuState>>().get(),
                MenuState::Main
            );
        }
    }
}
