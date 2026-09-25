//! Fixed 2D headless fixture for CLI -> Session -> Capture acceptance.
#[path = "headless_session/diagnostics.rs"]
mod diagnostics;

use bevy::{
    app::ScheduleRunnerPlugin,
    camera::{ImageRenderTarget, RenderTarget},
    prelude::*,
    render::render_resource::TextureFormat,
    time::TimeUpdateStrategy,
    window::ExitCondition,
};
use std::time::Duration;

const WIDTH: u32 = 321;
const HEIGHT: u32 = 181;
const SCALE: f32 = 1.5;
const SPRITE_STEP_PER_TICK: f32 = 4.0;

#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]
struct SceneState {
    ticks: u64,
    virtual_millis: u64,
    camera_translation: [f32; 3],
    sprite_translation: [f32; 3],
}

#[derive(Resource, Reflect)]
#[reflect(Resource)]
struct ImageTargetInfo {
    physical_width: u32,
    physical_height: u32,
    scale_factor: f32,
    sprite_step_per_tick: f32,
}

#[derive(Component)]
struct GameCamera;

#[derive(Component)]
struct WorldSprite;

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let image = images.add(Image::new_target_texture(
        WIDTH,
        HEIGHT,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    let camera = commands
        .spawn((
            Name::new("headless-camera"),
            Camera2d,
            Msaa::Off,
            RenderTarget::Image(ImageRenderTarget {
                handle: image,
                scale_factor: SCALE,
            }),
            GameCamera,
            woodpecker::session::HeadlessCaptureCamera2d,
        ))
        .id();
    commands.spawn((
        Name::new("red-world-sprite"),
        Sprite::from_color(Color::srgb(1.0, 0.0, 0.0), Vec2::splat(60.0)),
        Transform::default(),
        WorldSprite,
    ));
    commands.spawn((
        Name::new("green-ui-overlay"),
        Node {
            position_type: PositionType::Absolute,
            left: px(8.0),
            top: px(8.0),
            width: px(48.0),
            height: px(24.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.0, 1.0, 0.0)),
        UiTargetCamera(camera),
    ));
}

fn move_sprite(
    keys: Res<ButtonInput<KeyCode>>,
    mut sprite: Single<&mut Transform, With<WorldSprite>>,
) {
    if keys.pressed(KeyCode::KeyD) {
        sprite.translation.x += SPRITE_STEP_PER_TICK;
    }
}

fn observe_scene(
    time: Res<Time<Virtual>>,
    camera: Single<&Transform, With<GameCamera>>,
    sprite: Single<&Transform, (With<WorldSprite>, Without<GameCamera>)>,
    mut state: ResMut<SceneState>,
) {
    state.ticks += 1;
    state.virtual_millis = time.elapsed().as_millis() as u64;
    state.camera_translation = camera.translation.to_array();
    state.sprite_translation = sprite.translation.to_array();
}

fn assert_no_window(world: &mut World) {
    assert_eq!(world.query::<&Window>().iter(world).count(), 0);
}

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                })
                .disable::<bevy::winit::WinitPlugin>()
                .disable::<bevy::audio::AudioPlugin>()
                .disable::<bevy::gilrs::GilrsPlugin>(),
        )
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_millis(1)))
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            20,
        )))
        .init_resource::<SceneState>()
        .insert_resource(ImageTargetInfo {
            physical_width: WIDTH,
            physical_height: HEIGHT,
            scale_factor: SCALE,
            sprite_step_per_tick: SPRITE_STEP_PER_TICK,
        })
        .register_type::<SceneState>()
        .register_type::<ImageTargetInfo>()
        .add_systems(Startup, setup)
        .add_systems(PostStartup, assert_no_window)
        .add_systems(Update, (move_sprite, observe_scene).chain())
        .add_plugins(diagnostics::Plugin)
        .add_plugins(woodpecker::session::Plugin)
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_d_moves_the_fixture_once_per_explicit_update() {
        let mut app = App::new();
        app.insert_resource(ButtonInput::<KeyCode>::default())
            .add_systems(Update, move_sprite);
        let sprite = app
            .world_mut()
            .spawn((Transform::default(), WorldSprite))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<Transform>(sprite).unwrap().translation.x,
            0.0
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyD);
        app.update();
        app.update();
        assert_eq!(
            app.world().get::<Transform>(sprite).unwrap().translation.x,
            2.0 * SPRITE_STEP_PER_TICK
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::KeyD);
        app.update();
        assert_eq!(
            app.world().get::<Transform>(sprite).unwrap().translation.x,
            2.0 * SPRITE_STEP_PER_TICK
        );
    }
}
