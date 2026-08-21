//! Optional PNG screenshot artifacts for Rendered Mode Controlled Sessions.
//!
//! [`Plugin`] advertises the capability only when Bevy rendering and screenshot resources already
//! exist. Artifact paths are sandboxed beneath a session artifact root and checked against absolute
//! paths, traversal, and symbolic-link escape.

use crate::artifact_root_path;
use bevy::{
    prelude::*,
    render::{
        RenderApp,
        view::window::screenshot::{CapturedScreenshots, Screenshot, ScreenshotCaptured},
    },
    window::PrimaryWindow,
};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};
use std::{
    fmt, fs,
    io::{Cursor, Read, Write},
    path::{Component, Path, PathBuf},
};

/// Capability name advertised in [`crate::Ready::controls`].
pub const CONTROL_NAME: &str = "screenshot";
/// MIME type reported for completed screenshot artifacts.
pub const MIME_TYPE: &str = "image/png";

#[cfg(test)]
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Requests capture of the primary rendered window into a session artifact.
///
/// Constructing or validating this value does not establish renderer capability or capture
/// success; execution occurs later through [`Plugin`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    /// Relative forward-slash `.png` path below the session artifact root.
    pub path: String,
}

impl Command {
    /// Creates a screenshot request without validating its path.
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }

    /// Validates lexical artifact-path rules only.
    ///
    /// Paths must be relative UTF-8 forward-slash paths ending in `.png`, with no empty, `.`, or
    /// `..` components.
    ///
    /// ```
    /// use automation_control::screenshot::Command;
    /// assert!(Command::new("captures/current.png").validate().is_ok());
    /// assert!(Command::new("../outside.png").validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<(), Error> {
        validate_relative_path(Path::new(&self.path)).map(|_| ())
    }
}

/// Screenshot validation, capability, sandbox, I/O, or capture failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// The composition did not register screenshot capability.
    CapabilityUnavailable,
    /// The requested artifact path is empty.
    EmptyPath,
    /// The requested artifact path is absolute.
    AbsolutePath,
    /// The path contains `.` or `..` traversal components.
    PathTraversal,
    /// The path violates another UTF-8, separator, component, or extension rule.
    InvalidPath(String),
    /// A target path crosses a symbolic link.
    SymlinkEscape(PathBuf),
    /// Canonical containment would leave the configured artifact root.
    OutsideArtifactRoot(PathBuf),
    /// Artifact-directory or PNG-writing I/O failed.
    Io(String),
    /// Renderer capture setup or completion failed.
    Capture(String),
}

impl Error {
    /// Returns the stable protocol error code.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::CapabilityUnavailable => "screenshot_capability_unavailable",
            Self::EmptyPath | Self::InvalidPath(_) => "invalid_artifact_path",
            Self::AbsolutePath => "absolute_artifact_path",
            Self::PathTraversal => "artifact_path_traversal",
            Self::SymlinkEscape(_) => "artifact_symlink_escape",
            Self::OutsideArtifactRoot(_) => "artifact_outside_root",
            Self::Io(_) => "artifact_io_failed",
            Self::Capture(_) => "screenshot_capture_failed",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapabilityUnavailable => {
                formatter.write_str("screenshot capture is not installed in this composition")
            }
            Self::EmptyPath => formatter.write_str("artifact path must not be empty"),
            Self::AbsolutePath => formatter.write_str("artifact path must be relative"),
            Self::PathTraversal => {
                formatter.write_str("artifact path must not contain '.' or '..'")
            }
            Self::InvalidPath(message) => formatter.write_str(message),
            Self::SymlinkEscape(path) => write!(
                formatter,
                "artifact path crosses symbolic link {}",
                path.display()
            ),
            Self::OutsideArtifactRoot(path) => write!(
                formatter,
                "artifact target {} is outside the session artifact root",
                path.display()
            ),
            Self::Io(message) => formatter.write_str(message),
            Self::Capture(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for Error {}

/// Installs screenshot capture for a rendered Controlled Session.
///
/// Adding this plugin does not install Bevy's renderer. The capability is registered only when
/// the composition already contains `RenderApp` and Bevy's screenshot resources.
pub struct Plugin {
    artifact_root: Option<PathBuf>,
}

impl Default for Plugin {
    fn default() -> Self {
        Self {
            artifact_root: None,
        }
    }
}

impl Plugin {
    /// Creates the plugin with an explicit session artifact root.
    ///
    /// This root takes precedence over [`crate::AUTOMATION_CONTROL_ARTIFACT_DIR`]. [`Default`]
    /// instead resolves the environment-aware fallback through [`crate::artifact_root_path`].
    pub fn with_artifact_root(path: impl Into<PathBuf>) -> Self {
        Self {
            artifact_root: Some(path.into()),
        }
    }
}

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Completions>();
    }

