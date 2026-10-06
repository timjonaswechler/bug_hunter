//! Keep the render-world bootstrap behind the first completed explicit tick.
//!
//! Startup may create active cameras whose derived render data is still unprepared:
//! application PostUpdate has not run yet. Neither extraction nor rendering may
//! consume that data. Once a tick completes, the original render path resumes,
//! including during subsequent idle control iterations. No main-world camera or
//! application schedule is changed here. The gate travels with RenderApp when
//! Bevy moves it to the pipelined rendering thread.
use bevy::{app::App, ecs::schedule::ScheduleLabel, prelude::*, render::RenderApp};

#[derive(Resource)]
struct Installed(bool);

#[derive(Resource, Default)]
struct Ready(bool);

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct Bootstrap;

pub(super) fn install(app: &mut App) {
    let Some(render) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    let mut extract = render.take_extract();
    let schedule = render.update_schedule;
    render.init_resource::<Ready>();
    render.set_extract(move |main, render| {
        let ready = main.resource::<Installed>().0;
        render.resource_mut::<Ready>().0 = ready;
        if ready && let Some(extract) = extract.as_mut() {
            extract(main, render);
        }
    });
    render.add_systems(Bootstrap, move |world: &mut World| {
        if world.resource::<Ready>().0
            && let Some(schedule) = schedule
        {
            world.run_schedule(schedule);
        }
    });
    render.update_schedule = Some(Bootstrap.intern());
    app.insert_resource(Installed(false));
}

pub(super) fn start(world: &mut World) {
    if let Some(mut installed) = world.get_resource_mut::<Installed>() {
        installed.0 = true;
    }
}

#[cfg(feature = "screenshot")]
pub(super) fn waiting(world: &World) -> bool {
    world
        .get_resource::<Installed>()
        .is_some_and(|installed| !installed.0)
}
