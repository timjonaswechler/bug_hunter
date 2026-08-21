//! A 3D scene for exercising Bevy mesh picking.
use bevy::{
    color::palettes::tailwind::*,
    picking::hover::PickingInteraction,
    prelude::*,
    window::{Window, WindowResolution},
};

#[cfg(feature = "automation")]
use automation_control::AutomationTarget;

#[derive(Component)]
struct RotatingMesh;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Reflect)]
enum MeshEvent {
    #[default]
    Idle,
    Hover,
    Out,
    Press,
    Release,
    Drag,
}

#[derive(Clone, Debug, Default, PartialEq, Component, Reflect)]
#[reflect(Component)]
struct MeshInteractionState {
    last_interaction: MeshEvent,
    drag_events: u32,
}

struct MaterialPalette {
    idle: Handle<StandardMaterial>,
    hover: Handle<StandardMaterial>,
    pressed: Handle<StandardMaterial>,
}

fn main() {
    let mut app = App::new();
    bevy_test_apps::composition::rendered(
        &mut app,
        Window {
            title: "Mesh picking test".into(),
            resolution: WindowResolution::new(640, 360).with_scale_factor_override(1.0),
            resizable: false,
            ..default()
        },
    );
    app.add_plugins(MeshPickingPlugin)
        .register_type::<Transform>()
        .register_type::<Pickable>()
        .register_type::<PickingInteraction>()
        .register_type::<MeshInteractionState>()
        .add_systems(Startup, setup_scene)
        .add_systems(Update, rotate_meshes)
        .run();
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let interaction_materials = MaterialPalette {
        idle: materials.add(Color::WHITE),
        hover: materials.add(Color::from(CYAN_300)),
        pressed: materials.add(Color::from(YELLOW_300)),
    };
    spawn_test_mesh(
        &mut commands,
        "center-cube",
        meshes.add(Cuboid::from_length(2.0)),
        Transform::from_xyz(0.0, 1.0, 0.0),
        &interaction_materials,
    );
    spawn_test_mesh(
        &mut commands,
        "left-sphere",
        meshes.add(Sphere::new(1.0).mesh().ico(3).unwrap()),
        Transform::from_xyz(-3.0, 1.0, 0.0),
        &interaction_materials,
    );
    spawn_test_mesh(
        &mut commands,
        "right-cylinder",
        meshes.add(Cylinder::new(0.8, 2.0)),
        Transform::from_xyz(3.0, 1.0, 0.0),
        &interaction_materials,
    );

    commands.spawn((
        Name::new("ground"),
        Mesh3d(meshes.add(Plane3d::default().mesh().size(20.0, 20.0))),
        MeshMaterial3d(materials.add(Color::from(GRAY_300))),
        Pickable::IGNORE,
    ));
    commands.spawn((
        Name::new("scene-light"),
        PointLight {
            intensity: 4_000_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
    commands.spawn((
        Name::new("scene-camera"),
        Camera3d::default(),
        Transform::from_xyz(0.0, 4.0, 10.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
    ));
    commands.spawn((
        Text::new("Hover, press, and drag the meshes"),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(12),
            ..default()
        },
    ));
}

fn spawn_test_mesh(
    commands: &mut Commands,
    name: &'static str,
    mesh: Handle<Mesh>,
    transform: Transform,
    materials: &MaterialPalette,
) {
    let entity = commands
        .spawn((
            Name::new(name),
            Mesh3d(mesh),
            MeshMaterial3d(materials.idle.clone()),
            transform,
            Pickable::default(),
            PickingInteraction::default(),
            RotatingMesh,
            MeshInteractionState::default(),
        ))
        .observe(update_on::<Pointer<Over>>(
            materials.hover.clone(),
            MeshEvent::Hover,
        ))
        .observe(update_on::<Pointer<Out>>(
            materials.idle.clone(),
            MeshEvent::Out,
        ))
        .observe(update_on::<Pointer<Press>>(
            materials.pressed.clone(),
            MeshEvent::Press,
        ))
        .observe(update_on::<Pointer<Release>>(
            materials.hover.clone(),
            MeshEvent::Release,
        ))
        .observe(rotate_on_drag)
        .id();
    mark_automation_target(commands, entity);
}

fn update_on<E: EntityEvent>(
    material: Handle<StandardMaterial>,
    interaction: MeshEvent,
) -> impl Fn(
    On<E>,
    Query<(
        &mut MeshMaterial3d<StandardMaterial>,
        &mut MeshInteractionState,
    )>,
) {
    move |event, mut meshes| {
        if let Ok((mut current_material, mut state)) = meshes.get_mut(event.event_target()) {
            current_material.0 = material.clone();
            state.last_interaction = interaction;
        }
    }
}

fn rotate_on_drag(
    drag: On<Pointer<Drag>>,
    mut meshes: Query<(&mut Transform, &mut MeshInteractionState)>,
) {
    if let Ok((mut transform, mut state)) = meshes.get_mut(drag.entity) {
        transform.rotate_y(drag.delta.x * 0.02);
        transform.rotate_x(drag.delta.y * 0.02);
        state.last_interaction = MeshEvent::Drag;
        state.drag_events += 1;
    }
}

fn rotate_meshes(mut meshes: Query<&mut Transform, With<RotatingMesh>>, time: Res<Time>) {
    for mut transform in &mut meshes {
        transform.rotate_y(time.delta_secs() / 2.0);
    }
}

#[cfg(feature = "automation")]
fn mark_automation_target(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).insert(AutomationTarget);
}

#[cfg(not(feature = "automation"))]
fn mark_automation_target(_commands: &mut Commands, _entity: Entity) {}