    fn finish(&self, app: &mut App) {
        let renderer_installed = app.get_sub_app(RenderApp).is_some();
        let capture_installed = app.world().contains_resource::<CapturedScreenshots>();
        if renderer_installed && capture_installed {
            let artifact_root = self
                .artifact_root
                .clone()
                .unwrap_or_else(|| artifact_root_path("artifacts"));
            match open_artifact_root(&artifact_root) {
                Ok(directory) => {
                    app.insert_resource(Service {
                        artifact_root,
                        directory,
                    });
                }
                Err(error) => eprintln!("automation-control screenshot unavailable: {error}"),
            }
        }
    }
}

#[derive(Resource)]
pub(crate) struct Service {
    artifact_root: PathBuf,
    directory: Dir,
}

#[derive(Clone, Debug)]
pub(crate) struct Capture {
    pub entity: Entity,
}

#[derive(Component)]
struct Destination(PathBuf);

#[derive(Default, Resource)]
struct Completions(Vec<(Entity, Result<Value, Error>)>);

pub(crate) fn is_available(world: &World) -> bool {
    world.contains_resource::<Service>()
}

pub(crate) fn start(world: &mut World, command: &Command) -> Result<Capture, Error> {
    let relative_path = validate_relative_path(Path::new(&command.path))?;
    let service = world
        .get_resource::<Service>()
        .ok_or(Error::CapabilityUnavailable)?;
    prepare_target(&service.artifact_root, &relative_path)?;

    let mut query = world.query_filtered::<Entity, With<PrimaryWindow>>();
    let mut windows = query.iter(world);
    let Some(window) = windows.next() else {
        return Err(Error::Capture(
            "primary rendered window is unavailable".into(),
        ));
    };
    if windows.next().is_some() {
        return Err(Error::Capture(
            "more than one primary rendered window is available".into(),
        ));
    }

    let entity = world
        .spawn((Screenshot::window(window), Destination(relative_path)))
        .observe(complete_capture)
        .id();
    Ok(Capture { entity })
}

pub(crate) fn take_completion(
    world: &mut World,
    capture: &Capture,
) -> Option<Result<Value, Error>> {
    let mut completions = world.get_resource_mut::<Completions>()?;
    let index = completions
        .0
        .iter()
        .position(|(entity, _)| *entity == capture.entity)?;
    Some(completions.0.swap_remove(index).1)
}

fn complete_capture(
    event: On<ScreenshotCaptured>,
    destinations: Query<&Destination>,
    service: Res<Service>,
    mut completions: ResMut<Completions>,
) {
    let entity = event.entity;
    let result = destinations
        .get(entity)
        .map_err(|_| Error::Capture("screenshot destination was lost".into()))
        .and_then(|destination| finish(&service, &destination.0, event.image.clone()));
    completions.0.push((entity, result));
}

fn finish(service: &Service, relative_path: &Path, image: Image) -> Result<Value, Error> {
    prepare_target(&service.artifact_root, relative_path)?;
    let width = image.texture_descriptor.size.width;
    let height = image.texture_descriptor.size.height;
    write_png(&service.directory, relative_path, image)?;
    Ok(json!({
        "artifact": {
            "type": "screenshot",
            "path": path_to_wire(relative_path)?,
            "mime_type": MIME_TYPE,
            "width": width,
            "height": height,
        }
    }))
}

