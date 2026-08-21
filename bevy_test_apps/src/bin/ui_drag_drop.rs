//! A compact UI grid for exercising Bevy drag-and-drop picking.
use bevy::{
    color::palettes::tailwind::{AMBER_500, BLUE_500, GREEN_500, ROSE_500},
    prelude::*,
    window::{Window, WindowResolution},
};

#[cfg(feature = "automation")]
use automation_control::AutomationTarget;

const TILE_SIZE: f32 = 120.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Reflect)]
enum TileId {
    Amber,
    Blue,
    Green,
    Rose,
}

impl TileId {
    fn metadata(self) -> (&'static str, &'static str) {
        match self {
            Self::Amber => ("tile-amber", "AMBER"),
            Self::Blue => ("tile-blue", "BLUE"),
            Self::Green => ("tile-green", "GREEN"),
            Self::Rose => ("tile-rose", "ROSE"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Reflect)]
enum DragPhase {
    DragStart,
    Drag,
    DragDrop,
    DragEnd,
}

#[derive(Clone, Debug, Component, Reflect)]
#[reflect(Component)]
struct SceneState {
    active_tile: Option<TileId>,
    hover_events: u32,
    drag_start_events: u32,
    drag_events: u32,
    drag_drop_events: u32,
    drag_end_events: u32,
    drag_sequence: Vec<DragPhase>,
    occupancy: Vec<TileId>,
}

impl Default for SceneState {
    fn default() -> Self {
        Self {
            active_tile: None,
            hover_events: 0,
            drag_start_events: 0,
            drag_events: 0,
            drag_drop_events: 0,
            drag_end_events: 0,
            drag_sequence: Vec::new(),
            occupancy: vec![TileId::Amber, TileId::Blue, TileId::Green, TileId::Rose],
        }
    }
}

#[derive(Clone, Copy, Component)]
struct Tile(TileId);

fn main() {
    let mut app = App::new();
    bevy_test_apps::composition::rendered(
        &mut app,
        Window {
            title: "UI drag and drop test".into(),
            resolution: WindowResolution::new(640, 480).with_scale_factor_override(1.0),
            resizable: false,
            ..default()
        },
    );
    app.register_type::<Node>()
        .register_type::<UiTransform>()
        .register_type::<GlobalZIndex>()
        .register_type::<Outline>()
        .register_type::<TileId>()
        .register_type::<DragPhase>()
        .register_type::<SceneState>()
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((Name::new("ui-camera"), Camera2d));
    commands
        .spawn((
            Name::new("ui-root"),
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(16),
                ..default()
            },
            BackgroundColor(Color::srgb(0.06, 0.07, 0.1)),
            Pickable::IGNORE,
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("Drag a tile onto another tile"),
                TextFont::from_font_size(24.0),
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
            let grid = root
                .spawn((
                    Name::new("drag-grid"),
                    Node {
                        display: Display::Grid,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.18, 0.2, 0.25)),
                    Pickable::IGNORE,
                    SceneState::default(),
                ))
                .id();
            mark_automation_target(&mut root.commands(), grid);
            root.commands().entity(grid).with_children(|grid| {
                for (index, tile, color) in [
                    (0, TileId::Amber, Color::from(AMBER_500)),
                    (1, TileId::Blue, Color::from(BLUE_500)),
                    (2, TileId::Green, Color::from(GREEN_500)),
                    (3, TileId::Rose, Color::from(ROSE_500)),
                ] {
                    spawn_tile(grid, index, tile, color);
                }
            });
        });
}

