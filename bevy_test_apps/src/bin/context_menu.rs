//! Adapted from Bevy's `examples/usage/context_menu.rs`; see the package README for provenance.

use bevy::{
    color::palettes::basic,
    ecs::{relationship::RelatedSpawner, spawn::SpawnWith},
    prelude::*,
    text::{EditableText, TextCursorStyle},
    window::WindowResolution,
};
use bevy_test_apps::composition;
use std::fmt::Debug;

#[cfg(feature = "automation")]
use automation_control::AutomationTarget;

/// Event opening a new context menu at a pointer position.
#[derive(Event)]
struct OpenContextMenu {
    pos: Vec2,
}

/// Event closing all currently open context menus.
#[derive(Event)]
struct CloseContextMenus;

#[derive(Component)]
struct Background;

#[derive(Component)]
struct ContextMenu;

#[derive(Component)]
struct ContextMenuItem {
    name: &'static str,
    color: Srgba,
}

#[derive(Component)]
struct DummyTextInput;

/// Small reflected application state used by the observation smoke test.
#[derive(Component, Reflect)]
#[reflect(Component)]
struct SessionState {
    menu_open: bool,
    selected_item: String,
    key_a_held: bool,
    key_a_presses: u32,
    key_a_releases: u32,
    text: String,
}

fn main() {
    let mut app = App::new();
    composition::rendered(
        &mut app,
        Window {
            title: "Controlled context menu test".into(),
            resolution: WindowResolution::new(640, 360).with_scale_factor_override(1.0),
            resizable: false,
            ..default()
        },
    );

    app.register_type::<SessionState>()
        .add_systems(Startup, setup)
        .add_systems(Update, observe_keyboard_and_text)
        .add_observer(on_trigger_menu)
        .add_observer(on_trigger_close_menus)
        .add_observer(text_color_on_hover::<Out>(basic::WHITE.into()))
        .add_observer(text_color_on_hover::<Over>(basic::RED.into()))
        .run();
}