fn validate_relative_path(path: &Path) -> Result<PathBuf, Error> {
    if path.as_os_str().is_empty() {
        return Err(Error::EmptyPath);
    }
    let wire = path
        .to_str()
        .ok_or_else(|| Error::InvalidPath("artifact path must be valid UTF-8".into()))?;
    if path.is_absolute() || wire.starts_with(['/', '\\']) || wire.as_bytes().get(1) == Some(&b':')
    {
        return Err(Error::AbsolutePath);
    }
    if wire.contains('\\') {
        return Err(Error::InvalidPath(
            "artifact path must use forward slashes".into(),
        ));
    }
    if wire.split('/').any(|component| component.is_empty()) {
        return Err(Error::InvalidPath(
            "artifact path must not contain empty components".into(),
        ));
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(component) => normalized.push(component),
            Component::ParentDir | Component::CurDir => return Err(Error::PathTraversal),
            Component::RootDir | Component::Prefix(_) => return Err(Error::AbsolutePath),
        }
    }
    if normalized.extension().and_then(|value| value.to_str()) != Some("png") {
        return Err(Error::InvalidPath(
            "screenshot artifact path must end in .png".into(),
        ));
    }
    Ok(normalized)
}

fn open_artifact_root(root: &Path) -> Result<Dir, Error> {
    fs::create_dir_all(root).map_err(|error| {
        Error::Io(format!(
            "failed to create artifact root {}: {error}",
            root.display()
        ))
    })?;
    reject_symlink(root)?;
    Dir::open_ambient_dir(root, ambient_authority()).map_err(|error| {
        Error::Io(format!(
            "failed to open artifact root {}: {error}",
            root.display()
        ))
    })
}

fn prepare_target(root: &Path, relative: &Path) -> Result<PathBuf, Error> {
    fs::create_dir_all(root).map_err(|error| {
        Error::Io(format!(
            "failed to create artifact root {}: {error}",
            root.display()
        ))
    })?;
    reject_symlink(root)?;
    let canonical_root = fs::canonicalize(root).map_err(|error| {
        Error::Io(format!(
            "failed to resolve artifact root {}: {error}",
            root.display()
        ))
    })?;

    let parent = relative.parent().unwrap_or_else(|| Path::new(""));
    let mut current = canonical_root.clone();
    for component in parent.components() {
        let Component::Normal(component) = component else {
            return Err(Error::PathTraversal);
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(Error::SymlinkEscape(current));
                }
                if !metadata.is_dir() {
                    return Err(Error::InvalidPath(format!(
                        "artifact parent {} is not a directory",
                        current.display()
                    )));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current).map_err(|error| {
                    Error::Io(format!(
                        "failed to create artifact directory {}: {error}",
                        current.display()
                    ))
                })?;
            }
            Err(error) => {
                return Err(Error::Io(format!(
                    "failed to inspect artifact directory {}: {error}",
                    current.display()
                )));
            }
        }
        ensure_below_root(&canonical_root, &current)?;
    }

    let target = canonical_root.join(relative);
    if let Ok(metadata) = fs::symlink_metadata(&target) {
        if metadata.file_type().is_symlink() {
            return Err(Error::SymlinkEscape(target));
        }
        ensure_below_root(&canonical_root, &target)?;
        return Err(Error::InvalidPath(format!(
            "artifact target {} already exists",
            target.display()
        )));
    }
    Ok(target)
}

fn reject_symlink(path: &Path) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| Error::Io(format!("failed to inspect {}: {error}", path.display())))?;
    if metadata.file_type().is_symlink() {
        Err(Error::SymlinkEscape(path.to_path_buf()))
    } else {
        Ok(())
    }
}

fn ensure_below_root(root: &Path, path: &Path) -> Result<(), Error> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| Error::Io(format!("failed to resolve {}: {error}", path.display())))?;
    if canonical.starts_with(root) {
        Ok(())
    } else {
        Err(Error::OutsideArtifactRoot(canonical))
    }
}

fn write_png(directory: &Dir, relative: &Path, image: Image) -> Result<(), Error> {
    let dynamic = image
        .try_into_dynamic()
        .map_err(|error| Error::Capture(format!("captured image cannot be encoded: {error}")))?;
    let mut encoded = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(dynamic.to_rgb8())
        .write_to(&mut encoded, image::ImageFormat::Png)
        .map_err(|error| Error::Capture(format!("captured image cannot be encoded: {error}")))?;

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = directory.open_with(relative, &options).map_err(|error| {
        Error::Io(format!(
            "failed to create screenshot {}: {error}",
            relative.display()
        ))
    })?;
    file.write_all(encoded.get_ref())
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all())
        .map_err(|error| {
            Error::Io(format!(
                "failed to write screenshot {}: {error}",
                relative.display()
            ))
        })?;
    drop(file);
    verify_png(&directory, relative)
}

