//! Pure selection rules for the opt-in fixed image targets.
use bevy::prelude::Entity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ImagePipeline {
    TwoD,
    ThreeD,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Choice {
    Window(Entity),
    Image,
    Unavailable,
}

pub(super) fn choose(window: Option<Entity>, image_configured: bool) -> Choice {
    match (window, image_configured) {
        (Some(window), _) => Choice::Window(window),
        (None, true) => Choice::Image,
        (None, false) => Choice::Unavailable,
    }
}

pub(super) fn fixed_2d_camera_supported(
    marked: bool,
    camera_2d: bool,
    active: bool,
    full_viewport: bool,
    writes_output: bool,
    valid_scale: bool,
    dimensions_match: bool,
) -> bool {
    marked
        && camera_2d
        && active
        && full_viewport
        && writes_output
        && valid_scale
        && dimensions_match
}

#[derive(Clone, Copy)]
pub(super) struct Fixed3dCamera {
    pub marked: bool,
    pub camera_3d: bool,
    pub perspective: bool,
    pub active: bool,
    pub full_viewport: bool,
    pub writes_output: bool,
    pub valid_scale: bool,
    pub dimensions_match: bool,
    pub valid_projection: bool,
}

pub(super) fn fixed_3d_camera_supported(camera: Fixed3dCamera) -> bool {
    camera.marked
        && camera.camera_3d
        && camera.perspective
        && camera.active
        && camera.full_viewport
        && camera.writes_output
        && camera.valid_scale
        && camera.dimensions_match
        && camera.valid_projection
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendered_window_wins_even_with_a_configured_image_camera() {
        let window = Entity::from_bits(41);
        assert_eq!(choose(Some(window), true), Choice::Window(window));
    }

    #[test]
    fn image_requires_explicit_configuration_and_every_2d_constraint() {
        assert_eq!(choose(None, false), Choice::Unavailable);
        assert_eq!(choose(None, true), Choice::Image);
        assert!(fixed_2d_camera_supported(
            true, true, true, true, true, true, true
        ));
        for unsupported in 0..7 {
            let mut conditions = [true; 7];
            conditions[unsupported] = false;
            assert!(!fixed_2d_camera_supported(
                conditions[0],
                conditions[1],
                conditions[2],
                conditions[3],
                conditions[4],
                conditions[5],
                conditions[6],
            ));
        }
    }

    #[test]
    fn fixed_3d_requires_explicit_perspective_and_every_target_constraint() {
        let supported = Fixed3dCamera {
            marked: true,
            camera_3d: true,
            perspective: true,
            active: true,
            full_viewport: true,
            writes_output: true,
            valid_scale: true,
            dimensions_match: true,
            valid_projection: true,
        };
        assert!(fixed_3d_camera_supported(supported));
        for unsupported in [
            Fixed3dCamera {
                marked: false,
                ..supported
            },
            Fixed3dCamera {
                camera_3d: false,
                ..supported
            },
            Fixed3dCamera {
                perspective: false,
                ..supported
            },
            Fixed3dCamera {
                active: false,
                ..supported
            },
            Fixed3dCamera {
                full_viewport: false,
                ..supported
            },
            Fixed3dCamera {
                writes_output: false,
                ..supported
            },
            Fixed3dCamera {
                valid_scale: false,
                ..supported
            },
            Fixed3dCamera {
                dimensions_match: false,
                ..supported
            },
            Fixed3dCamera {
                valid_projection: false,
                ..supported
            },
        ] {
            assert!(!fixed_3d_camera_supported(unsupported));
        }
    }
}