fn text_color_on_hover<T: Debug + Clone + Reflect>(
    color: Color,
) -> impl FnMut(On<Pointer<T>>, Query<&mut TextColor>, Query<&Children>) {
    move |mut event: On<Pointer<T>>,
          mut text_color: Query<&mut TextColor>,
          children: Query<&Children>| {
        let Ok(children) = children.get(event.original_event_target()) else {
            return;
        };
        event.propagate(false);
        for child in children.iter() {
            if let Ok(mut col) = text_color.get_mut(child) {
                col.0 = color;
            }
        }
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands
        .spawn((
            Name::new("background"),
            Background,
            SessionState {
                menu_open: false,
                selected_item: "none".into(),
                key_a_held: false,
                key_a_presses: 0,
                key_a_releases: 0,
                text: String::new(),
            },
            #[cfg(feature = "automation")]
            AutomationTarget,
            background_and_button(),
        ))
        .observe(|_: On<Pointer<Press>>, mut commands: Commands| {
            commands.trigger(CloseContextMenus);
        });
}

fn observe_keyboard_and_text(
    keys: Res<ButtonInput<KeyCode>>,
    input: Single<&EditableText, With<DummyTextInput>>,
    mut state: Single<&mut SessionState, With<Background>>,
) {
    state.key_a_held = keys.pressed(KeyCode::KeyA);
    if keys.just_pressed(KeyCode::KeyA) {
        state.key_a_presses += 1;
    }
    if keys.just_released(KeyCode::KeyA) {
        state.key_a_releases += 1;
    }
    state.text = input.value().to_string();
}

fn on_trigger_close_menus(
    _event: On<CloseContextMenus>,
    mut commands: Commands,
    menus: Query<Entity, With<ContextMenu>>,
    mut state: Query<&mut SessionState, With<Background>>,
) {
    for entity in menus.iter() {
        commands.entity(entity).despawn();
    }
    if let Ok(mut state) = state.single_mut() {
        state.menu_open = false;
    }
}

fn on_trigger_menu(
    event: On<OpenContextMenu>,
    mut commands: Commands,
    mut state: Query<&mut SessionState, With<Background>>,
) {
    commands.trigger(CloseContextMenus);
    if let Ok(mut state) = state.single_mut() {
        state.menu_open = true;
    }

    let pos = event.pos;
    commands
        .spawn((
            Name::new("context-menu"),
            ContextMenu,
            Node {
                position_type: PositionType::Absolute,
                left: px(pos.x),
                top: px(pos.y),
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BorderColor::all(Color::BLACK),
            BackgroundColor(Color::linear_rgb(0.1, 0.1, 0.1)),
            children![
                context_item("fuchsia", basic::FUCHSIA),
                context_item("gray", basic::GRAY),
                context_item("maroon", basic::MAROON),
                context_item("purple", basic::PURPLE),
                context_item("teal", basic::TEAL),
            ],
        ))
        .observe(
            |event: On<Pointer<Press>>,
             menu_items: Query<&ContextMenuItem>,
             mut clear_col: ResMut<ClearColor>,
             mut state: Query<&mut SessionState, With<Background>>,
             mut commands: Commands| {
                let target = event.original_event_target();
                if let Ok(item) = menu_items.get(target) {
                    clear_col.0 = item.color.into();
                    if let Ok(mut state) = state.single_mut() {
                        state.selected_item = item.name.into();
                        state.menu_open = false;
                    }
                    commands.trigger(CloseContextMenus);
                }
            },
        );
}

fn context_item(text: &'static str, color: Srgba) -> impl Bundle {
    (
        Name::new(format!("item-{text}")),
        ContextMenuItem { name: text, color },
        #[cfg(feature = "automation")]
        AutomationTarget,
        Button,
        Node {
            padding: UiRect::all(px(5)),
            ..default()
        },
        children![(
            Pickable::IGNORE,
            Text::new(text),
            TextFont {
                font_size: FontSize::Px(24.0),
                ..default()
            },
            TextColor(Color::WHITE),
        )],
    )
}

fn background_and_button() -> impl Bundle {
    (
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: px(20),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        ZIndex(-10),
        Children::spawn(SpawnWith(|parent: &mut RelatedSpawner<ChildOf>| {
            parent
                .spawn((
                    Name::new("button"),
                    #[cfg(feature = "automation")]
                    AutomationTarget,
                    Button,
                    Node {
                        width: px(250),
                        height: px(65),
                        border: UiRect::all(px(5)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BorderColor::all(Color::BLACK),
                    BackgroundColor(Color::BLACK),
                    children![(
                        Pickable::IGNORE,
                        Text::new("Context Menu"),
                        TextFont {
                            font_size: FontSize::Px(28.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        TextShadow::default(),
                    )],
                ))
                .observe(|mut event: On<Pointer<Press>>, mut commands: Commands| {
                    event.propagate(false);
                    debug!("click: {}", event.pointer_location.position);
                    commands.trigger(OpenContextMenu {
                        pos: event.pointer_location.position,
                    });
                });
            parent.spawn((
                Name::new("text-input"),
                DummyTextInput,
                #[cfg(feature = "automation")]
                AutomationTarget,
                EditableText {
                    visible_width: Some(20.0),
                    allow_newlines: false,
                    ..default()
                },
                TextCursorStyle::default(),
                TextLayout::no_wrap(),
                TextFont {
                    font_size: FontSize::Px(24.0),
                    ..default()
                },
                Node {
                    width: px(250),
                    min_height: px(45),
                    border: UiRect::all(px(2)),
                    padding: UiRect::all(px(8)),
                    ..default()
                },
                BorderColor::all(Color::WHITE),
                BackgroundColor(Color::linear_rgb(0.08, 0.08, 0.08)),
            ));
        })),
    )
}