fn spawn_tile(parent: &mut ChildSpawnerCommands, index: i16, tile: TileId, color: Color) {
    let row = index / 2 + 1;
    let column = index % 2 + 1;
    let border = color.darker(0.12);
    let (name, label) = tile.metadata();
    let entity = parent
        .spawn((
            Name::new(name),
            Tile(tile),
            Node {
                width: px(TILE_SIZE),
                height: px(TILE_SIZE),
                border: px(5).all(),
                grid_row: GridPlacement::start(row),
                grid_column: GridPlacement::start(column),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BorderColor::all(border),
            BackgroundColor(color),
            Outline {
                width: px(3),
                offset: px(2),
                color: Color::NONE,
            },
            Pickable {
                should_block_lower: false,
                is_hoverable: true,
            },
            GlobalZIndex::default(),
        ))
        .observe(
            move |event: On<Pointer<Over>>,
                  mut tiles: Query<&mut BackgroundColor>,
                  mut states: Query<&mut SceneState>| {
                if let Ok(mut background) = tiles.get_mut(event.event_target()) {
                    background.0 = color.lighter(0.12);
                }
                if let Ok(mut state) = states.single_mut() {
                    state.hover_events += 1;
                }
            },
        )
        .observe(
            move |event: On<Pointer<Out>>, mut tiles: Query<&mut BackgroundColor>| {
                if let Ok(mut background) = tiles.get_mut(event.event_target()) {
                    background.0 = color;
                }
            },
        )
        .observe(drag::start)
        .observe(drag::update)
        .observe(drag::end)
        .observe(drag::drop)
        .with_child((
            Text::new(label),
            TextFont::from_font_size(20.0),
            TextColor(Color::WHITE),
            Pickable::IGNORE,
        ))
        .id();
    mark_automation_target(&mut parent.commands(), entity);
}

mod drag {
    use super::*;

    pub(super) fn start(
        event: On<Pointer<DragStart>>,
        mut tiles: Query<(&Tile, &mut Outline, &mut GlobalZIndex)>,
        mut states: Query<&mut SceneState>,
    ) {
        let Ok((tile, mut outline, mut z_index)) = tiles.get_mut(event.event_target()) else {
            return;
        };
        outline.color = Color::WHITE;
        z_index.0 = 1;
        if let Ok(mut state) = states.single_mut() {
            state.active_tile = Some(tile.0);
            state.drag_start_events += 1;
            state.drag_sequence.push(DragPhase::DragStart);
        }
    }

    pub(super) fn update(
        event: On<Pointer<Drag>>,
        mut transforms: Query<&mut UiTransform, With<Tile>>,
        mut states: Query<&mut SceneState>,
    ) {
        let Ok(mut transform) = transforms.get_mut(event.event_target()) else {
            return;
        };
        transform.translation = Val2::px(event.distance.x, event.distance.y);
        if let Ok(mut state) = states.single_mut() {
            state.drag_events += 1;
            state.drag_sequence.push(DragPhase::Drag);
        }
    }

    pub(super) fn drop(
        event: On<Pointer<DragDrop>>,
        mut tiles: Query<(&Tile, &mut Node)>,
        mut states: Query<&mut SceneState>,
    ) {
        let destination = event.event_target();
        let source = event.dropped;
        if source == destination {
            return;
        }
        let Ok(
            [
                (source_tile, mut source_node),
                (destination_tile, mut destination_node),
            ],
        ) = tiles.get_many_mut([source, destination])
        else {
            return;
        };
        let source_id = source_tile.0;
        let destination_id = destination_tile.0;
        core::mem::swap(&mut source_node.grid_row, &mut destination_node.grid_row);
        core::mem::swap(
            &mut source_node.grid_column,
            &mut destination_node.grid_column,
        );
        if let Ok(mut state) = states.single_mut() {
            let source_slot = state.occupancy.iter().position(|tile| *tile == source_id);
            let destination_slot = state
                .occupancy
                .iter()
                .position(|tile| *tile == destination_id);
            if let (Some(source_slot), Some(destination_slot)) = (source_slot, destination_slot) {
                state.occupancy.swap(source_slot, destination_slot);
                state.drag_drop_events += 1;
                state.drag_sequence.push(DragPhase::DragDrop);
            }
        }
    }

    pub(super) fn end(
        event: On<Pointer<DragEnd>>,
        mut tiles: Query<(&mut UiTransform, &mut Outline, &mut GlobalZIndex), With<Tile>>,
        mut states: Query<&mut SceneState>,
    ) {
        let Ok((mut transform, mut outline, mut z_index)) = tiles.get_mut(event.event_target())
        else {
            return;
        };
        transform.translation = Val2::ZERO;
        outline.color = Color::NONE;
        z_index.0 = 0;
        if let Ok(mut state) = states.single_mut() {
            state.active_tile = None;
            state.drag_end_events += 1;
            state.drag_sequence.push(DragPhase::DragEnd);
        }
    }
}

#[cfg(feature = "automation")]
fn mark_automation_target(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).insert(AutomationTarget);
}

#[cfg(not(feature = "automation"))]
fn mark_automation_target(_commands: &mut Commands, _entity: Entity) {}