fn verify_png(directory: &Dir, relative: &Path) -> Result<(), Error> {
    let mut file = directory.open(relative).map_err(|error| {
        Error::Io(format!(
            "failed to reopen written screenshot {}: {error}",
            relative.display()
        ))
    })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|error| {
        Error::Io(format!(
            "failed to read written screenshot {}: {error}",
            relative.display()
        ))
    })?;
    if bytes.len() <= 8 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(Error::Io(format!(
            "written screenshot {} is empty or not PNG",
            relative.display()
        )));
    }
    image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).map_err(|error| {
        Error::Io(format!(
            "written screenshot {} cannot be decoded: {error}",
            relative.display()
        ))
    })?;
    Ok(())
}

fn path_to_wire(path: &Path) -> Result<String, Error> {
    path.to_str()
        .map(|path| path.replace(std::path::MAIN_SEPARATOR, "/"))
        .ok_or_else(|| Error::InvalidPath("artifact path must be valid UTF-8".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        asset::RenderAssetUsages,
        render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    };

    fn temporary_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "automation-control-screenshot-{name}-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn test_image() -> Image {
        Image::new_fill(
            Extent3d {
                width: 2,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[255, 0, 255, 255],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        )
    }

    #[test]
    fn accepts_only_normalized_relative_png_paths() {
        assert_eq!(
            validate_relative_path(Path::new("shots/current.png")).unwrap(),
            PathBuf::from("shots/current.png")
        );
        assert_eq!(
            validate_relative_path(Path::new("/tmp/current.png")),
            Err(Error::AbsolutePath)
        );
        assert_eq!(
            validate_relative_path(Path::new("shots/../current.png")),
            Err(Error::PathTraversal)
        );
        assert_eq!(
            validate_relative_path(Path::new("./current.png")),
            Err(Error::PathTraversal)
        );
        for path in [
            "current.jpg",
            "shots//current.png",
            r"shots\current.png",
            r"C:\current.png",
        ] {
            assert!(
                validate_relative_path(Path::new(path)).is_err(),
                "accepted {path:?}"
            );
        }
    }

    #[test]
    fn writes_and_reopens_a_nonempty_png_before_success() {
        let root = temporary_dir("write");
        let relative = Path::new("captures/test.png");
        prepare_target(&root, relative).unwrap();
        let directory = open_artifact_root(&root).unwrap();
        write_png(&directory, relative, test_image()).unwrap();
        let bytes = fs::read(root.join(relative)).unwrap();
        assert!(bytes.len() > 8);
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));

        fs::remove_dir_all(root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlink_that_leaves_the_artifact_root() {
        use std::os::unix::fs::symlink;

        let root = temporary_dir("symlink-root");
        let outside = temporary_dir("symlink-outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, root.join("escape")).unwrap();

        let error = prepare_target(&root, Path::new("escape/capture.png")).unwrap_err();
        assert!(matches!(error, Error::SymlinkEscape(_)));
        assert!(!outside.join("capture.png").exists());

        fs::remove_dir_all(root).ok();
        fs::remove_dir_all(outside).ok();
    }

    #[test]
    fn identical_relative_names_stay_in_their_session_roots() {
        let first = temporary_dir("first");
        let second = temporary_dir("second");
        let relative = Path::new("capture.png");
        let first_target = prepare_target(&first, relative).unwrap();
        let second_target = prepare_target(&second, relative).unwrap();
        let first_directory = open_artifact_root(&first).unwrap();
        let second_directory = open_artifact_root(&second).unwrap();
        write_png(&first_directory, relative, test_image()).unwrap();
        write_png(&second_directory, relative, test_image()).unwrap();

        assert_ne!(first_target, second_target);
        assert!(first_target.starts_with(fs::canonicalize(&first).unwrap()));
        assert!(second_target.starts_with(fs::canonicalize(&second).unwrap()));
        assert!(first.join(relative).is_file());
        assert!(second.join(relative).is_file());

        fs::remove_dir_all(first).ok();
        fs::remove_dir_all(second).ok();
    }
}
